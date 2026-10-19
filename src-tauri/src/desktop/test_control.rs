//! Explicit developer test commands. No arbitrary target, raw text or permission changes.
use super::*;

#[derive(Default)]
pub(super) struct State(Mutex<ControlState>);
#[derive(Default)]
struct ControlState {
    pending: bool,
    closed: bool,
    session: Option<u64>,
}
impl State {
    fn lock(&self) -> std::sync::MutexGuard<'_, ControlState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn reserve(&self) -> Result<(), String> {
        let mut state = self.lock();
        if state.closed {
            return Err("Test control expired".into());
        }
        if state.pending {
            return Err("Test start already pending".into());
        }
        state.pending = true;
        state.session = None;
        Ok(())
    }
    fn complete(&self, session: Option<u64>) -> bool {
        let mut state = self.lock();
        state.session = session;
        state.pending = false;
        state.closed
    }
    fn shutdown(&self) -> Option<u64> {
        let mut state = self.lock();
        state.closed = true;
        state.session
    }
}

impl Desktop {
    /// Metadata-only observation; never reserves admission or creates a Target.
    pub fn test_control_inspect(&self, expected_pid: i32) -> Result<serde_json::Value, String> {
        if self.0.test_control.lock().closed {
            return Err("Test control expired".into());
        }
        use macos_integration::target::{FieldRole, HoldReason, inspection};
        let metadata = inspection::inspect(expected_pid).map_err(str::to_owned)?;
        let eligibility = metadata.evidence.eligibility();
        let hold = eligibility.err().map(|reason| match reason {
            HoldReason::UnconfirmedField => "unconfirmedField",
            HoldReason::NotEditable => "notEditable",
            HoldReason::UnknownEditability => "unknownEditability",
            HoldReason::SecureField => "secureField",
        });
        Ok(serde_json::json!({
            "kind": "own-test-recipient-field-metadata", "pid": metadata.pid,
            "role": match metadata.role.value.unwrap_or(FieldRole::Other) {
                FieldRole::TextField => "textField", FieldRole::TextArea => "textArea",
                FieldRole::ComboBox => "comboBox", FieldRole::Other => "other",
            },
            "roleRead": metadata.role.status.as_str(), "roleErrorCode": metadata.role.error_code,
            "secure": metadata.evidence.secure,
            "subroleRead": metadata.subrole.status.as_str(), "subroleErrorCode": metadata.subrole.error_code,
            "enabled": metadata.enabled.value,
            "enabledRead": metadata.enabled.status.as_str(), "enabledErrorCode": metadata.enabled.error_code,
            "valueSettable": metadata.value_settable, "valueSettableErrorCode": metadata.value_settable_error_code,
            "selectedTextSettable": metadata.selected_text_settable, "selectedTextSettableErrorCode": metadata.selected_text_settable_error_code,
            "captureEligible": eligibility.is_ok(), "holdReason": hold,
        }))
    }

    pub fn test_control_status(&self) -> Result<serde_json::Value, String> {
        // Hold ownership through the runtime read: an ACK cannot turn pending
        // into owned between two fields of this status snapshot.
        let control = self.0.test_control.lock();
        let snapshot = self.snapshot()?;
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        let current = shared
            .client
            .as_ref()
            .and_then(|c| c.runtime().ok().flatten())
            .and_then(|r| r.snapshot().ok());
        let phase = current
            .as_ref()
            .map(|r| format!("{:?}", r.session.phase))
            .unwrap_or_else(|| format!("{:?}", snapshot.session.phase));
        let session = current
            .as_ref()
            .and_then(|r| r.session.session)
            .or(snapshot.session.id);
        let owned = control.session;
        Ok(status_projection(
            &snapshot,
            phase,
            session,
            owned.filter(|id| session == Some(*id)),
            control.pending,
        ))
    }

    /// Must run on main, before any window activation. Uses ordinary runtime start,
    /// with explicit-start semantics: it never toggles an unrelated recording off.
    pub fn test_control_start(&self, expected_pid: i32) -> Result<(), String> {
        let control = &self.0.test_control;
        if control.lock().closed {
            return Err("Test control expired".into());
        }
        let process = macos_integration::target::frontmost_test_recipient(expected_pid)
            .map_err(|_| "Own test recipient is not frontmost")?;
        let runtime = self.runtime()?;
        if runtime
            .snapshot()
            .map_err(|_| "Runtime unavailable")?
            .session
            .phase
            != Phase::Idle
        {
            return Err("An existing session is active".into());
        }
        if ui_services::microphone_authorization() != MicAuthorization::Granted
            || !macos_integration::target::accessibility_trusted()
            || macos_integration::target::secure_input_active()
        {
            return Err("Required native conditions are not satisfied".into());
        }
        control.reserve()?;
        let result = (|| {
            let mut admission = self
                .0
                .admission
                .lock()
                .map_err(|_| "Admission lock failed")?;
            if admission.suspended || admission.changing_settings || admission.pending_starts != 0 {
                return Err("Another native action is pending".to_owned());
            }
            let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
            if shared.preferences.needs_restart || shared.settings.device.is_some() {
                return Err(
                    "Test requires ready settings and the system-default microphone".into(),
                );
            }
            let settings = shared.settings.clone();
            drop(shared);
            let reply = runtime
                .start_controlled(
                    SessionSettings {
                        device: None,
                        language: settings.language.into(),
                        generation: 1,
                    },
                    process,
                )
                .map_err(|_| "Test start could not be queued")?;
            admission.pending_starts += 1;
            let reservation = Reservation {
                owner: Arc::clone(&self.0),
                kind: ReservationKind::Start,
            };
            drop(admission);
            let desktop = self.clone();
            tauri::async_runtime::spawn(async move {
                let _reservation = reservation;
                match reply.await {
                    Ok(Ok(session)) => {
                        if desktop.0.test_control.complete(Some(session)) {
                            let _result = runtime.cancel(session);
                        }
                    }
                    _ => {
                        desktop.0.test_control.complete(None);
                        desktop.action_error("Automatischer Teststart fehlgeschlagen".into());
                    }
                }
            });
            Ok(())
        })();
        if result.is_err() {
            control.complete(None);
        }
        result
    }

