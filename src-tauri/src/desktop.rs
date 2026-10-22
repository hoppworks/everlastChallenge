//! Desktop owner and renderer-facing projection. Session truth stays in Runtime.
use crate::{
    application::{AppSupervisor, Client as AppClient},
    contracts as view,
    controller::SessionSettings,
    hotkeys::{self, Hotkeys},
    preferences::{HotkeyRebinder, PreferencesError, PreferencesStore, SettingsDraft},
    runtime::Client as RuntimeClient,
    settings::Settings,
    setup,
    startup_settings::StartupSettings,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use dictation_core::session::{Outcome, Phase};
use macos_integration::{
    target::ProcessIdentity,
    ui_services::{self, MicAuthorization},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::{Emitter, Manager};
use tokio::sync::{Semaphore, oneshot};

#[cfg(feature = "measurement")]
#[path = "desktop/start_observer.rs"]
mod start_observer;

#[cfg(feature = "test-control")]
mod test_control;

mod action_errors;

pub const SNAPSHOT_EVENT: &str = "dictation:snapshot";
struct Shared {
    view: view::Snapshot,
    client: Option<AppClient>,
    settings: Settings,
    action_errors: action_errors::ActionErrors,
    settings_requests: u32,
    key_labels: macos_integration::keyboard::KeyLabels,
    preferences: PreferenceState,
}
#[derive(Default)]
struct PreferenceState {
    devices: Vec<crate::devices::InputDevice>,
    error: Option<String>,
    needs_restart: bool,
    recovery_required: bool,
}
#[derive(Default)]
struct Admission {
    suspended: bool,
    memory_pressure: bool,
    pending_starts: usize,
    changing_settings: bool,
    refreshing_devices: bool,
}
enum ReservationKind {
    Start,
    Settings,
    Devices,
}
struct Reservation {
    owner: Arc<Inner>,
    kind: ReservationKind,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut admission = self
            .owner
            .admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.kind {
            ReservationKind::Start => admission.pending_starts -= 1,
            ReservationKind::Settings => admission.changing_settings = false,
            ReservationKind::Devices => admission.refreshing_devices = false,
        }
    }
}
enum SettingsCommand {
    Reset {
        reply: oneshot::Sender<Result<(), String>>,
        _reservation: Reservation,
    },
    Save {
        draft: SettingsDraft,
        reply: oneshot::Sender<Result<(), String>>,
        _reservation: Reservation,
    },
    Devices {
        reply: oneshot::Sender<Result<(), String>>,
        _reservation: Reservation,
    },
}
struct Inner {
    #[cfg(feature = "test-control")]
    test_control: test_control::State,
    #[cfg(feature = "measurement")]
    native_starts: Mutex<start_observer::Observer>,
    #[cfg(feature = "measurement")]
    native_starts_enabled: bool,
    #[cfg(feature = "measurement")]
    native_starts_lost: AtomicBool,
    shared: Mutex<Shared>,
    closing: AtomicBool,
    refresh: AtomicBool,
    worker: Mutex<Option<JoinHandle<Result<(), String>>>>,
    finished: AtomicBool,
    admissions: Arc<Semaphore>,
    prompts: Arc<Semaphore>,
    // Serializes a power transition with start admission and runtime handoff.
    admission: Mutex<Admission>,
    requests: Sender<SettingsCommand>,
}
#[derive(Clone, Copy)]
enum StartOwner {
    Native,
    #[cfg(all(feature = "measurement", feature = "test-control"))]
    TestControl,
}
#[derive(Clone)]
pub struct Desktop(Arc<Inner>);
impl Desktop {
    #[cfg(feature = "measurement")]
    fn start_measurement(&self, action: impl FnOnce(&mut start_observer::Observer)) {
        if !self.0.native_starts_enabled {
            return;
        }
        // Observation must never wait for another observer or block a real command.
        if let Ok(mut observer) = self.0.native_starts.try_lock() {
            action(&mut observer);
        } else {
            self.0.native_starts_lost.store(true, Ordering::Release);
        }
    }
    #[cfg(feature = "measurement")]
    fn start_measurement_interference(&self, reason: &'static str) {
        self.start_measurement(|o| o.invalidate(reason));
    }
    fn start_measurement_begin(&self, _origin: hotkeys::NativeOrigin) -> Option<u64> {
        #[cfg(feature = "measurement")]
        {
            if !self.0.native_starts_enabled {
                return None;
            }
            let Some(at) = _origin.received_at else {
                self.0.native_starts_lost.store(true, Ordering::Release);
                return None;
            };
            if let Ok(mut o) = self.0.native_starts.try_lock() {
                return o.begin(at);
            }
            self.0.native_starts_lost.store(true, Ordering::Release);
        }
        None
    }
    fn start_measurement_ack(&self, _ticket: Option<u64>, _success: bool) {
        #[cfg(feature = "measurement")]
        if let Some(ticket) = _ticket {
            let state = self
                .runtime()
                .ok()
                .and_then(|r| r.snapshot().ok())
                .map(|s| native_start_state(&s));
            let at = std::time::Instant::now();
            self.start_measurement(|o| o.ack(ticket, _success, state, at));
        }
    }
    /// Developer-only readout. No IPC, export side effect, automatic recording or pass decision.
    #[cfg(feature = "measurement")]
    pub fn native_start_observations(&self) -> serde_json::Value {
        if !self.0.native_starts_enabled {
            return serde_json::json!({"schemaVersion":1,"kind":"native-start-upper-bounds-v1","enabled":false,"qualification":false});
        }
        let Ok(mut o) = self.0.native_starts.try_lock() else {
            return serde_json::json!({"kind":"native-start-upper-bounds-v1", "invalid":true, "error":"observer-unavailable"});
        };
        o.expire(std::time::Instant::now());
        serde_json::json!({"schemaVersion":1,"kind":"native-start-upper-bounds-v1","enabled":true,
            "maximumRecords":start_observer::MAX_RECORDS,"pendingOrdinal":o.pending_ordinal(),
            "overflowAttempts":o.overflow(),"counterError":o.counter_error(),
            "observerContentionOrPoison":self.0.native_starts_lost.load(Ordering::Acquire),
            "qualification":false,"exactCpalLatency":false,"L":null,
            "records":o.records().iter().map(|r|serde_json::json!({"ordinal":r.ordinal,"session":r.sid,
                "observedUpperNanos":r.upper_nanos,"ackOffsetNanos":r.ack_nanos,"reason":r.reason})).collect::<Vec<_>>()})
    }

    pub fn spawn(app: tauri::AppHandle) -> Result<Self, String> {
        Self::spawn_inner(app, false)
    }
    #[cfg(feature = "measurement")]
    pub fn spawn_observing_native_start(app: tauri::AppHandle) -> Result<Self, String> {
        Self::spawn_inner(app, true)
    }
    fn spawn_inner(app: tauri::AppHandle, _observe_native_starts: bool) -> Result<Self, String> {
        let (requests, receive) = bounded(8);
        let desktop = Self(Arc::new(Inner {
            #[cfg(feature = "test-control")]
            test_control: test_control::State::default(),
            shared: Mutex::new(Shared {
                view: initial(),
                client: None,
                settings: Settings::default(),
                action_errors: Default::default(),
                settings_requests: 0,
                key_labels: Default::default(),
                preferences: PreferenceState::default(),
            }),
            closing: AtomicBool::new(false),
            refresh: AtomicBool::new(true),
            worker: Mutex::new(None),
            finished: AtomicBool::new(false),
            admissions: Arc::new(Semaphore::new(16)),
            prompts: Arc::new(Semaphore::new(1)),
            admission: Mutex::new(Admission::default()),
            requests,
            #[cfg(feature = "measurement")]
            native_starts: Mutex::new(start_observer::Observer::default()),
            #[cfg(feature = "measurement")]
            native_starts_enabled: _observe_native_starts,
            #[cfg(feature = "measurement")]
            native_starts_lost: AtomicBool::new(false),
        }));
        let owner = desktop.clone();
        let worker = thread::Builder::new()
            .name("desktop-owner".into())
            .spawn(move || {
                let _finished = Finish(Arc::clone(&owner.0));
                owner.run(app, receive)
            })
            .map_err(|error| error.to_string())?;
        *desktop.0.worker.lock().map_err(|_| "Desktop lock failed")? = Some(worker);
        Ok(desktop)
    }
    fn install_hotkeys(
        &self,
        app: tauri::AppHandle,
        key: crate::settings::Hotkey,
    ) -> Result<Hotkeys, hotkeys::HotkeyError> {
        let callback = self.clone();
        #[cfg(feature = "measurement")]
        {
            Hotkeys::install_observed(
                app,
                key,
                move |event| callback.hotkey(event),
                self.0.native_starts_enabled,
            )
        }
        #[cfg(not(feature = "measurement"))]
        {
            Hotkeys::install(app, key, move |event| callback.hotkey(event))
        }
    }
    pub fn snapshot(&self) -> Result<view::Snapshot, String> {
        Ok(self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .view
            .clone())
    }
    /// Queue a persistent UI request, so a late renderer subscription cannot lose it.
    pub fn request_settings(&self) -> Result<u32, String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        // Navigation is safe during setup/recording. Individual settings
        // mutations retain their existing admission checks.
        shared.settings_requests = shared
            .settings_requests
            .checked_add(1)
            .ok_or("Einstellungsanfragen ausgeschöpft; Anwendung neu starten")?;
        Ok(shared.settings_requests)
    }
    /// Published settings navigation counter; reading it never clones a transcript.
    pub fn settings_request(&self) -> Result<u32, String> {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok(shared.view.settings_request)
    }
    /// Read only the bounded native projection when its revision changes.
    pub fn native_header_after(
        &self,
        revision: Option<u64>,
    ) -> Result<Option<(u64, macos_integration::glass_header::HeaderState)>, String> {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok((revision != Some(shared.view.revision))
            .then(|| (shared.view.revision, crate::header::project(&shared.view))))
    }
    /// A bounded projection: no transcript clone crosses into native feedback.
    pub fn native_feedback_after(
        &self,
        revision: Option<u64>,
    ) -> Result<Option<(u64, crate::feedback::FeedbackState)>, String> {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok((revision != Some(shared.view.revision))
            .then(|| (shared.view.revision, crate::feedback::project(&shared.view))))
    }
    /// Main-thread-only sound request, gated against actual Runtime admission.
    /// Desktop/App locks have been released by runtime() before entering it.
    pub fn held_feedback_tone(&self, session: u64) -> Result<bool, String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(false);
        }
        let runtime = self.runtime()?;
        match runtime
            .with_idle_feedback(session, macos_integration::feedback::held_tone)
            .map_err(|error| error.to_string())?
        {
            Some(result) => result.map(|()| true).map_err(|error| error.to_string()),
            None => Ok(false),
        }
    }
    pub fn native_action(&self, action: macos_integration::glass_header::HeaderAction) {
        use macos_integration::glass_header::HeaderAction;
        let ticket = match self.begin_action() {
            Ok(ticket) => ticket,
            Err(error) => {
                self.action_error(error);
                return;
            }
        };
        let result = (|| {
            match action {
                HeaderAction::Settings => self.request_settings().map(|_| ()),
                HeaderAction::Stop(session) => self.stop_recording(session.get()),
                HeaderAction::Cancel(session) => self.cancel_recording(session.get()),
                HeaderAction::Start => {
                    let admission = self.ui_permit()?;
                    let reply = self.start_request(None, false, false)?;
                    let owner = self.clone();
                    let pending = Arc::clone(&ticket);
                    tauri::async_runtime::spawn(async move {
                        let _admission = admission;
                        let result = reply
                            .await
                            .map_err(|_| "Aufnahmeanfrage beendet".to_owned())
                            .and_then(|result| result);
                        owner.finish_action(&pending, result);
                    });
                    return Ok(true);
                }
            }?;
            Ok(false)
        })();
        match result {
            Ok(true) => {} // The asynchronous acknowledgement owns completion.
            Ok(false) => self.finish_action(&ticket, Ok(())),
            Err(error) => self.finish_action(&ticket, Err(error)),
        }
    }
    /// Install one complete native layout snapshot, independent of the selected
    /// settings key. Callers publish the returned revision via SNAPSHOT_EVENT.
    /// Both selection and labels are projected under the same lock, so a layout
    /// refresh cannot restore an older hotkey. No native work occurs under lock.
    pub fn update_hotkey_labels(
        &self,
        labels: macos_integration::keyboard::KeyLabels,
    ) -> Result<Option<view::Snapshot>, String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(None);
        }
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(None);
        }
        shared.key_labels = labels;
        let current = draft_of(&shared.settings);
        let hotkey = crate::hotkey_labels::shortcut(&shared.settings.hotkey, &shared.key_labels);
        let choices = crate::hotkey_labels::options(&shared.key_labels);
        if hotkey == shared.view.labels.hotkey
            && choices == shared.view.preferences.key_labels
            && current == shared.view.preferences.current
        {
            return Ok(None);
        }
        let revision = shared
            .view
            .revision
            .checked_add(1)
            .ok_or("Snapshot revision exhausted")?;
        shared.view.preferences.current = current;
        shared.view.labels.hotkey = hotkey;
        shared.view.preferences.key_labels = choices;
        shared.view.revision = revision;
        Ok(Some(shared.view.clone()))
    }
    pub fn refresh_permissions(&self) {
        self.0.refresh.store(true, Ordering::Release);
    }
    pub fn set_memory_pressure(&self, pressured: bool) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("memory-pressure");
        let mut admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        admission.memory_pressure = pressured;
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(());
        }
        let client = self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .client
            .clone();
        if let Some(client) = client
            && let Some(runtime) = client.runtime().map_err(|e| e.to_string())?
        {
            runtime
                .set_memory_pressure(pressured)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Called by the native notification observer. Never joins workers or resumes recording.
    pub fn set_suspended(&self, sleeping: bool) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("power-transition");
        let mut suspended = self.0.admission.lock().map_err(|_| "Power lock failed")?;
        suspended.suspended = sleeping;
        if !suspended.suspended {
            self.refresh_permissions();
        }
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(());
        }
        let client = self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .client
            .clone();
        if let Some(client) = client {
            let runtime = match client.runtime() {
                Ok(runtime) => runtime,
                Err(_) if self.0.closing.load(Ordering::Acquire) => return Ok(()),
                Err(error) => return Err(error.to_string()),
            };
            if let Some(runtime) = runtime {
                if suspended.suspended {
                    runtime.suspend().map_err(|error| error.to_string())?;
                } else {
                    runtime.resume().map_err(|error| error.to_string())?;
                }
            }
        }
        Ok(())
    }
    fn runtime(&self) -> Result<RuntimeClient, String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        let client = self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .client
            .clone()
            .ok_or("Anwendung wird eingerichtet")?;
        client
            .runtime()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Spracherkennung ist noch nicht bereit".into())
    }
    fn app_client(&self) -> Result<AppClient, String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        self.0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .client
            .clone()
            .ok_or_else(|| "Anwendung wird eingerichtet".into())
    }
    fn action_error(&self, error: String) {
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_errors.report(error);
        }
    }
    fn hotkey_error(&self, error: String) {
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_errors.persistent(Some(error));
        }
    }
    fn begin_action(&self) -> Result<Arc<()>, String> {
        Ok(self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .action_errors
            .begin())
    }
    fn finish_action(&self, ticket: &Arc<()>, result: Result<(), String>) {
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_errors.complete(ticket, result);
        }
    }
    pub fn download(&self) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("model-setup");
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_errors.clear_transient();
        }
        self.app_client()?
            .download()
            .map_err(|error| error.to_string())
    }
    pub fn cancel_setup(&self) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("model-setup-cancel");
        self.app_client()?
            .cancel_setup()
            .map_err(|error| error.to_string())
    }
    pub fn download_refinement(&self) -> Result<(), String> {
        let admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        if admission.suspended || admission.changing_settings || admission.pending_starts != 0 {
            return Err(
                "Bitte laufende Aufnahmen oder Einstellungsänderungen zuerst abschließen.".into(),
            );
        }
        let client = self.app_client()?;
        if client
            .runtime()
            .map_err(|e| e.to_string())?
            .map(|r| r.snapshot())
            .transpose()
            .map_err(|e| e.to_string())?
            .is_some_and(|r| r.session.phase != Phase::Idle)
        {
            return Err("Bitte das laufende Diktat zuerst abschließen.".into());
        }
        client.download_refinement().map_err(|e| e.to_string())
    }
    pub fn cancel_refinement_setup(&self) -> Result<(), String> {
        self.app_client()?
            .cancel_refinement_setup()
            .map_err(|e| e.to_string())
    }
    fn start_request(
        &self,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        self.start_request_tagged(process, delivery, toggle, None, StartOwner::Native)
    }
    fn start_request_tagged(
        &self,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
        _measurement_ticket: Option<u64>,
        _owner: StartOwner,
    ) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        let mut admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        #[cfg(feature = "measurement")]
        if _measurement_ticket.is_none() {
            self.start_measurement_interference("another-ui-or-native-start");
        }
        if admission.suspended {
            return Err("Aufnahme während des Ruhezustands nicht verfügbar".into());
        }
        if admission.changing_settings || admission.pending_starts >= 16 {
            return Err(
                "Einstellungen werden geändert oder eine Aufnahme wird noch gestartet".into(),
            );
        }
        let settings = {
            let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
            if shared.preferences.needs_restart {
                return Err(
                    "Einstellungen konnten nicht wiederhergestellt werden; bitte neu starten"
                        .into(),
                );
            }
            shared.settings.clone()
        };
        let runtime = self.runtime()?;
        #[cfg(all(feature = "measurement", feature = "test-control"))]
        if matches!(_owner, StartOwner::TestControl) {
            // Keep the clean-idle check atomic with request handoff. A start
            // queued after the outer preflight must never turn this into a toggle-stop.
            let state = runtime.snapshot().map_err(|_| "Runtime unavailable")?;
            if admission.pending_starts != 0
                || settings.device.is_some()
                || state.session.phase != Phase::Idle
                || state.session.session.is_some()
                || !native_start_state(&state).ready
            {
                return Err("Host measurement admission is no longer clean Idle".into());
            }
        }
        #[cfg(feature = "measurement")]
        if let Some(ticket) = _measurement_ticket {
            let state = runtime.snapshot().ok().map(|s| native_start_state(&s));
            self.start_measurement(|o| {
                if let Some(state) = state {
                    o.admit(ticket, state, admission.pending_starts == 0);
                } else {
                    o.invalidate("admission-snapshot-unavailable");
                }
            });
        }
        let stopping = toggle
            && matches!(
                runtime
                    .snapshot()
                    .map_err(|error| error.to_string())?
                    .session
                    .phase,
                Phase::Starting | Phase::Recording
            );
        if !stopping
            && self
                .app_client()?
                .refinement_busy()
                .map_err(|e| e.to_string())?
        {
            return Err(
                "Bitte die Textmodelleinrichtung zuerst abschließen oder abbrechen.".into(),
            );
        }
        if !stopping && ui_services::microphone_authorization() != MicAuthorization::Granted {
            return Err("Bitte zuerst den Mikrofonzugriff freigeben".into());
        }
        let session_settings = SessionSettings {
            refinement: settings.refinement,
            device: settings.device,
            language: settings.language.into(),
            generation: 1,
        };
        let reply = match _owner {
            StartOwner::Native => runtime
                .start(session_settings, process, delivery, toggle)
                .map_err(|error| error.to_string())?,
            #[cfg(all(feature = "measurement", feature = "test-control"))]
            StartOwner::TestControl => {
                let reply = runtime
                    .start_host_controlled(
                        session_settings,
                        process.ok_or("Own recipient identity required")?,
                    )
                    .map_err(|error| error.to_string())?;
                self.forward_test_host_receipt(runtime.clone(), reply)
            }
        };
        admission.pending_starts += 1;
        let reservation = Reservation {
            owner: Arc::clone(&self.0),
            kind: ReservationKind::Start,
        };
        drop(admission);
        Ok(forward_start(reply, reservation))
    }
    pub async fn start_recording(&self) -> Result<(), String> {
        let ticket = self.begin_action()?;
        let result = async {
            let _admission = self.ui_permit()?;
            self.start_request(None, false, false)?
                .await
                .map_err(|_| "Aufnahmeanfrage beendet".to_owned())?
        }
        .await;
        self.finish_action(&ticket, result.clone());
        result
    }
    pub fn stop_recording(&self, session: u64) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("ui-stop");
        valid_id(session)?;
        self.runtime()?
            .stop(session)
            .map_err(|error| error.to_string())
    }
    pub fn cancel_recording(&self, session: u64) -> Result<(), String> {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("ui-cancel");
        valid_id(session)?;
        self.runtime()?
            .cancel(session)
            .map_err(|error| error.to_string())
    }
    pub async fn clear_text(&self, expected_text: String) -> Result<(), String> {
        let _admission = self
            .0
            .admissions
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Zu viele laufende Anfragen")?;
        self.runtime()?
            .clear_text_if_unchanged(expected_text)
            .map_err(|error| error.to_string())?
            .await
            .map_err(|_| "Textanfrage beendet")?
    }
    fn hotkey(&self, event: hotkeys::Event) {
        match event {
            hotkeys::Event::Cancel { session } => {
                #[cfg(feature = "measurement")]
                self.start_measurement_interference("hotkey-cancel");
                let result = self
                    .runtime()
                    .and_then(|runtime| runtime.cancel(session).map_err(|error| error.to_string()));
                if let Err(error) = result {
                    self.action_error(error);
                }
            }
            hotkeys::Event::Toggle {
                process,
                origin: _origin,
            } => {
                // A second press is a priority stop, before ordinary request admission.
                if let Ok(runtime) = self.runtime()
                    && let Ok(snapshot) = runtime.snapshot()
                    && matches!(snapshot.session.phase, Phase::Starting | Phase::Recording)
                    && let Some(session) = snapshot.session.session
                {
                    #[cfg(feature = "measurement")]
                    self.start_measurement_interference("hotkey-priority-stop");
                    if let Err(error) = runtime.stop(session) {
                        self.action_error(error.to_string());
                    }
                    return;
                }
                if let Err(error) = self.dispatch_toggle(process, _origin, StartOwner::Native) {
                    self.action_error(error);
                }
            }
        }
    }
    /// Shared productive host admission. Controlled callers may change only receipt ownership.
    fn dispatch_toggle(
        &self,
        process: Option<ProcessIdentity>,
        origin: hotkeys::NativeOrigin,
        owner: StartOwner,
    ) -> Result<(), String> {
        let action_ticket = self.begin_action()?;
        let measured_ticket = self.start_measurement_begin(origin);
        let permit = self.0.admissions.clone().try_acquire_owned().map_err(|_| {
            self.start_measurement_ack(measured_ticket, false);
            "Zu viele laufende Anfragen".to_owned()
        })?;
        let reply = self
            .start_request_tagged(process, true, true, measured_ticket, owner)
            .inspect_err(|_| self.start_measurement_ack(measured_ticket, false))?;
        let desktop = self.clone();
        tauri::async_runtime::spawn(async move {
            let _permit = permit;
            match reply.await {
                Ok(Ok(())) => {
                    desktop.start_measurement_ack(measured_ticket, true);
                    desktop.finish_action(&action_ticket, Ok(()));
                }
                Ok(Err(error)) => {
                    desktop.start_measurement_ack(measured_ticket, false);
                    desktop.finish_action(&action_ticket, Err(error));
                }
                Err(_) => {
                    desktop.start_measurement_ack(measured_ticket, false);
                    desktop.finish_action(&action_ticket, Err("Aufnahmeanfrage beendet".into()));
                }
            }
        });
        Ok(())
    }
    pub async fn save_settings(&self, draft: SettingsDraft) -> Result<(), String> {
        let _permit = self.ui_permit()?;
        self.enqueue_settings(draft)?
            .await
            .map_err(|_| "Einstellungsanfrage beendet".to_owned())?
    }
    fn enqueue_settings(
        &self,
        draft: SettingsDraft,
    ) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        let mut admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        if admission.changing_settings || admission.pending_starts != 0 {
            return Err("Eine Aufnahme oder Einstellungsänderung wird noch gestartet".into());
        }
        let current = {
            let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
            if shared.preferences.needs_restart {
                return Err(
                    "Bitte Anwendung neu starten, bevor Einstellungen geändert werden".into(),
                );
            }
            shared.settings.clone()
        };
        draft.clone().apply_to(&current).map_err(preference_error)?;
        let client = self.app_client()?;
        if client.refinement_busy().map_err(|e| e.to_string())? {
            return Err(
                "Bitte die Textmodelleinrichtung zuerst abschließen oder abbrechen.".into(),
            );
        }
        validate_refinement_enable(
            &current,
            &draft,
            &client.snapshot().map_err(|e| e.to_string())?,
        )?;
        let setup = client
            .snapshot()
            .map_err(|error| error.to_string())?
            .setup
            .phase;
        if matches!(
            setup,
            setup::Phase::Checking
                | setup::Phase::Downloading
                | setup::Phase::Loading
                | setup::Phase::Cancelling
        ) {
            return Err("Bitte zuerst die Modelleinrichtung abschließen".into());
        }
        if client
            .runtime()
            .map_err(|error| error.to_string())?
            .map(|runtime| runtime.snapshot().map_err(|error| error.to_string()))
            .transpose()?
            .is_some_and(|snapshot| snapshot.session.phase != Phase::Idle)
        {
            return Err("Einstellungen sind während eines Diktats gesperrt".into());
        }
        admission.changing_settings = true;
        let reservation = Reservation {
            owner: Arc::clone(&self.0),
            kind: ReservationKind::Settings,
        };
        drop(admission);
        let (reply, receive) = oneshot::channel();
        self.0
            .requests
            .try_send(SettingsCommand::Save {
                draft,
                reply,
                _reservation: reservation,
            })
            .map_err(|_| "Einstellungsanfrage konnte nicht angenommen werden".to_owned())?;
        Ok(receive)
    }
    pub async fn reset_settings(&self) -> Result<(), String> {
        let _permit = self.ui_permit()?;
        self.enqueue_reset()?
            .await
            .map_err(|_| "Zurücksetzen wurde beendet".to_owned())?
    }
    fn enqueue_reset(&self) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        let mut admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        if admission.changing_settings || admission.pending_starts != 0 {
            return Err("Eine Einstellungsanfrage läuft bereits".into());
        }
        if !self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .preferences
            .recovery_required
        {
            return Err(
                "Zurücksetzen ist nur bei nicht lesbaren Starteinstellungen möglich".into(),
            );
        }
        admission.changing_settings = true;
        let reservation = Reservation {
            owner: Arc::clone(&self.0),
            kind: ReservationKind::Settings,
        };
        drop(admission);
        let (reply, receive) = oneshot::channel();
        self.0
            .requests
            .try_send(SettingsCommand::Reset {
                reply,
                _reservation: reservation,
            })
            .map_err(|_| "Zurücksetzen konnte nicht angefragt werden".to_owned())?;
        Ok(receive)
    }
    pub async fn refresh_devices(&self) -> Result<(), String> {
        let _permit = self.ui_permit()?;
        self.enqueue_devices()?
            .await
            .map_err(|_| "Mikrofonanfrage beendet".to_owned())?
    }
    fn enqueue_devices(&self) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        let mut admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        if admission.refreshing_devices {
            return Err("Mikrofonliste wird bereits aktualisiert".into());
        }
        admission.refreshing_devices = true;
        let reservation = Reservation {
            owner: Arc::clone(&self.0),
            kind: ReservationKind::Devices,
        };
        drop(admission);
        let (reply, receive) = oneshot::channel();
        self.0
            .requests
            .try_send(SettingsCommand::Devices {
                reply,
                _reservation: reservation,
            })
            .map_err(|_| "Mikrofonliste konnte nicht angefragt werden".to_owned())?;
        Ok(receive)
    }
    fn refresh_device_inventory(&self) -> Result<(), String> {
        let result = crate::devices::input_devices();
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        match result {
            Ok(devices) => {
                shared.preferences.devices = devices;
                if !shared.preferences.needs_restart && !shared.preferences.recovery_required {
                    shared.preferences.error = None;
                }
                Ok(())
            }
            Err(error) => {
                shared.preferences.error = Some(error.clone());
                Err(error)
            }
        }
    }
    fn process_settings(
        &self,
        request: SettingsCommand,
        store: &PreferencesStore,
        hotkeys: &mut Option<Hotkeys>,
        app: &tauri::AppHandle,
    ) {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("settings-or-device-request");
        match request {
            SettingsCommand::Reset {
                reply,
                _reservation,
            } => {
                if reply
                    .send(Err("Starteinstellungen sind bereits gültig".into()))
                    .is_err()
                { /* caller gone */ }
            }
            SettingsCommand::Devices {
                reply,
                _reservation,
            } => {
                let result = self.refresh_device_inventory();
                if reply.send(result).is_err() { /* caller no longer waiting */ }
            }
            SettingsCommand::Save {
                draft,
                reply,
                _reservation,
            } => {
                let result = self.apply_preferences(draft, store, hotkeys, app);
                if reply.send(result).is_err() { /* caller no longer waiting */ }
            }
        }
    }
    fn apply_preferences(
        &self,
        draft: SettingsDraft,
        store: &PreferencesStore,
        hotkeys: &mut Option<Hotkeys>,
        app: &tauri::AppHandle,
    ) -> Result<(), String> {
        self.refresh_device_inventory()?;
        let current = {
            let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
            if let Some(selected) = &draft.device
                && !shared
                    .preferences
                    .devices
                    .iter()
                    .any(|device| device.name == *selected && device.available)
            {
                return Err(
                    "Das gewählte Mikrofon ist nicht verfügbar oder nicht eindeutig".into(),
                );
            }
            shared.settings.clone()
        };
        let mut binder = DesktopBinder {
            hotkeys,
            app,
            desktop: self.clone(),
            last_error: None,
        };
        validate_refinement_enable(
            &current,
            &draft,
            &self.app_client()?.snapshot().map_err(|e| e.to_string())?,
        )?;
        let result = store.commit(&current, draft, &mut binder);
        let native_error = binder.last_error.take();
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        match result {
            Ok(settings) => {
                shared.preferences.needs_restart |= current.model != settings.model;
                shared.settings = settings;
                shared.preferences.error = None;
                shared.action_errors.clear_transient();
                if binder.binding().is_some() {
                    shared.action_errors.persistent(None);
                }
                Ok(())
            }
            Err(error) => {
                let degraded = matches!(error, PreferencesError::Degraded { .. });
                let mut message = preference_error(error);
                if let Some(native) = native_error {
                    message.push_str(": ");
                    message.push_str(&native);
                }
                shared.preferences.error = Some(message.clone());
                shared.preferences.needs_restart |= degraded;
                drop(shared);
                if degraded
                    && let Some(hotkeys) = binder.hotkeys
                    && let Err(error) = hotkeys.shutdown()
                {
                    eprintln!("Shortcut cleanup after rollback failed: {error}");
                }
                Err(message)
            }
        }
    }
    fn close(&self) -> bool {
        #[cfg(feature = "measurement")]
        self.start_measurement_interference("shutdown");
        let first = {
            let _admission = self
                .0
                .admission
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            !self.0.closing.swap(true, Ordering::AcqRel)
        };
        if let Ok(shared) = self.0.shared.lock()
            && let Some(client) = &shared.client
            && let Err(error) = client.shutdown()
        {
            eprintln!("Application shutdown request failed: {error}");
        }
        first
    }
    pub fn shutdown(&self) {
        self.close();
    }
    pub fn begin_shutdown(&self) -> bool {
        self.close()
    }
    pub fn is_finished(&self) -> bool {
        self.0.finished.load(Ordering::Acquire)
    }
    /// Only from an exit worker while the GUI event loop can still process cleanup.
    pub fn join(&self) -> Result<(), String> {
        self.shutdown();
        let worker = self
            .0
            .worker
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .take();
        match worker {
            Some(worker) => worker.join().map_err(|_| "Desktop owner panicked")?,
            None => Ok(()),
        }
    }
    fn publish_recovery(
        &self,
        error: Option<&str>,
        publish: &mut impl FnMut(&view::Snapshot) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        shared.preferences.recovery_required = error.is_some();
        shared.preferences.error = error.map(str::to_owned);
        shared.view.preferences.recovery_required = error.is_some();
        shared.view.preferences.error = error.map(str::to_owned);
        shared.view.setup.phase = if error.is_some() {
            view::SetupPhase::Failed
        } else {
            view::SetupPhase::Checking
        };
        shared.view.setup.error = error.map(str::to_owned);
        shared.view.settings_busy = true;
        shared.view.revision = shared
            .view
            .revision
            .checked_add(1)
            .ok_or("Snapshot revision exhausted")?;
        publish(&shared.view)
    }
    fn await_startup_settings(
        &self,
        startup: &mut StartupSettings,
        receive: &Receiver<SettingsCommand>,
        mut publish: impl FnMut(&view::Snapshot) -> Result<(), String>,
    ) -> Result<Option<Settings>, String> {
        if let Some(settings) = startup.ready() {
            return Ok(Some(settings.clone()));
        }
        self.publish_recovery(startup.error(), &mut publish)?;
        while !self.0.closing.load(Ordering::Acquire) {
            match receive.recv_timeout(Duration::from_millis(50)) {
                Ok(SettingsCommand::Reset {
                    reply,
                    _reservation,
                }) => {
                    let result = startup.reset();
                    self.publish_recovery(startup.error(), &mut publish)?;
                    let success = result.is_ok();
                    if reply.send(result.map(|_| ())).is_err() { /* explicit request survives caller drop */
                    }
                    if success {
                        return Ok((!self.0.closing.load(Ordering::Acquire))
                            .then(|| startup.ready().cloned())
                            .flatten());
                    }
                }
                Ok(SettingsCommand::Save {
                    reply,
                    _reservation,
                    ..
                }) => {
                    if reply
                        .send(Err(
                            "Bitte zuerst die Starteinstellungen wiederherstellen".into()
                        ))
                        .is_err()
                    { /* caller gone */ }
                }
                Ok(SettingsCommand::Devices {
                    reply,
                    _reservation,
                }) => {
                    if reply
                        .send(Err(
                            "Bitte zuerst die Starteinstellungen wiederherstellen".into()
                        ))
                        .is_err()
                    { /* caller gone */ }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err("Einstellungsdienst beendet".into());
                }
            }
        }
        Ok(None)
    }
    /// Publish the owner before releasing admission, so shutdown either wins
    /// before construction or observes the newly admitted client's shutdown port.
    fn install_application(
        &self,
        settings: &Settings,
        create: impl FnOnce() -> Result<AppSupervisor, String>,
    ) -> Result<Option<AppSupervisor>, String> {
        let _admission = self
            .0
            .admission
            .lock()
            .map_err(|_| "Admission lock failed")?;
        if self.0.closing.load(Ordering::Acquire) {
            return Ok(None);
        }
        let supervisor = create()?;
        let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        shared.settings = settings.clone();
        shared.client = Some(supervisor.client());
        Ok(Some(supervisor))
    }
    fn run(&self, app: tauri::AppHandle, receive: Receiver<SettingsCommand>) -> Result<(), String> {
        let mut supervisor = None;
        let mut hotkeys = None;
        let result = (|| {
            if self.0.closing.load(Ordering::Acquire) {
                return Ok(());
            }
            let directory = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            let mut startup = StartupSettings::load(directory);
            let Some(settings) =
                self.await_startup_settings(&mut startup, &receive, |snapshot| {
                    app.emit(SNAPSHOT_EVENT, snapshot)
                        .map_err(|error| error.to_string())
                })?
            else {
                return Ok(());
            };
            supervisor = self.install_application(&settings, || {
                AppSupervisor::for_app(app.clone(), settings.model.definition())
                    .map_err(|error| error.to_string())
            })?;
            let Some(owner) = supervisor.as_ref() else {
                return Ok(());
            };
            let client = owner.client();
            let preference_store = PreferencesStore::new(
                app.path()
                    .app_data_dir()
                    .map_err(|error| error.to_string())?,
            );
            if let Err(error) = self.refresh_device_inventory() {
                self.action_error(error);
            }
            if self.0.closing.load(Ordering::Acquire) {
                return Ok(());
            }
            match self.install_hotkeys(app.clone(), settings.hotkey) {
                Ok(owner) => hotkeys = Some(owner),
                Err(error) => self.hotkey_error(format!("Tastenkürzel nicht verfügbar: {error}")),
            }
            while !self.0.closing.load(Ordering::Acquire) {
                let pending = receive.try_recv().ok();
                let application = client.snapshot().map_err(|error| error.to_string())?;
                let suspended = self.0.admission.lock().map_err(|_| "Power lock failed")?;
                let runtime = match client.runtime() {
                    Ok(runtime) => runtime,
                    Err(_) if self.0.closing.load(Ordering::Acquire) => break,
                    Err(error) => {
                        return Err(application
                            .error
                            .clone()
                            .unwrap_or_else(|| error.to_string()));
                    }
                };
                if suspended.suspended
                    && let Some(runtime) = &runtime
                {
                    runtime.suspend().map_err(|error| error.to_string())?;
                }
                if let Some(runtime) = &runtime {
                    runtime
                        .set_memory_pressure(suspended.memory_pressure)
                        .map_err(|e| e.to_string())?;
                }
                let runtime = runtime
                    .map(|runtime| runtime.snapshot())
                    .transpose()
                    .map_err(|error| error.to_string())?;
                #[cfg(feature = "measurement")]
                self.start_measurement(|o| {
                    o.observe(
                        runtime.as_ref().map(native_start_state),
                        std::time::Instant::now(),
                    )
                });
                let changing_settings = suspended.changing_settings;
                let refreshing_devices = suspended.refreshing_devices;
                drop(suspended);
                let active = runtime
                    .as_ref()
                    .filter(|runtime| runtime.session.phase != Phase::Idle)
                    .and_then(|runtime| runtime.session.session);
                if let Some(hotkeys) = &mut hotkeys {
                    if let Err(error) = hotkeys.enable_cancel(active) {
                        self.hotkey_error(error.to_string());
                    }
                    if let Some(error) = hotkeys.take_error().map_err(|error| error.to_string())? {
                        #[cfg(feature = "measurement")]
                        if self.0.native_starts_enabled {
                            self.0.native_starts_lost.store(true, Ordering::Release);
                            self.start_measurement_interference("native-hotkey-transport-error");
                        }
                        // Dispatch/backpressure errors are transient; registration errors
                        // arrive through install_hotkeys/enable_cancel instead.
                        self.action_error(error);
                    }
                }
                let native = self
                    .0
                    .refresh
                    .swap(false, Ordering::AcqRel)
                    .then(ui_services::permission_snapshot);
                let mut shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
                let mut next = project(
                    &application,
                    runtime.as_ref(),
                    &shared.settings,
                    shared.view.permissions.clone(),
                );
                next.settings_busy |= client.refinement_busy().map_err(|e| e.to_string())?;
                if let Some(native) = native {
                    next.permissions = permissions(native);
                }
                next.labels.hotkey =
                    crate::hotkey_labels::shortcut(&shared.settings.hotkey, &shared.key_labels);
                next.settings_request = shared.settings_requests;
                next.preferences = view::Preferences {
                    key_labels: crate::hotkey_labels::options(&shared.key_labels),
                    current: draft_of(&shared.settings),
                    devices: shared.preferences.devices.clone(),
                    error: shared.preferences.error.clone(),
                    refreshing: refreshing_devices,
                    needs_restart: shared.preferences.needs_restart,
                    recovery_required: shared.preferences.recovery_required,
                    languages: crate::preferences::SUPPORTED_LANGUAGES.to_vec(),
                };
                next.settings_busy |= changing_settings || shared.preferences.needs_restart;
                next.error = next.error.or_else(|| shared.action_errors.message());
                next.revision = shared.view.revision;
                if next != shared.view {
                    next.revision = next
                        .revision
                        .checked_add(1)
                        .ok_or("Snapshot revision exhausted")?;
                    shared.view = next;
                    app.emit(SNAPSHOT_EVENT, &shared.view)
                        .map_err(|error| error.to_string())?;
                }
                drop(shared);
                if let Some(pending) = pending {
                    self.process_settings(pending, &preference_store, &mut hotkeys, &app);
                }
                thread::sleep(Duration::from_millis(50));
            }
            Ok::<(), String>(())
        })();
        let hotkey_cleanup = match &mut hotkeys {
            Some(hotkeys) => hotkeys.shutdown().map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let cleaned = match supervisor {
            Some(supervisor) => supervisor.join().map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let result = finish_services(result, hotkey_cleanup, cleaned);
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.client = None;
            shared.view.running = false;
            shared.view.settings_busy = true;
            shared.view.revision = shared.view.revision.saturating_add(1);
            if let Err(error) = &result {
                shared.view.error = Some(error.clone());
                shared.view.setup.phase = view::SetupPhase::Failed;
                shared.view.setup.can_cancel = false;
                shared.view.settings_busy = false;
            }
            if let Err(error) = app.emit(SNAPSHOT_EVENT, &shared.view) {
                eprintln!("Final UI snapshot failed: {error}");
            }
        }
        result
    }
    pub fn ui_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        self.0
            .admissions
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Zu viele laufende Anfragen".into())
    }
    pub fn prompt_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        self.0
            .prompts
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Eine Berechtigungsanfrage läuft bereits".into())
    }
}
fn valid_id(id: u64) -> Result<(), String> {
    if id == 0 || id > 9_007_199_254_740_991 {
        Err("Ungültige Sitzungskennung".into())
    } else {
        Ok(())
    }
}
fn draft_of(settings: &Settings) -> SettingsDraft {
    SettingsDraft {
        refinement: settings.refinement,
        model: crate::preferences::PreferenceModel::from_settings(settings),
        device: settings.device.clone(),
        language: settings.language,
        hotkey: settings.hotkey.clone(),
    }
}
fn validate_refinement_enable(
    current: &Settings,
    draft: &SettingsDraft,
    application: &crate::application::Snapshot,
) -> Result<(), String> {
    if draft.refinement
        && !current.refinement
        && !application
            .refinement_setup
            .as_ref()
            .is_some_and(|s| s.phase == setup::Phase::Ready)
    {
        return Err("Bitte zuerst das Textmodell vollständig einrichten.".into());
    }
    Ok(())
}
fn forward_start(
    reply: oneshot::Receiver<Result<(), String>>,
    reservation: Reservation,
) -> oneshot::Receiver<Result<(), String>> {
    let (send, receive) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let result = reply
            .await
            .unwrap_or_else(|_| Err("Aufnahmeanfrage beendet".into()));
        // The Runtime publishes its state before the ACK. Caller cancellation
        // cannot release this reservation while a queued start still exists.
        drop(reservation);
        if send.send(result).is_err() { /* caller no longer waiting */ }
    });
    receive
}
struct DesktopBinder<'a> {
    hotkeys: &'a mut Option<Hotkeys>,
    app: &'a tauri::AppHandle,
    desktop: Desktop,
    last_error: Option<String>,
}
impl HotkeyRebinder for DesktopBinder<'_> {
    type Error = String;
    fn binding(&self) -> Option<crate::settings::Hotkey> {
        self.hotkeys.as_ref().and_then(Hotkeys::current_binding)
    }
    fn rebind(&mut self, requested: Option<&crate::settings::Hotkey>) -> Result<(), String> {
        let result = match (requested, self.hotkeys.as_mut()) {
            (Some(key), Some(owner)) => owner
                .reconfigure(key.clone())
                .map_err(|error| error.to_string()),
            (Some(key), None) => self
                .desktop
                .install_hotkeys(self.app.clone(), key.clone())
                .map(|owner| *self.hotkeys = Some(owner))
                .map_err(|error| error.to_string()),
            (None, Some(owner)) => owner
                .shutdown()
                .map(|()| *self.hotkeys = None)
                .map_err(|error| error.to_string()),
            (None, None) => Ok(()),
        };
        if let Err(error) = &result {
            self.last_error = Some(error.clone());
        }
        result
    }
}
fn preference_error(error: PreferencesError) -> String {
    match error {
        PreferencesError::ModelUnavailable => "Dieses Modell ist in diesem Build nicht verfügbar".into(),
        PreferencesError::LanguageUnavailable => "Diese Sprache ist noch nicht freigegeben".into(),
        PreferencesError::Invalid(_) => "Mikrofon oder Tastenkürzel ist ungültig".into(),
        PreferencesError::Registration => "Tastenkürzel konnte nicht registriert werden; vorherige Einstellung bleibt".into(),
        PreferencesError::Storage => "Einstellungen konnten nicht gespeichert werden; vorherige Einstellung bleibt".into(),
        PreferencesError::Stale => "Die Einstellungsdatei wurde geändert; bitte Anwendung neu starten".into(),
        PreferencesError::Unreadable => "Die Einstellungsdatei konnte nicht sicher gelesen werden".into(),
        PreferencesError::Degraded { .. } => "Einstellungen konnten nicht sicher wiederhergestellt werden. Tastenkürzel sind deaktiviert; bitte Anwendung neu starten".into(),
    }
}
fn permissions(native: ui_services::PermissionSnapshot) -> view::Permissions {
    view::Permissions {
        microphone: match native.microphone {
            MicAuthorization::Granted => view::PermissionStatus::Granted,
            MicAuthorization::Denied => view::PermissionStatus::Denied,
            MicAuthorization::Restricted => view::PermissionStatus::Restricted,
            MicAuthorization::NotDetermined => view::PermissionStatus::NotDetermined,
            MicAuthorization::Unknown => view::PermissionStatus::Unavailable,
        },
        accessibility: if native.accessibility_trusted {
            view::PermissionStatus::Granted
        } else {
            view::PermissionStatus::Denied
        },
    }
}
/// Legacy IPC field names expose the complete setup inventory and provenance.
/// The locally derived payload's actual transport URL stays empty.
fn model_view(definition: crate::model::ModelDefinition) -> view::Model {
    view::Model {
        name: definition.id().into(),
        download_bytes: definition.installation_bytes().min(u64::from(u32::MAX)) as u32,
        source_url: if definition.is_reference_encoder() {
            crate::model::REFERENCE.url()
        } else {
            definition.url()
        },
    }
}
fn initial() -> view::Snapshot {
    let settings = Settings::default();
    let definition = settings.model.definition();
    view::Snapshot {
        refinement: view::Refinement {
            setup: view::RefinementSetup {
                available: false,
                phase: view::SetupPhase::Missing,
                downloaded_bytes: 0,
                total_bytes: 0,
                can_cancel: false,
                error: None,
            },
            session_id: None,
            original: None,
            status: view::RefinementStatus::Disabled,
            message: None,
        },
        revision: 1,
        running: true,
        setup: view::Setup {
            phase: view::SetupPhase::Checking,
            model: model_view(definition),
            downloaded_bytes: 0,
            total_bytes: definition.installation_bytes().min(u64::from(u32::MAX)) as u32,
            can_cancel: false,
            error: None,
        },
        session: view::Session {
            phase: view::SessionPhase::Idle,
            id: None,
            text: String::new(),
            outcome: None,
            maximum_warning: false,
            maximum_reached: false,
            last_held_session: None,
            elapsed_millis: 0,
            level_permille: 0,
        },
        permissions: view::Permissions {
            microphone: view::PermissionStatus::Unavailable,
            accessibility: view::PermissionStatus::Unavailable,
        },
        labels: view::Labels {
            device: "Standardmikrofon".into(),
            language: "Deutsch".into(),
            hotkey: crate::hotkey_labels::shortcut(&settings.hotkey, &Default::default()),
        },
        settings_busy: true,
        settings_request: 0,
        preferences: view::Preferences {
            key_labels: crate::hotkey_labels::options(&Default::default()),
            current: draft_of(&settings),
            devices: Vec::new(),
            error: None,
            refreshing: false,
            needs_restart: false,
            recovery_required: false,
            languages: crate::preferences::SUPPORTED_LANGUAGES.to_vec(),
        },
        error: None,
    }
}
fn project(
    application: &crate::application::Snapshot,
    runtime: Option<&crate::runtime::RuntimeSnapshot>,
    settings: &Settings,
    permissions: view::Permissions,
) -> view::Snapshot {
    let mut result = initial();
    if let Some(setup) = &application.refinement_setup {
        result.refinement.setup = view::RefinementSetup {
            available: true,
            phase: setup_phase(setup.phase),
            downloaded_bytes: setup.received.min(u64::from(u32::MAX)) as u32,
            total_bytes: setup.total.min(u64::from(u32::MAX)) as u32,
            can_cancel: matches!(setup.phase, setup::Phase::Checking | setup::Phase::Downloading | setup::Phase::Loading),
            error: setup.failure.as_ref().map(|_| "Das Textmodell konnte nicht eingerichtet werden. Prüfe Verbindung und freien Speicherplatz und versuche es erneut.".into()),
        };
    }
    result.permissions = permissions;
    result.running = application.running && !application.closing;
    result.setup.phase = match application.setup.phase {
        setup::Phase::Missing => view::SetupPhase::Missing,
        setup::Phase::Checking => view::SetupPhase::Checking,
        setup::Phase::Downloading => view::SetupPhase::Downloading,
        setup::Phase::Loading => view::SetupPhase::Loading,
        setup::Phase::Cancelling => view::SetupPhase::Cancelling,
        setup::Phase::Cancelled => view::SetupPhase::Cancelled,
        setup::Phase::Ready if runtime.is_some_and(|runtime| runtime.model_ready) => {
            view::SetupPhase::Ready
        }
        setup::Phase::Ready => view::SetupPhase::Loading,
        setup::Phase::Failed => view::SetupPhase::Failed,
    };
    let definition = application.setup.model;
    result.setup.model = model_view(definition);
    result.setup.downloaded_bytes = application.setup.received.min(u64::from(u32::MAX)) as u32;
    result.setup.total_bytes = application.setup.total.min(u64::from(u32::MAX)) as u32;
    result.setup.can_cancel = matches!(
        application.setup.phase,
        setup::Phase::Checking | setup::Phase::Downloading | setup::Phase::Loading
    );
    result.setup.error = application.setup.failure.as_ref().map(|failure| {
        match failure {
            setup::Failure::Busy => "Das Modell wird bereits von einer anderen Einrichtung verwendet. Bitte warte und versuche es erneut.",
            setup::Failure::Permission => "Das Modellverzeichnis ist nicht zugänglich. Bitte prüfe die Dateirechte.",
            setup::Failure::DiskFull => "Es ist nicht genügend Speicherplatz für das Modell frei.",
            setup::Failure::Io => "Die Modelldatei konnte nicht gelesen oder gespeichert werden.",
            setup::Failure::Size | setup::Failure::Hash => "Die Modelldatei ist unvollständig oder beschädigt. Bitte lade sie erneut.",
            setup::Failure::Network => "Die Netzwerkverbindung zur Modellquelle ist fehlgeschlagen. Bitte prüfe deine Verbindung.",
            setup::Failure::Tls => "Die TLS-Verbindung zur Modellquelle konnte nicht sicher hergestellt werden. Bitte versuche es später erneut.",
            setup::Failure::Timeout => "Der Modelldownload hat das Zeitlimit überschritten. Bitte versuche es erneut.",
            setup::Failure::Redirect => "Die Modellquelle hat auf ein nicht zugelassenes Ziel verwiesen.",
            setup::Failure::Http { .. } => "Die Modellquelle ist momentan nicht verfügbar. Bitte versuche es später erneut.",
            setup::Failure::Initialization => "Die lokale Spracherkennung konnte nicht gestartet werden. Bitte versuche es erneut.",
            setup::Failure::Worker => "Die Modelleinrichtung wurde unerwartet beendet.",
        }.to_owned()
    });
    result.error = application.error.clone();
    result.settings_busy = matches!(
        application.setup.phase,
        setup::Phase::Checking
            | setup::Phase::Downloading
            | setup::Phase::Loading
            | setup::Phase::Cancelling
    );
    result.labels.device = settings
        .device
        .clone()
        .unwrap_or_else(|| "Standardmikrofon".into());
    result.labels.language = match settings.language {
        crate::settings::SettingsLanguage::German => "Deutsch",
        crate::settings::SettingsLanguage::English => "English",
    }
    .into();
    result.labels.hotkey = crate::hotkey_labels::shortcut(&settings.hotkey, &Default::default());
    if let Some(runtime) = runtime {
        use crate::controller::RefinementStatus as R;
        result.refinement.session_id = runtime.session.refinement.session;
        result.refinement.original = runtime.session.refinement.original.clone();
        result.refinement.status = match &runtime.session.refinement.status {
            R::Disabled => view::RefinementStatus::Disabled,
            R::Processing => view::RefinementStatus::Processing,
            R::Skipped(_) => view::RefinementStatus::Skipped,
            R::CheckedUnchanged => view::RefinementStatus::Unchanged,
            R::Edited => view::RefinementStatus::Edited,
            R::Failed(_) => view::RefinementStatus::Failed,
            R::HeldOriginal(_) => view::RefinementStatus::Held,
            R::Cancelled => view::RefinementStatus::Cancelled,
        };
        result.refinement.message = match &runtime.session.refinement.status {
            R::Failed(_) | R::HeldOriginal(_) => Some("Die Textbereinigung konnte nicht abgeschlossen werden. Das Original bleibt erhalten; es wurde nichts eingefügt.".into()),
            R::Cancelled => Some("Textbereinigung abgebrochen. Das Original bleibt erhalten; es wurde nichts eingefügt.".into()),
            _ => None,
        };
        result.session.phase = match runtime.session.phase {
            Phase::Idle => view::SessionPhase::Idle,
            Phase::Starting => view::SessionPhase::Starting,
            Phase::Recording => view::SessionPhase::Recording,
            Phase::Finalizing => view::SessionPhase::Finalizing,
            Phase::Delivering => view::SessionPhase::Delivering,
            Phase::Cancelling => view::SessionPhase::Cancelling,
        };
        result.session.id = runtime.session.session;
        result.session.text = runtime.session.text.clone().unwrap_or_default();
        result.session.maximum_warning = runtime.maximum_warning;
        result.session.maximum_reached = runtime.maximum_reached;
        result.session.last_held_session = runtime.session.last_held_session;
        result.session.elapsed_millis = runtime.elapsed_millis;
        result.session.level_permille = runtime.level_permille;
        result.session.outcome = runtime
            .session
            .outcome
            .as_ref()
            .map(|outcome| match outcome {
                Outcome::Local => view::SessionOutcome::Local,
                Outcome::Empty => view::SessionOutcome::Empty,
                Outcome::Sent => view::SessionOutcome::Sent,
                Outcome::Cancelled => view::SessionOutcome::Cancelled,
                Outcome::Held(message) => view::SessionOutcome::Held {
                    message: message.clone(),
                },
                Outcome::Failed(message) => view::SessionOutcome::Failed {
                    message: message.clone(),
                },
            });
        result.error = runtime.error.clone().or(result.error);
        result.settings_busy |= runtime.session.phase != Phase::Idle;
    }
    result
}

