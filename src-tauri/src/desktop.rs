//! Desktop owner and renderer-facing projection. Session truth stays in Runtime.
use crate::{
    application::{AppSupervisor, Client as AppClient},
    contracts as view,
    controller::SessionSettings,
    hotkeys::{self, Hotkeys},
    runtime::Client as RuntimeClient,
    settings::{Settings, SettingsStore},
    setup,
};
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

pub const SNAPSHOT_EVENT: &str = "dictation:snapshot";
struct Shared {
    view: view::Snapshot,
    client: Option<AppClient>,
    settings: Settings,
    action_error: Option<String>,
    settings_requests: u32,
}
struct Inner {
    shared: Mutex<Shared>,
    closing: AtomicBool,
    refresh: AtomicBool,
    worker: Mutex<Option<JoinHandle<Result<(), String>>>>,
    finished: AtomicBool,
    admissions: Arc<Semaphore>,
    prompts: Arc<Semaphore>,
    // Serializes a power transition with start admission and runtime handoff.
    suspended: Mutex<bool>,
}
#[derive(Clone)]
pub struct Desktop(Arc<Inner>);
impl Desktop {
    pub fn spawn(app: tauri::AppHandle) -> Result<Self, String> {
        let desktop = Self(Arc::new(Inner {
            shared: Mutex::new(Shared {
                view: initial(),
                client: None,
                settings: Settings::default(),
                action_error: None,
                settings_requests: 0,
            }),
            closing: AtomicBool::new(false),
            refresh: AtomicBool::new(true),
            worker: Mutex::new(None),
            finished: AtomicBool::new(false),
            admissions: Arc::new(Semaphore::new(16)),
            prompts: Arc::new(Semaphore::new(1)),
            suspended: Mutex::new(false),
        }));
        let owner = desktop.clone();
        let worker = thread::Builder::new()
            .name("desktop-owner".into())
            .spawn(move || {
                let _finished = Finish(Arc::clone(&owner.0));
                owner.run(app)
            })
            .map_err(|error| error.to_string())?;
        *desktop.0.worker.lock().map_err(|_| "Desktop lock failed")? = Some(worker);
        Ok(desktop)
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
        if shared.view.settings_busy {
            return Err("Einstellungen sind momentan nicht verfügbar".into());
        }
        shared.settings_requests = shared
            .settings_requests
            .checked_add(1)
            .ok_or("Einstellungsanfragen ausgeschöpft; Anwendung neu starten")?;
        Ok(shared.settings_requests)
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
    pub fn native_action(&self, action: macos_integration::glass_header::HeaderAction) {
        use macos_integration::glass_header::HeaderAction;
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_error = None;
        }
        let result = (|| {
            match action {
                HeaderAction::Settings => self.request_settings().map(|_| ()),
                HeaderAction::Stop(session) => self.stop_recording(session.get()),
                HeaderAction::Cancel(session) => self.cancel_recording(session.get()),
                HeaderAction::Start => {
                    // Bound acknowledgement waiters before spawning any task.
                    let admission = self.ui_permit()?;
                    let reply = self.start_request(None, false, false)?;
                    let owner = self.clone();
                    tauri::async_runtime::spawn(async move {
                        let _admission = admission;
                        if let Err(error) = reply
                            .await
                            .map_err(|_| "Aufnahmeanfrage beendet".to_owned())
                            .and_then(|result| result)
                        {
                            owner.action_error(error);
                        }
                    });
                    Ok(())
                }
            }
        })();
        if let Err(error) = result {
            self.action_error(error);
        }
    }
    pub fn refresh_permissions(&self) {
        self.0.refresh.store(true, Ordering::Release);
    }
    /// Called by the native notification observer. Never joins workers or resumes recording.
    pub fn set_suspended(&self, sleeping: bool) -> Result<(), String> {
        let mut suspended = self.0.suspended.lock().map_err(|_| "Power lock failed")?;
        *suspended = sleeping;
        if !*suspended {
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
                if *suspended {
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
            shared.action_error = Some(error);
        }
    }
    pub fn download(&self) -> Result<(), String> {
        if let Ok(mut shared) = self.0.shared.lock() {
            shared.action_error = None;
        }
        self.app_client()?
            .download()
            .map_err(|error| error.to_string())
    }
    pub fn cancel_setup(&self) -> Result<(), String> {
        self.app_client()?
            .cancel_setup()
            .map_err(|error| error.to_string())
    }
    fn start_request(
        &self,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<oneshot::Receiver<Result<(), String>>, String> {
        let suspended = self.0.suspended.lock().map_err(|_| "Power lock failed")?;
        if *suspended {
            return Err("Aufnahme während des Ruhezustands nicht verfügbar".into());
        }
        let runtime = self.runtime()?;
        let stopping = toggle
            && matches!(
                runtime
                    .snapshot()
                    .map_err(|error| error.to_string())?
                    .session
                    .phase,
                Phase::Starting | Phase::Recording
            );
        if !stopping && ui_services::microphone_authorization() != MicAuthorization::Granted {
            return Err("Bitte zuerst den Mikrofonzugriff freigeben".into());
        }
        let settings = self
            .0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .settings
            .clone();
        runtime
            .start(
                SessionSettings {
                    device: settings.device,
                    language: settings.language.into(),
                    generation: 1,
                },
                process,
                delivery,
                toggle,
            )
            .map_err(|error| error.to_string())
    }
    pub async fn start_recording(&self) -> Result<(), String> {
        let _admission = self
            .0
            .admissions
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Zu viele laufende Anfragen")?;
        self.start_request(None, false, false)?
            .await
            .map_err(|_| "Aufnahmeanfrage beendet")?
    }
    pub fn stop_recording(&self, session: u64) -> Result<(), String> {
        valid_id(session)?;
        self.runtime()?
            .stop(session)
            .map_err(|error| error.to_string())
    }
    pub fn cancel_recording(&self, session: u64) -> Result<(), String> {
        valid_id(session)?;
        self.runtime()?
            .cancel(session)
            .map_err(|error| error.to_string())
    }
    pub async fn clear_text(&self) -> Result<(), String> {
        let _admission = self
            .0
            .admissions
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Zu viele laufende Anfragen")?;
        self.runtime()?
            .clear_text()
            .map_err(|error| error.to_string())?
            .await
            .map_err(|_| "Textanfrage beendet")?
    }
    pub fn text(&self) -> Result<String, String> {
        self.runtime()?
            .snapshot()
            .map_err(|error| error.to_string())?
            .session
            .text
            .ok_or_else(|| "Kein Text zum Kopieren vorhanden".into())
    }
    fn hotkey(&self, event: hotkeys::Event) {
        match event {
            hotkeys::Event::Cancel { session } => {
                let result = self
                    .runtime()
                    .and_then(|runtime| runtime.cancel(session).map_err(|error| error.to_string()));
                if let Err(error) = result {
                    self.action_error(error);
                }
            }
            hotkeys::Event::Toggle { process } => {
                // A second press is a priority stop, before ordinary request admission.
                if let Ok(runtime) = self.runtime()
                    && let Ok(snapshot) = runtime.snapshot()
                    && matches!(snapshot.session.phase, Phase::Starting | Phase::Recording)
                    && let Some(session) = snapshot.session.session
                {
                    if let Err(error) = runtime.stop(session) {
                        self.action_error(error.to_string());
                    }
                    return;
                }
                let Ok(permit) = self.0.admissions.clone().try_acquire_owned() else {
                    self.action_error("Zu viele laufende Anfragen".into());
                    return;
                };
                match self.start_request(process, true, true) {
                    Ok(reply) => {
                        let desktop = self.clone();
                        tauri::async_runtime::spawn(async move {
                            let _permit = permit;
                            match reply.await {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => desktop.action_error(error),
                                Err(_) => desktop.action_error("Aufnahmeanfrage beendet".into()),
                            }
                        });
                    }
                    Err(error) => self.action_error(error),
                }
            }
        }
    }
    pub fn shutdown(&self) {
        self.0.closing.store(true, Ordering::Release);
        if let Ok(shared) = self.0.shared.lock()
            && let Some(client) = &shared.client
            && let Err(error) = client.shutdown()
        {
            eprintln!("Application shutdown request failed: {error}");
        }
    }
    pub fn begin_shutdown(&self) -> bool {
        let first = !self.0.closing.swap(true, Ordering::AcqRel);
        self.shutdown();
        first
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
    fn run(&self, app: tauri::AppHandle) -> Result<(), String> {
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
            let store = SettingsStore::new(directory);
            let settings = match store.load().map_err(|error| error.to_string())? {
                Some(settings) => settings,
                None => {
                    let settings = Settings::default();
                    store.save(&settings).map_err(|error| error.to_string())?;
                    settings
                }
            };
            supervisor = Some(
                AppSupervisor::for_app(app.clone(), settings.model.definition())
                    .map_err(|error| error.to_string())?,
            );
            let client = supervisor
                .as_ref()
                .ok_or("Application owner missing")?
                .client();
            self.0
                .shared
                .lock()
                .map_err(|_| "Desktop lock failed")?
                .settings = settings.clone();
            self.0
                .shared
                .lock()
                .map_err(|_| "Desktop lock failed")?
                .client = Some(client.clone());
            let callback = self.clone();
            if self.0.closing.load(Ordering::Acquire) {
                return Ok(());
            }
            match Hotkeys::install(app.clone(), settings.hotkey, move |event| {
                callback.hotkey(event)
            }) {
                Ok(owner) => hotkeys = Some(owner),
                Err(error) => self.action_error(format!("Tastenkürzel nicht verfügbar: {error}")),
            }
            while !self.0.closing.load(Ordering::Acquire) {
                let application = client.snapshot().map_err(|error| error.to_string())?;
                let suspended = self.0.suspended.lock().map_err(|_| "Power lock failed")?;
                let runtime = match client.runtime() {
                    Ok(runtime) => runtime,
                    Err(_) if self.0.closing.load(Ordering::Acquire) => break,
                    Err(error) => return Err(error.to_string()),
                };
                if *suspended && let Some(runtime) = &runtime {
                    runtime.suspend().map_err(|error| error.to_string())?;
                }
                let runtime = runtime
                    .map(|runtime| runtime.snapshot())
                    .transpose()
                    .map_err(|error| error.to_string())?;
                drop(suspended);
                let active = runtime
                    .as_ref()
                    .filter(|runtime| runtime.session.phase != Phase::Idle)
                    .and_then(|runtime| runtime.session.session);
                if let Some(hotkeys) = &mut hotkeys {
                    if let Err(error) = hotkeys.enable_cancel(active) {
                        self.action_error(error.to_string());
                    }
                    if let Some(error) = hotkeys.take_error().map_err(|error| error.to_string())? {
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
                if let Some(native) = native {
                    next.permissions = permissions(native);
                }
                next.settings_request = shared.settings_requests;
                next.error = shared.action_error.clone().or(next.error);
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
        let result = result.and(hotkey_cleanup).and(cleaned);
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
fn initial() -> view::Snapshot {
    let settings = Settings::default();
    let definition = settings.model.definition();
    view::Snapshot {
        revision: 1,
        running: true,
        setup: view::Setup {
            phase: view::SetupPhase::Checking,
            model: view::Model {
                name: definition.id().into(),
                download_bytes: definition.bytes() as u32,
                source_url: definition.url(),
            },
            downloaded_bytes: 0,
            total_bytes: definition.bytes() as u32,
            can_cancel: false,
            error: None,
        },
        session: view::Session {
            phase: view::SessionPhase::Idle,
            id: None,
            text: String::new(),
            outcome: None,
            maximum_warning: false,
        },
        permissions: view::Permissions {
            microphone: view::PermissionStatus::Unavailable,
            accessibility: view::PermissionStatus::Unavailable,
        },
        labels: view::Labels {
            device: "Standardmikrofon".into(),
            language: "Deutsch".into(),
            hotkey: "⌥D (physisch)".into(),
        },
        settings_busy: true,
        settings_request: 0,
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
    result.setup.model = view::Model {
        name: definition.id().into(),
        download_bytes: definition.bytes() as u32,
        source_url: definition.url(),
    };
    result.setup.downloaded_bytes = application.setup.received.min(u64::from(u32::MAX)) as u32;
    result.setup.total_bytes = application.setup.total.min(u64::from(u32::MAX)) as u32;
    result.setup.can_cancel = matches!(
        application.setup.phase,
        setup::Phase::Checking | setup::Phase::Downloading | setup::Phase::Loading
    );
    result.setup.error = application.setup.failure.as_ref().map(|failure| {
        match failure {
            setup::Failure::Permission => "Das Modellverzeichnis ist nicht zugänglich. Bitte prüfe die Dateirechte.",
            setup::Failure::DiskFull => "Es ist nicht genügend Speicherplatz für das Modell frei.",
            setup::Failure::Io => "Die Modelldatei konnte nicht gelesen oder gespeichert werden.",
            setup::Failure::Size | setup::Failure::Hash => "Die Modelldatei ist unvollständig oder beschädigt. Bitte lade sie erneut.",
            setup::Failure::NetworkOrTls => "Die sichere Verbindung zur Modellquelle ist fehlgeschlagen. Bitte prüfe deine Verbindung.",
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
    let key = &settings.hotkey;
    result.labels.hotkey = format!(
        "{}{}{}{}{:?} (physisch)",
        if key.control { "⌃" } else { "" },
        if key.option { "⌥" } else { "" },
        if key.shift { "⇧" } else { "" },
        if key.command { "⌘" } else { "" },
        key.key
    );
    if let Some(runtime) = runtime {
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

struct Finish(Arc<Inner>);
impl Drop for Finish {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{controller::SessionSnapshot, model::REFERENCE, runtime::RuntimeSnapshot};
    fn unstarted_desktop() -> Desktop {
        Desktop(Arc::new(Inner {
            shared: Mutex::new(Shared {
                view: initial(),
                client: None,
                settings: Settings::default(),
                action_error: None,
                settings_requests: 0,
            }),
            closing: AtomicBool::new(false),
            refresh: AtomicBool::new(false),
            worker: Mutex::new(None),
            finished: AtomicBool::new(false),
            admissions: Arc::new(Semaphore::new(16)),
            prompts: Arc::new(Semaphore::new(1)),
            suspended: Mutex::new(false),
        }))
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
    fn native_settings_requests_are_monotone_and_respect_busy_and_shutdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let desktop = unstarted_desktop();
        assert!(desktop.request_settings().is_err());
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "lock")?
            .view
            .settings_busy = false;
        assert_eq!(desktop.request_settings()?, 1);
        assert_eq!(desktop.request_settings()?, 2);
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
        assert!(!recording.settings_enabled);
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
                phase: Phase::Recording,
                session: Some(7),
                text: Some("previous words".into()),
                outcome: None,
            },
            maximum_warning: true,
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
                phase: Phase::Idle,
                session: None,
                text: Some("retained words".into()),
                outcome: Some(Outcome::Held("Original field changed".into())),
            },
            maximum_warning: false,
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
    }
    #[test]
    fn ipc_session_ids_are_positive_exact_javascript_integers() {
        assert!(valid_id(0).is_err());
        assert!(valid_id(9_007_199_254_740_992).is_err());
        assert!(valid_id(1).is_ok());
        assert!(valid_id(9_007_199_254_740_991).is_ok());
    }
}
