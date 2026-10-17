//! Bounded native feedback projection and coalesced per-session tone acknowledgement.
//! This is a presentation adapter; Runtime gates the request while capture admission
//! is idle. NSBeep gives no playback-end receipt; later capture can overlap decay.
use crate::contracts::{SessionOutcome, SessionPhase, Snapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackState {
    pub status: String,
    pub held_session: Option<u64>,
    pub current_held: bool,
    pub idle: bool,
    pub running: bool,
    pub attention: bool,
}
pub fn project(snapshot: &Snapshot) -> FeedbackState {
    let mut status = crate::header::project(snapshot).status;
    let current_held = matches!(snapshot.session.outcome, Some(SessionOutcome::Held { .. }));
    if let Some(SessionOutcome::Held { message }) = &snapshot.session.outcome {
        status.push_str(" · ");
        status.push_str(&bounded(message, 256));
        if let Some(error) = &snapshot.error
            && error != message
        {
            status.push_str(" · ");
            status.push_str(&bounded(error, 128));
        }
    }
    FeedbackState {
        status: bounded(&status, 512),
        held_session: snapshot
            .session
            .last_held_session
            .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991),
        current_held,
        idle: snapshot.session.phase == SessionPhase::Idle,
        running: snapshot.running,
        attention: current_held
            || (snapshot.session.phase == SessionPhase::Recording
                && snapshot.session.maximum_warning)
            || snapshot.session.maximum_reached,
    }
}
fn bounded(value: &str, bytes: usize) -> String {
    if value.len() <= bytes {
        return value.to_owned();
    }
    let end = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= bytes)
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayPresentation {
    pub title: &'static str,
    pub text: String,
}
#[derive(Default)]
pub struct Feedback {
    latest: Option<FeedbackState>,
    highest_toned: u64,
}
impl Feedback {
    /// Publish status before requesting sound. `tone` must enter Runtime's
    /// with_idle_feedback guard and return true only after Some(Ok(())).
    /// False defers to a later poll; errors retain the pending session identity.
    pub fn update(
        &mut self,
        state: &FeedbackState,
        publish: impl FnOnce(&TrayPresentation) -> Result<(), String>,
        tone: impl FnOnce(u64) -> Result<bool, String>,
    ) -> Result<(), String> {
        let pending = state.held_session.filter(|id| *id > self.highest_toned);
        if self.latest.as_ref() != Some(state) {
            let mut text = state.status.clone();
            if pending.is_some() && !state.current_held {
                text.push_str(" · Ein vorheriges Diktat wurde zurückgehalten");
            }
            publish(&TrayPresentation {
                title: if state.running && (state.attention || pending.is_some()) {
                    "!"
                } else {
                    ""
                },
                text,
            })?;
            self.latest = Some(state.clone());
        }
        if state.running
            && state.idle
            && let Some(session) = pending
            && tone(session)?
        {
            self.highest_toned = session;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishes_before_tone_and_acknowledges_identity_not_text() -> Result<(), String> {
        let mut feedback = Feedback::default();
        let mut state = FeedbackState {
            status: "Text wurde zurückgehalten · Fokus gewechselt".into(),
            held_session: Some(1),
            current_held: true,
            idle: true,
            running: true,
            attention: true,
        };
        let events = std::cell::RefCell::new(Vec::new());
        feedback.update(
            &state,
            |_| {
                events.borrow_mut().push("status");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("tone");
                Ok(true)
            },
        )?;
        assert_eq!(*events.borrow(), ["status", "tone"]);
        feedback.update(
            &state,
            |_| Err("duplicate status".into()),
            |_| Err("duplicate tone".into()),
        )?;
        state.held_session = Some(2); // Identical text in a distinct session.
        feedback.update(
            &state,
            |_| Ok(()),
            |id| {
                assert_eq!(id, 2);
                Ok(true)
            },
        )?;
        state.held_session = Some(1); // A stale identity cannot cause another tone.
        feedback.update(&state, |_| Ok(()), |_| Err("stale tone".into()))?;
        Ok(())
    }
    #[test]
    fn defers_while_active_coalesces_to_latest_and_keeps_failure_pending() -> Result<(), String> {
        let mut feedback = Feedback::default();
        let mut state = FeedbackState {
            status: "Aufnahme läuft".into(),
            held_session: Some(1),
            current_held: false,
            idle: false,
            running: true,
            attention: false,
        };
        let text = std::cell::RefCell::new(String::new());
        feedback.update(
            &state,
            |view| {
                *text.borrow_mut() = view.text.clone();
                Ok(())
            },
            |_| Err("tone during capture".into()),
        )?;
        assert!(
            text.borrow()
                .contains("Ein vorheriges Diktat wurde zurückgehalten")
        );
        state.held_session = Some(3); // Intermediate Held 2 is intentionally coalesced.
        state.idle = true;
        state.status = "Einfügebefehl gesendet".into();
        feedback.update(
            &state,
            |_| Ok(()),
            |id| {
                assert_eq!(id, 3);
                Ok(false)
            },
        )?;
        assert_eq!(
            feedback.update(
                &state,
                |_| Err("duplicate publish".into()),
                |_| Err("sound request failed".into())
            ),
            Err("sound request failed".into())
        );
        feedback.update(
            &state,
            |_| Err("duplicate publish".into()),
            |id| {
                assert_eq!(id, 3);
                Ok(true)
            },
        )?;
        feedback.update(
            &state,
            |_| Err("duplicate publish".into()),
            |_| Err("duplicate tone".into()),
        )?;
        state.running = false;
        state.held_session = Some(4);
        feedback.update(&state, |_| Ok(()), |_| Err("tone after shutdown".into()))?;
        Ok(())
    }
    #[test]
    fn failed_status_publish_never_requests_sound() {
        let mut feedback = Feedback::default();
        let state = FeedbackState {
            status: "Text wurde zurückgehalten".into(),
            held_session: Some(1),
            current_held: true,
            idle: true,
            running: true,
            attention: true,
        };
        assert_eq!(
            feedback.update(
                &state,
                |_| Err("tray unavailable".into()),
                |_| Err("must not request sound".into())
            ),
            Err("tray unavailable".into())
        );
    }
}