fn setup_phase(phase: setup::Phase) -> view::SetupPhase {
    match phase {
        setup::Phase::Missing => view::SetupPhase::Missing,
        setup::Phase::Checking => view::SetupPhase::Checking,
        setup::Phase::Downloading => view::SetupPhase::Downloading,
        setup::Phase::Loading => view::SetupPhase::Loading,
        setup::Phase::Cancelling => view::SetupPhase::Cancelling,
        setup::Phase::Cancelled => view::SetupPhase::Cancelled,
        setup::Phase::Ready => view::SetupPhase::Ready,
        setup::Phase::Failed => view::SetupPhase::Failed,
    }
}

#[cfg(feature = "measurement")]
fn native_start_state(s: &crate::runtime::RuntimeSnapshot) -> start_observer::State {
    start_observer::State {
        phase: match s.session.phase {
            Phase::Idle => start_observer::Phase::Idle,
            Phase::Starting => start_observer::Phase::Starting,
            Phase::Recording => start_observer::Phase::Recording,
            _ => start_observer::Phase::Other,
        },
        sid: s.session.session,
        ready: s.model_ready,
    }
}

/// Preserve the initiating failure and distinct cleanup causes. A generic Closed
/// observation must not erase the owner failure discovered while joining it.
fn finish_services(
    result: Result<(), String>,
    hotkeys: Result<(), String>,
    application: Result<(), String>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for error in [result, hotkeys, application]
        .into_iter()
        .filter_map(Result::err)
    {
        if !errors.contains(&error) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

struct Finish(Arc<Inner>);
impl Drop for Finish {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    mod settings_ack_contract;
    use super::*;
    use crate::{controller::SessionSnapshot, model::REFERENCE, runtime::RuntimeSnapshot};
    pub(super) fn unstarted_desktop() -> Desktop {
        let (requests, _) = bounded(8);
        Desktop(Arc::new(Inner {
            #[cfg(feature = "test-control")]
            test_control: test_control::State::default(),
            shared: Mutex::new(Shared {
                view: initial(),
                client: None,
                settings: Settings::default(),
                action_errors: Default::default(),
                settings_requests: 0,
                key_labels: Default::default(),
                preferences: PreferenceState::default(),
            }),
            closing: AtomicBool::new(false),
            refresh: AtomicBool::new(false),
            worker: Mutex::new(None),
            finished: AtomicBool::new(false),
            admissions: Arc::new(Semaphore::new(16)),
            prompts: Arc::new(Semaphore::new(1)),
            admission: Mutex::new(Admission::default()),
            requests,
            #[cfg(feature = "measurement")]
            native_starts: Mutex::new(start_observer::Observer::default()),
            #[cfg(feature = "measurement")]
            native_starts_enabled: false,
            #[cfg(feature = "measurement")]
            native_starts_lost: AtomicBool::new(false),
        }))
    }
    #[test]
    fn transport_failures_clear_on_retry_but_registration_faults_remain()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        desktop.action_error("Hotkey main-thread request still outstanding".into());
        let retry = desktop.begin_action()?;
        desktop.finish_action(&retry, Ok(()));
        assert!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock")?
                .action_errors
                .message()
                .is_none()
        );
        desktop.hotkey_error("Registration unavailable".into());
        let retry = desktop.begin_action()?;
        desktop.finish_action(&retry, Ok(()));
        assert_eq!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock")?
                .action_errors
                .message()
                .as_deref(),
            Some("Registration unavailable")
        );
        Ok(())
    }

    #[test]
    fn service_completion_preserves_owner_failure_after_generic_closed() {
        let error = finish_services(
            Err("Die Anwendung wird beendet".into()),
            Err("Hotkey cleanup failed".into()),
            Err("Native delivery worker panicked".into()),
        );
        assert_eq!(error, Err("Die Anwendung wird beendet; Hotkey cleanup failed; Native delivery worker panicked".into()));
        assert_eq!(finish_services(Ok(()), Ok(()), Ok(())), Ok(()));
        assert_eq!(
            finish_services(Err("same".into()), Ok(()), Err("same".into())),
            Err("same".into())
        );
    }

    #[test]
    fn initial_model_disclosure_counts_the_complete_installation() {
        let view = initial();
        let definition = Settings::default().model.definition();
        let total = definition.installation_bytes().min(u64::from(u32::MAX)) as u32;
        assert_eq!(view.setup.model.download_bytes, total);
        assert_eq!(view.setup.total_bytes, total);
    }

    #[cfg(feature = "mlx")]
    #[test]
    fn derived_model_disclosure_uses_the_verified_origin_and_installation_inventory() {
        let definition = crate::model::MLX_REFERENCE_COEFFICIENTS;
        let settings = Settings {
            model: crate::settings::ModelChoice::MlxReferenceEncoder,
            ..Settings::default()
        };
        let application = crate::application::Snapshot {
            refinement_setup: None,
            revision: 1,
            running: true,
            closing: false,
            setup: setup::Snapshot {
                revision: 1,
                phase: setup::Phase::Missing,
                model: definition,
                received: 17,
                total: definition.installation_bytes(),
                failure: None,
            },
            runtime_available: false,
            error: None,
        };
        let view = project(&application, None, &settings, initial().permissions);
        assert!(
            definition.url().is_empty(),
            "the local payload has no download endpoint"
        );
        assert_eq!(view.setup.model.download_bytes, 3_813_494_037);
        assert_eq!(view.setup.model.source_url, REFERENCE.url());
        assert_eq!(view.setup.total_bytes, 3_813_494_037);
        assert_eq!(view.setup.downloaded_bytes, 17);
    }

    #[test]
    fn sleep_before_runtime_handoff_blocks_start_and_wake_only_refreshes() {
        let desktop = unstarted_desktop();
        assert!(desktop.set_suspended(true).is_ok());
        assert!(desktop.set_suspended(true).is_ok());
        assert!(matches!(desktop.start_request(None, false, false),
            Err(message) if message.contains("Ruhezustands")));
        assert!(desktop.set_suspended(false).is_ok());
        assert!(desktop.0.refresh.load(Ordering::Acquire));
        assert!(matches!(desktop.start_request(None, false, false),
            Err(message) if message == "Anwendung wird eingerichtet"));
        assert!(
            desktop
                .0
                .shared
                .lock()
                .is_ok_and(|shared| shared.client.is_none())
        );
    }
    #[test]
    fn wake_after_shutdown_cannot_reopen_start_admission() {
        let desktop = unstarted_desktop();
        desktop.shutdown();
        assert!(desktop.set_suspended(false).is_ok());
        assert!(matches!(desktop.start_request(None, false, false),
            Err(message) if message == "Anwendung wird beendet"));
    }
    #[test]
    fn native_settings_requests_remain_available_while_setup_is_busy_and_respect_shutdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        for (index, phase) in [
            view::SetupPhase::Checking,
            view::SetupPhase::Loading,
            view::SetupPhase::Downloading,
        ]
        .into_iter()
        .enumerate()
        {
            {
                let mut shared = desktop.0.shared.lock().map_err(|_| "lock")?;
                shared.view.setup.phase = phase;
                assert!(shared.view.settings_busy);
            }
            assert_eq!(desktop.request_settings()?, u32::try_from(index + 1)?);
            let (_, header) = desktop.native_header_after(None)?.ok_or("missing header")?;
            assert!(header.settings_enabled, "navigation must remain available");
            assert!(
                header.primary.is_none(),
                "busy setup must never enable capture"
            );
        }
        desktop.shutdown();
        assert!(desktop.request_settings().is_err());
        Ok(())
    }
    #[test]
    fn native_header_freezes_actions_to_the_displayed_session()
    -> Result<(), Box<dyn std::error::Error>> {
        use macos_integration::glass_header::{HeaderPrimary, HeaderState};
        let mut view = initial();
        view.setup.phase = view::SetupPhase::Ready;
        view.permissions.microphone = view::PermissionStatus::Granted;
        view.settings_busy = false;
        let ready = crate::header::project(&view);
        assert_eq!(ready.primary, Some(HeaderPrimary::Start));
        assert!(ready.settings_enabled);
        view.session.phase = view::SessionPhase::Recording;
        view.session.id = Some(7);
        view.settings_busy = true;
        let recording = crate::header::project(&view);
        assert_eq!(recording.primary, Some(HeaderPrimary::Stop(7.try_into()?)));
        assert_eq!(recording.cancel, 7.try_into().ok());
        assert!(recording.settings_enabled);
        view.session.id = Some(8);
        assert_ne!(recording, crate::header::project(&view));
        assert_ne!(recording, HeaderState::default());
        Ok(())
    }
    #[test]
    fn native_header_never_promises_capture_before_readiness_or_after_shutdown() {
        let mut view = initial();
        assert!(crate::header::project(&view).primary.is_none());
        view.setup.phase = view::SetupPhase::Ready;
        view.settings_busy = false;
        assert!(crate::header::project(&view).primary.is_none());
        view.permissions.microphone = view::PermissionStatus::Granted;
        assert!(crate::header::project(&view).primary.is_some());
        view.session.phase = view::SessionPhase::Recording;
        view.session.id = Some(0);
        let missing = crate::header::project(&view);
        assert!(missing.primary.is_none() && missing.cancel.is_none());
        view.session.id = Some(7);
        view.running = false;
        let stopped = crate::header::project(&view);
        assert!(stopped.primary.is_none() && stopped.cancel.is_none());
        assert!(!stopped.settings_enabled);
    }
    #[test]
    fn native_header_reads_only_changed_revisions_and_no_transcript_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        let (revision, state) = desktop.native_header_after(None)?.ok_or("missing header")?;
        assert_eq!(state.status, "Modell wird geprüft");
        assert!(desktop.native_header_after(Some(revision))?.is_none());
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock")?;
            shared.view.revision += 1;
            shared.view.session.text = "private dictated text".into();
            shared.view.session.phase = view::SessionPhase::Recording;
            shared.view.session.id = Some(2);
            shared.view.session.maximum_warning = true;
        }
        let (_, state) = desktop
            .native_header_after(Some(revision))?
            .ok_or("missing update")?;
        assert_eq!(state.status, "Aufnahme endet bei 5 Minuten");
        assert!(!state.status.contains("private"));
        Ok(())
    }
    #[test]
    fn native_settings_counter_exhaustion_is_explicit_and_does_not_wrap()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock")?;
            shared.view.settings_busy = false;
            shared.settings_requests = u32::MAX;
        }
        assert!(desktop.request_settings().is_err());
        assert_eq!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock")?
                .settings_requests,
            u32::MAX
        );
        Ok(())
    }
    fn application() -> crate::application::Snapshot {
        crate::application::Snapshot {
            refinement_setup: None,
            revision: 1,
            running: true,
            closing: false,
            setup: setup::Snapshot {
                revision: 2,
                phase: setup::Phase::Ready,
                model: REFERENCE,
                received: REFERENCE.bytes(),
                total: REFERENCE.bytes(),
                failure: None,
            },
            runtime_available: false,
            error: None,
        }
    }
    #[test]
    fn enabling_refinement_requires_ready_assets_but_disabling_remains_available() {
        let current = Settings::default();
        let mut draft = draft_of(&current);
        draft.refinement = true;
        let mut application = application();
        assert!(validate_refinement_enable(&current, &draft, &application).is_err());
        for phase in [
            setup::Phase::Missing,
            setup::Phase::Checking,
            setup::Phase::Failed,
            setup::Phase::Ready,
        ] {
            application.refinement_setup = Some(crate::refinement_setup::Snapshot {
                revision: 1,
                phase,
                received: 0,
                total: 1,
                failure: None,
            });
            assert_eq!(
                validate_refinement_enable(&current, &draft, &application).is_ok(),
                phase == setup::Phase::Ready
            );
        }
        draft.refinement = false;
        application.refinement_setup = None;
        assert!(validate_refinement_enable(&current, &draft, &application).is_ok());
    }
    #[test]
    fn refinement_projection_exposes_setup_and_retained_original()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut application = application();
        application.refinement_setup = Some(crate::refinement_setup::Snapshot {
            revision: 3,
            phase: setup::Phase::Ready,
            received: 2_278_970_666,
            total: 2_278_970_666,
            failure: None,
        });
        let runtime = RuntimeSnapshot {
            revision: 5,
            running: true,
            model_ready: true,
            session: SessionSnapshot {
                phase: Phase::Finalizing,
                session: Some(8),
                last_held_session: None,
                text: Some("Das ist, äh, ein Test.".into()),
                outcome: None,
                refinement: crate::controller::RefinementSnapshot {
                    session: Some(8),
                    original: Some("Das ist, äh, ein Test.".into()),
                    status: crate::controller::RefinementStatus::Processing,
                },
            },
            maximum_warning: false,
            maximum_reached: false,
            elapsed_millis: 0,
            level_permille: 0,
            error: None,
        };
        let value = serde_json::to_value(project(
            &application,
            Some(&runtime),
            &Settings::default(),
            initial().permissions,
        ))?;
        assert_eq!(value["refinement"]["status"], "Processing");
        assert_eq!(value["refinement"]["original"], "Das ist, äh, ein Test.");
        assert_eq!(value["refinement"]["sessionId"], 8);
        assert_eq!(value["refinement"]["setup"]["phase"], "Ready");
        Ok(())
    }
    #[test]
    fn model_loaded_without_runtime_is_not_ready_to_record() {
        let result = project(
            &application(),
            None,
            &Settings::default(),
            initial().permissions,
        );
        assert_eq!(result.setup.phase, view::SetupPhase::Loading);
        assert_eq!(result.session.phase, view::SessionPhase::Idle);
        assert_eq!(result.session.id, None);
    }
    #[test]
    fn projection_preserves_previous_text_and_disables_settings_during_session() {
        let runtime = RuntimeSnapshot {
            revision: 3,
            running: true,
            model_ready: true,
            session: SessionSnapshot {
                refinement: Default::default(),
                phase: Phase::Recording,
                session: Some(7),
                text: Some("previous words".into()),
                outcome: None,
                last_held_session: None,
            },
            maximum_reached: false,
            maximum_warning: true,
            elapsed_millis: 0,
            level_permille: 0,
            error: None,
        };
        let result = project(
            &application(),
            Some(&runtime),
            &Settings::default(),
            initial().permissions,
        );
        assert_eq!(result.setup.phase, view::SetupPhase::Ready);
        assert_eq!(result.session.id, Some(7));
        assert_eq!(result.session.text, "previous words");
        assert!(result.session.outcome.is_none());
        assert!(result.settings_busy);
        assert!(result.session.maximum_warning);
    }
    #[test]
    fn idle_held_result_keeps_its_text_and_reason() {
        let runtime = RuntimeSnapshot {
            revision: 3,
            running: true,
            model_ready: true,
            session: SessionSnapshot {
                refinement: Default::default(),
                phase: Phase::Idle,
                session: None,
                text: Some("retained words".into()),
                outcome: Some(Outcome::Held("Original field changed".into())),
                last_held_session: Some(7),
            },
            maximum_reached: false,
            maximum_warning: false,
            elapsed_millis: 0,
            level_permille: 0,
            error: None,
        };
        let result = project(
            &application(),
            Some(&runtime),
            &Settings::default(),
            initial().permissions,
        );
        assert_eq!(result.session.text, "retained words");
        assert!(matches!(result.session.outcome,
            Some(view::SessionOutcome::Held { message }) if message == "Original field changed"));
        assert!(!result.settings_busy);
        assert_eq!(result.session.last_held_session, Some(7));
    }
    #[test]
    fn ipc_session_ids_are_positive_exact_javascript_integers() {
        assert!(valid_id(0).is_err());
        assert!(valid_id(9_007_199_254_740_992).is_err());
        assert!(valid_id(1).is_ok());
        assert!(valid_id(9_007_199_254_740_991).is_ok());
    }

    fn start_reservation(desktop: &Desktop) -> Result<Reservation, String> {
        desktop
            .0
            .admission
            .lock()
            .map_err(|_| "admission lock")?
            .pending_starts += 1;
        Ok(Reservation {
            owner: Arc::clone(&desktop.0),
            kind: ReservationKind::Start,
        })
    }

    fn await_start_release(desktop: &Desktop) -> Result<(), String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while desktop
            .0
            .admission
            .lock()
            .map_err(|_| "admission lock")?
            .pending_starts
            != 0
        {
            if std::time::Instant::now() >= deadline {
                return Err("start reservation was not released".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn dropped_start_caller_retains_reservation_until_real_ack()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        let (complete, pending) = oneshot::channel();
        let caller = forward_start(pending, start_reservation(&desktop)?);
        drop(caller);
        assert!(
            matches!(desktop.enqueue_settings(draft_of(&Settings::default())), Err(message) if message == "Eine Aufnahme oder Einstellungsänderung wird noch gestartet")
        );
        // Give the detached forwarding task time to observe the closed caller.
        // A closed UI receiver must not complete the still-pending runtime request.
        let deadline = std::time::Instant::now() + Duration::from_millis(25);
        while std::time::Instant::now() < deadline {
            assert_eq!(
                desktop
                    .0
                    .admission
                    .lock()
                    .map_err(|_| "admission lock")?
                    .pending_starts,
                1
            );
            assert!(!complete.is_closed());
            thread::sleep(Duration::from_millis(1));
        }
        assert!(complete.send(Ok(())).is_ok());
        await_start_release(&desktop)?;
        assert!(
            matches!(desktop.enqueue_settings(draft_of(&Settings::default())), Err(message) if message == "Anwendung wird eingerichtet")
        );
        Ok(())
    }

    #[test]
    fn failed_or_closed_real_ack_releases_start_and_forwards_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        for closed in [false, true] {
            let desktop = unstarted_desktop();
            let (complete, pending) = oneshot::channel();
            let caller = forward_start(pending, start_reservation(&desktop)?);
            if closed {
                drop(complete);
            } else {
                assert!(complete.send(Err("start rejected".into())).is_ok());
            }
            let result = tauri::async_runtime::block_on(caller)?;
            assert_eq!(
                result,
                Err(if closed {
                    "Aufnahmeanfrage beendet"
                } else {
                    "start rejected"
                }
                .into())
            );
            assert_eq!(
                desktop
                    .0
                    .admission
                    .lock()
                    .map_err(|_| "admission lock")?
                    .pending_starts,
                0
            );
        }
        Ok(())
    }

    struct SettingsQueueFixture {
        desktop: Desktop,
        requests: Receiver<SettingsCommand>,
        supervisor: AppSupervisor,
        _directory: tempfile::TempDir,
    }

    fn desktop_with_settings_queue() -> Result<SettingsQueueFixture, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let supervisor =
            AppSupervisor::spawn(directory.path().join("no-models"), REFERENCE, |_| {
                Err("empty-cache fixture must never start native services".into())
            })?;
        let client = supervisor.client();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while {
            let snapshot = client.snapshot()?;
            snapshot.setup.phase != setup::Phase::Missing || snapshot.revision <= 1
        } {
            if std::time::Instant::now() >= deadline {
                return Err("empty cache check did not complete".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        let mut desktop = unstarted_desktop();
        let (requests, receive) = bounded(8);
        Arc::get_mut(&mut desktop.0)
            .ok_or("fixture unexpectedly shared")?
            .requests = requests;
        desktop.0.shared.lock().map_err(|_| "desktop lock")?.client = Some(client);
        Ok(SettingsQueueFixture {
            desktop,
            requests: receive,
            supervisor,
            _directory: directory,
        })
    }

    #[test]
    fn settings_request_survives_caller_drop_and_blocks_start_until_owner_finishes()
    -> Result<(), Box<dyn std::error::Error>> {
        let SettingsQueueFixture {
            desktop,
            requests,
            supervisor,
            _directory,
        } = desktop_with_settings_queue()?;
        let caller = desktop.enqueue_settings(draft_of(&Settings::default()))?;
        drop(caller);
        assert!(
            matches!(desktop.start_request(None, false, false), Err(message) if message.contains("Einstellungen werden geändert"))
        );
        assert!(
            desktop
                .enqueue_settings(draft_of(&Settings::default()))
                .is_err()
        );
        let SettingsCommand::Save {
            reply,
            _reservation,
            ..
        } = requests.recv_timeout(Duration::from_secs(1))?
        else {
            return Err("expected settings request".into());
        };
        assert!(
            desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        assert!(reply.send(Ok(())).is_err());
        // The owner, not delivery of its acknowledgement, releases the reservation.
        assert!(
            desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        drop(_reservation);
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        assert!(
            matches!(desktop.start_request(None, false, false), Err(message) if message == "Spracherkennung ist noch nicht bereit")
        );
        supervisor.join()?;
        Ok(())
    }

    #[test]
    fn disconnected_settings_queue_releases_the_settings_flag()
    -> Result<(), Box<dyn std::error::Error>> {
        let SettingsQueueFixture {
            desktop,
            requests,
            supervisor,
            _directory,
        } = desktop_with_settings_queue()?;
        drop(requests);
        for _ in 0..2 {
            assert!(
                matches!(desktop.enqueue_settings(draft_of(&Settings::default())), Err(message) if message == "Einstellungsanfrage konnte nicht angenommen werden")
            );
            assert!(
                !desktop
                    .0
                    .admission
                    .lock()
                    .map_err(|_| "admission lock")?
                    .changing_settings
            );
        }
        supervisor.join()?;
        Ok(())
    }

    fn recovery_desktop(capacity: usize) -> Result<(Desktop, Receiver<SettingsCommand>), String> {
        let mut desktop = unstarted_desktop();
        let (requests, receive) = bounded(capacity);
        Arc::get_mut(&mut desktop.0)
            .ok_or("fixture unexpectedly shared")?
            .requests = requests;
        Ok((desktop, receive))
    }

    #[test]
    fn healthy_startup_rejects_reset_without_queuing_or_reserving()
    -> Result<(), Box<dyn std::error::Error>> {
        let (desktop, requests) = recovery_desktop(8)?;
        let directory = tempfile::tempdir()?;
        let mut startup = StartupSettings::load(directory.path());
        let ready = desktop.await_startup_settings(&mut startup, &requests, |_| {
            Err("healthy startup must not publish recovery".into())
        })?;
        assert_eq!(ready, Some(Settings::default()));
        assert!(
            matches!(desktop.enqueue_reset(), Err(message) if message.contains("nur bei nicht lesbaren Starteinstellungen"))
        );
        assert!(requests.is_empty());
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        Ok(())
    }

    #[test]
    fn recovery_allows_one_reset_and_caller_drop_does_not_release_its_reservation()
    -> Result<(), Box<dyn std::error::Error>> {
        let (desktop, requests) = recovery_desktop(8)?;
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .preferences
            .recovery_required = true;
        let caller = desktop.enqueue_reset()?;
        drop(caller);
        assert!(
            matches!(desktop.enqueue_reset(), Err(message) if message == "Eine Einstellungsanfrage läuft bereits")
        );
        assert_eq!(requests.len(), 1);
        let command = requests.recv_timeout(Duration::from_secs(1))?;
        assert!(
            desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        let SettingsCommand::Reset {
            reply,
            _reservation,
        } = command
        else {
            return Err("expected Reset".into());
        };
        assert!(reply.send(Ok(())).is_err());
        assert!(
            desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        drop(_reservation);
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        let next = desktop.enqueue_reset()?;
        drop(requests.recv_timeout(Duration::from_secs(1))?);
        assert!(tauri::async_runtime::block_on(next).is_err());
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        Ok(())
    }

    #[test]
    fn failed_reset_enqueue_releases_admission_for_full_and_disconnected_queues()
    -> Result<(), Box<dyn std::error::Error>> {
        for disconnected in [false, true] {
            // A zero-capacity connected channel deterministically exercises Full;
            // no waiting owner exists. Drop its receiver to exercise Disconnected.
            let (desktop, receiver) = recovery_desktop(0)?;
            let receiver = (!disconnected).then_some(receiver);
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "desktop lock")?
                .preferences
                .recovery_required = true;
            for _ in 0..2 {
                assert!(
                    matches!(desktop.enqueue_reset(), Err(message) if message == "Zurücksetzen konnte nicht angefragt werden")
                );
                assert!(
                    !desktop
                        .0
                        .admission
                        .lock()
                        .map_err(|_| "admission lock")?
                        .changing_settings
                );
            }
            drop(receiver);
        }
        Ok(())
    }

    #[test]
    fn shutdown_rejects_recovery_reset_without_reserving_or_queuing()
    -> Result<(), Box<dyn std::error::Error>> {
        let (desktop, requests) = recovery_desktop(8)?;
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .preferences
            .recovery_required = true;
        desktop.shutdown();
        assert!(
            matches!(desktop.enqueue_reset(), Err(message) if message == "Anwendung wird beendet")
        );
        assert!(requests.is_empty());
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        Ok(())
    }

    #[test]
    fn startup_waitloop_recovers_only_after_explicit_reset_without_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let (desktop, requests) = recovery_desktop(8)?;
        struct ShutdownOnDrop(Desktop);
        impl Drop for ShutdownOnDrop {
            fn drop(&mut self) {
                self.0.shutdown();
            }
        }
        let _shutdown = ShutdownOnDrop(desktop.clone());
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        let invalid = b"invalid settings, preserve until reset";
        std::fs::write(&path, invalid)?;
        let mut startup = StartupSettings::load(directory.path());
        let owner = desktop.clone();
        let (published, snapshots) = bounded(2);
        let worker = thread::spawn(move || {
            owner.await_startup_settings(&mut startup, &requests, |snapshot| {
                published
                    .try_send(snapshot.clone())
                    .map_err(|_| "snapshot receiver unavailable".into())
            })
        });
        let recovery = snapshots.recv_timeout(Duration::from_secs(2))?;
        assert!(recovery.preferences.recovery_required);
        assert!(recovery.preferences.error.is_some());
        assert_eq!(recovery.setup.phase, view::SetupPhase::Failed);
        assert!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "desktop lock")?
                .client
                .is_none()
        );
        assert_eq!(std::fs::read(&path)?, invalid);
        let result = tauri::async_runtime::block_on(desktop.enqueue_reset()?)?;
        assert_eq!(result, Ok(()));
        let ready = snapshots.recv_timeout(Duration::from_secs(2))?;
        assert!(!ready.preferences.recovery_required);
        assert_eq!(ready.preferences.error, None);
        assert_eq!(ready.setup.phase, view::SetupPhase::Checking);
        assert_eq!(
            worker.join().map_err(|_| "startup owner panicked")??,
            Some(Settings::default())
        );
        assert_eq!(
            crate::settings::SettingsStore::new(directory.path()).load()?,
            Some(Settings::default())
        );
        assert!(
            !desktop
                .0
                .admission
                .lock()
                .map_err(|_| "admission lock")?
                .changing_settings
        );
        assert!(desktop.enqueue_reset().is_err());
        Ok(())
    }
    #[test]
    fn shutdown_during_recovery_publication_does_not_resume_startup()
    -> Result<(), Box<dyn std::error::Error>> {
        let (desktop, requests) = recovery_desktop(8)?;
        let directory = tempfile::tempdir()?;
        std::fs::write(directory.path().join("settings.json"), b"broken")?;
        let mut startup = StartupSettings::load(directory.path());
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .preferences
            .recovery_required = true;
        let reply = desktop.enqueue_reset()?;
        let ready = desktop.await_startup_settings(&mut startup, &requests, |snapshot| {
            if !snapshot.preferences.recovery_required {
                // Simulate shutdown winning while publication still owns the shared lock.
                desktop.0.closing.store(true, Ordering::Release);
            }
            Ok(())
        })?;
        assert!(ready.is_none(), "shutdown must prevent resumed startup");
        assert!(
            startup.ready().is_some(),
            "explicit reset itself may complete"
        );
        assert_eq!(tauri::async_runtime::block_on(reply)?, Ok(()));
        Ok(())
    }

    #[test]
    fn shutdown_before_application_handoff_never_constructs_services()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        assert!(desktop.begin_shutdown());
        assert!(!desktop.begin_shutdown());
        let result = desktop.install_application(&Settings::default(), || {
            Err("factory must not execute after shutdown".into())
        })?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn shutdown_after_application_admission_observes_its_published_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        let directory = tempfile::tempdir()?;
        let owner = desktop
            .install_application(&Settings::default(), || {
                AppSupervisor::spawn(
                    directory.path().to_path_buf(),
                    crate::model::REFERENCE,
                    |_| Err("no model in fixture".into()),
                )
                .map_err(|error| error.to_string())
            })?
            .ok_or("owner not admitted")?;
        desktop.shutdown();
        assert!(owner.client().snapshot()?.closing);
        owner.join()?;
        Ok(())
    }
    #[test]
    fn layout_refresh_updates_snapshot_without_restoring_a_previous_selected_key()
    -> Result<(), String> {
        let desktop = unstarted_desktop();
        let initial = desktop.snapshot()?;
        let labels = macos_integration::keyboard::KeyLabels::from_translator(|code, command| {
            Ok(match (code, command) {
                (0x02, false) => "E",
                (0x02, true) => "D",
                (0x28, _) => "T",
                _ => "X",
            }
            .into())
        });
        let changed = desktop
            .update_hotkey_labels(labels.clone())?
            .ok_or("missing update")?;
        assert!(changed.revision > initial.revision);
        assert_eq!(changed.labels.hotkey, "⌥E");
        assert_eq!(changed.preferences.key_labels.len(), 26);
        assert!(desktop.update_hotkey_labels(labels.clone())?.is_none());
        // A settings commit can occur before a later layout refresh. The full
        // table carries no selected key; projection must use the current choice.
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock failed")?;
            shared.settings.hotkey.key = crate::settings::PhysicalKey::K;
            shared.settings.hotkey.command = true;
        }
        desktop.update_hotkey_labels(labels)?;
        assert_eq!(desktop.snapshot()?.labels.hotkey, "⌥⌘T");
        assert_eq!(
            desktop.snapshot()?.preferences.current.hotkey.key,
            crate::settings::PhysicalKey::K
        );
        let fallback = desktop
            .update_hotkey_labels(Default::default())?
            .ok_or("missing fallback")?;
        assert_eq!(fallback.labels.hotkey, "⌥⌘[physisch K; Layout unbekannt]");
        desktop.shutdown();
        assert!(desktop.update_hotkey_labels(Default::default())?.is_none());
        Ok(())
    }
    #[test]
    fn maximum_notice_survives_finalization_and_sent_without_replacing_the_result() {
        let mut value = initial();
        value.setup.phase = view::SetupPhase::Ready;
        value.session.maximum_reached = true;
        value.session.phase = view::SessionPhase::Finalizing;
        assert_eq!(
            crate::header::project(&value).status,
            "5-Minuten-Grenze erreicht · Diktat wird verarbeitet"
        );
        value.session.phase = view::SessionPhase::Idle;
        value.session.outcome = Some(view::SessionOutcome::Sent);
        assert_eq!(
            crate::header::project(&value).status,
            "5-Minuten-Grenze erreicht · Einfügebefehl gesendet"
        );
    }

    #[test]
    fn native_feedback_is_bounded_text_free_and_observes_sticky_held_after_another_start()
    -> Result<(), String> {
        let desktop = unstarted_desktop();
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock failed")?;
            shared.view.setup.phase = view::SetupPhase::Ready;
            shared.view.session.phase = view::SessionPhase::Idle;
            shared.view.session.last_held_session = Some(7);
            shared.view.session.text = "PRIVATE TRANSCRIPT MUST NOT CROSS FEEDBACK".repeat(1000);
            shared.view.session.outcome = Some(view::SessionOutcome::Held {
                message: "界".repeat(1000),
            });
            shared.view.error = Some("Sichere Eingabe aktiv".into());
        }
        let (revision, state) = desktop
            .native_feedback_after(None)?
            .ok_or("missing feedback")?;
        assert!(state.status.len() <= 512);
        assert!(state.status.contains("Sichere Eingabe aktiv"));
        assert!(!format!("{state:?}").contains("PRIVATE TRANSCRIPT"));
        assert_eq!(state.held_session, Some(7));
        assert!(desktop.native_feedback_after(Some(revision))?.is_none());
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock failed")?;
            shared.view.session.phase = view::SessionPhase::Recording;
            shared.view.session.outcome = None;
            shared.view.session.maximum_warning = true;
            shared.view.revision += 1;
        }
        let (_, changed) = desktop
            .native_feedback_after(Some(revision))?
            .ok_or("missing change")?;
        assert_eq!(changed.held_session, Some(7));
        assert!(!changed.idle);
        assert!(changed.attention);
        assert_eq!(changed.status, "Aufnahme endet bei 5 Minuten");
        // A manual stop after the warning does not become a maximum-stop hint.
        let mut view = desktop.snapshot()?;
        view.session.phase = view::SessionPhase::Idle;
        view.session.maximum_reached = false;
        view.session.outcome = Some(view::SessionOutcome::Sent);
        let stopped = crate::feedback::project(&view);
        assert!(!stopped.attention);
        assert_eq!(stopped.status, "Einfügebefehl gesendet");
        Ok(())
    }
}