    pub fn test_control_stop(&self, session: u64) -> Result<(), String> {
        self.test_control_owned(session)?;
        self.stop_recording(session)
    }
    pub fn test_control_cancel(&self, session: u64) -> Result<(), String> {
        self.test_control_owned(session)?;
        self.cancel_recording(session)
    }
    fn test_control_owned(&self, session: u64) -> Result<(), String> {
        if session == 0 || self.0.test_control.lock().session != Some(session) {
            return Err("Session is not owned by test control".into());
        }
        Ok(())
    }
    pub fn test_control_shutdown(&self) -> Result<(), String> {
        if let Some(session) = self.0.test_control.shutdown() {
            self.cancel_recording(session)?;
        }
        Ok(())
    }
}

fn status_projection(
    snapshot: &view::Snapshot,
    phase: String,
    session: Option<u64>,
    owned: Option<u64>,
    pending: bool,
) -> serde_json::Value {
    use view::SessionOutcome;
    let outcome = snapshot.session.outcome.as_ref().map(|o| match o {
        SessionOutcome::Local => "local",
        SessionOutcome::Empty => "empty",
        SessionOutcome::Sent => "sent",
        SessionOutcome::Cancelled => "cancelled",
        SessionOutcome::Held { .. } => "held",
        SessionOutcome::Failed { .. } => "failed",
    });
    let expected = "Forschung im Bereich KI umfasst die Herstellung von Maschinen zur Automatisierung von Aufgaben, die intelligentes Verhalten erfordern.";
    serde_json::json!({
        "setupReady": snapshot.setup.phase == view::SetupPhase::Ready,
        "microphoneGranted": snapshot.permissions.microphone == view::PermissionStatus::Granted,
        "accessibilityGranted": snapshot.permissions.accessibility == view::PermissionStatus::Granted,
        "phase": phase, "sessionId": session, "controlledSessionId": owned,
        "controlledStartPending": pending, "outcome": outcome,
        "publicSentenceMatches": snapshot.session.text.split_whitespace().collect::<Vec<_>>().join(" ") == expected,
        "transcriptCharacters": snapshot.session.text.chars().count(),
        "actionError": snapshot.error.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_and_expiry_always_leave_an_exact_session_to_cancel()
    -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..100 {
            let state = Arc::new(State::default());
            state.reserve()?;
            let barrier = Arc::new(std::sync::Barrier::new(3));
            let ack_state = state.clone();
            let ack_barrier = barrier.clone();
            let ack = thread::spawn(move || {
                ack_barrier.wait();
                ack_state.complete(Some(7))
            });
            let close_state = state.clone();
            let close_barrier = barrier.clone();
            let close = thread::spawn(move || {
                close_barrier.wait();
                close_state.shutdown()
            });
            barrier.wait();
            let late_ack_must_cancel = ack.join().map_err(|_| "ack thread panicked")?;
            let close_session = close.join().map_err(|_| "close thread panicked")?;
            assert!(late_ack_must_cancel || close_session == Some(7));
            let snapshot = state.lock();
            assert!(!snapshot.pending);
            assert_eq!(snapshot.session, Some(7));
            assert!(snapshot.closed);
        }
        Ok(())
    }
    #[test]
    fn failed_receipt_allows_retry_without_stale_ownership()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = State::default();
        state.reserve()?;
        state.complete(None);
        assert!(state.reserve().is_ok());
        let snapshot = state.lock();
        assert!(snapshot.pending);
        assert_eq!(snapshot.session, None);
        Ok(())
    }
    #[test]
    fn status_does_not_export_private_transcript_or_error_text() {
        let mut state = super::super::initial();
        state.session.text = "private sentinel recording".into();
        state.error = Some("private error sentinel".into());
        state.session.outcome = Some(view::SessionOutcome::Held {
            message: "private target sentinel".into(),
        });
        let value = status_projection(&state, "Idle".into(), Some(2), None, false);
        let encoded = value.to_string();
        assert!(!encoded.contains("sentinel"));
        assert_eq!(value["outcome"], "held");
        assert_eq!(value["actionError"], true);
        assert_eq!(value["publicSentenceMatches"], false);
        assert_eq!(value["controlledSessionId"], serde_json::Value::Null);
    }
    #[test]
    fn unrelated_session_is_never_controllable() {
        let desktop = super::super::tests::unstarted_desktop();
        assert!(desktop.test_control_stop(1).is_err());
        assert!(desktop.test_control_cancel(1).is_err());
        assert!(desktop.test_control_stop(0).is_err());
    }
}
