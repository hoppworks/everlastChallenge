//! Application lifetime owner. Model I/O, startup and joins never run on the GUI thread.
use crate::{
    model::ModelDefinition,
    native_service::NativeService,
    runtime::{self, Client as RuntimeClient, NativeEndpoint, Runtime},
    setup::{self, SetupService},
};
use crossbeam_channel::{Sender, bounded};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::Manager;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub running: bool,
    pub closing: bool,
    pub setup: setup::Snapshot,
    pub refinement_setup: Option<crate::refinement_setup::Snapshot>,
    pub runtime_available: bool,
    pub error: Option<String>,
}
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Die Anwendung wird beendet.")]
    Closed,
    #[error("Es sind bereits zu viele Anwendungsanfragen offen.")]
    Busy,
    #[error("Der Anwendungszustand konnte nicht sicher abgeglichen werden.")]
    Synchronization,
    #[error("Der Hintergrunddienst der Anwendung ist fehlgeschlagen: {0}")]
    Worker(String),
}
struct State {
    snapshot: Snapshot,
    runtime: Option<RuntimeClient>,
    setup_epoch: u64,
    refinement_epoch: u64,
    refinement_pending: bool,
}
enum SetupRequest {
    Asr(u64),
    Refinement(u64),
}
#[derive(Clone)]
pub struct Client {
    state: Arc<Mutex<State>>,
    setup: Arc<SetupService>,
    requests: Sender<SetupRequest>,
    refinement: Option<Arc<crate::refinement_setup::Setup>>,
}
impl Client {
    pub fn snapshot(&self) -> Result<Snapshot, ApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?
            .snapshot
            .clone())
    }
    pub fn runtime(&self) -> Result<Option<RuntimeClient>, ApplicationError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if state.snapshot.closing || !state.snapshot.running {
            return Err(state
                .snapshot
                .error
                .clone()
                .map(ApplicationError::Worker)
                .unwrap_or(ApplicationError::Closed));
        }
        Ok(state.runtime.clone())
    }
    pub fn download(&self) -> Result<(), ApplicationError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if state.snapshot.closing || !state.snapshot.running {
            return Err(ApplicationError::Closed);
        }
        self.requests
            .try_send(SetupRequest::Asr(state.setup_epoch))
            .map_err(|error| match error {
                crossbeam_channel::TrySendError::Full(_) => ApplicationError::Busy,
                crossbeam_channel::TrySendError::Disconnected(_) => ApplicationError::Closed,
            })
    }
    pub fn cancel_setup(&self) -> Result<(), ApplicationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if state.snapshot.closing || !state.snapshot.running {
            return Err(ApplicationError::Closed);
        }
        state.setup_epoch = state
            .setup_epoch
            .checked_add(1)
            .ok_or(ApplicationError::Busy)?;
        self.setup
            .cancel()
            .map_err(|error| ApplicationError::Worker(error.to_string()))
    }
    pub fn download_refinement(&self) -> Result<(), ApplicationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if state.snapshot.closing || !state.snapshot.running {
            return Err(ApplicationError::Closed);
        }
        if self.refinement.is_none() {
            return Err(worker_error("Local text setup is unavailable"));
        }
        if state.refinement_pending
            || self
                .refinement
                .as_ref()
                .map(|s| s.snapshot())
                .transpose()
                .map_err(worker_error)?
                .is_some_and(|s| s.is_busy())
        {
            return Err(ApplicationError::Busy);
        }
        self.requests
            .try_send(SetupRequest::Refinement(state.refinement_epoch))
            .map_err(|error| match error {
                crossbeam_channel::TrySendError::Full(_) => ApplicationError::Busy,
                crossbeam_channel::TrySendError::Disconnected(_) => ApplicationError::Closed,
            })?;
        state.refinement_pending = true;
        Ok(())
    }
    /// Includes accepted requests before their I/O worker starts.
    pub fn refinement_busy(&self) -> Result<bool, ApplicationError> {
        self.refinement_status().map(|(_, busy)| busy)
    }
    /// Read current setup state and queue occupancy together. The renderer's
    /// published snapshot can lag behind a just-completed setup attempt.
    pub fn refinement_status(
        &self,
    ) -> Result<(Option<crate::refinement_setup::Snapshot>, bool), ApplicationError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        let snapshot = self
            .refinement
            .as_ref()
            .map(|s| s.snapshot())
            .transpose()
            .map_err(worker_error)?;
        let busy = state.refinement_pending || snapshot.as_ref().is_some_and(|s| s.is_busy());
        Ok((snapshot, busy))
    }
    pub fn cancel_refinement_setup(&self) -> Result<(), ApplicationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if state.snapshot.closing || !state.snapshot.running {
            return Err(ApplicationError::Closed);
        }
        state.refinement_epoch = state
            .refinement_epoch
            .checked_add(1)
            .ok_or(ApplicationError::Busy)?;
        state.refinement_pending = false;
        self.refinement
            .as_ref()
            .ok_or_else(|| worker_error("Local text setup is unavailable"))?
            .cancel()
            .map_err(worker_error)
    }
    /// Revokes access immediately; cleanup and joins remain on the owner thread.
    pub fn shutdown(&self) -> Result<(), ApplicationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Synchronization)?;
        if !state.snapshot.closing {
            state.snapshot.closing = true;
            state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        }
        let runtime_result = match &state.runtime {
            Some(runtime) => runtime.shutdown().map_err(worker_error),
            None => Ok(()),
        };
        let setup_result = match self.setup.cancel() {
            Ok(()) | Err(setup::SetupError::Closed) => Ok(()),
            Err(error) => Err(worker_error(error)),
        };
        let refinement_result = match &self.refinement {
            Some(refinement) => match refinement.cancel() {
                Ok(()) | Err(setup::SetupError::Closed) => Ok(()),
                Err(error) => Err(worker_error(error)),
            },
            None => Ok(()),
        };
        runtime_result.and(setup_result).and(refinement_result)
    }
}

pub struct AppSupervisor {
    client: Client,
    worker: Option<JoinHandle<Result<(), ApplicationError>>>,
}
struct RefinementConfig {
    models: PathBuf,
    worker: crate::refinement_service::WorkerSpec,
}
impl AppSupervisor {
    pub fn for_app(
        app: tauri::AppHandle,
        definition: ModelDefinition,
    ) -> Result<Self, ApplicationError> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| ApplicationError::Worker(error.to_string()))?
            .join("models");
        let model = crate::model::refinement::RefinementModel::bundled().map_err(worker_error)?;
        let executable = std::env::current_exe().map_err(worker_error)?;
        let executable = executable
            .parent()
            .ok_or_else(|| worker_error("Application directory is unavailable"))?
            .join("text-refinement-worker");
        let refinement = RefinementConfig {
            models: directory.clone(),
            worker: crate::refinement_service::WorkerSpec {
                executable,
                model_directory: model.cache_directory(&directory),
                runtime_directory: directory
                    .parent()
                    .ok_or_else(|| worker_error("Application data directory is unavailable"))?
                    .join("text-runtime"),
                identity: crate::refinement_service::ExpectedIdentity {
                    generation: std::num::NonZeroU64::MIN,
                    model_manifest_sha256: crate::model::refinement::RefinementModel::fingerprint(),
                    prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
                    policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
                },
            },
        };
        Self::spawn_configured(
            definition.cache_directory(&directory),
            definition,
            move |endpoint| NativeService::for_app(endpoint, app),
            Some(refinement),
        )
    }
    /// Starts the owner with a trusted host-selected model directory and native factory.
    /// This is a host integration boundary, not an IPC command: never pass renderer paths
    /// or URLs here. Normal application startup uses `for_app` and its own AppData path.
    /// Cached models are checked automatically; network access needs `Client::download`.
    pub fn spawn(
        directory: PathBuf,
        definition: ModelDefinition,
        native: impl FnOnce(NativeEndpoint) -> Result<NativeService, String> + Send + 'static,
    ) -> Result<Self, ApplicationError> {
        Self::spawn_configured(directory, definition, native, None)
    }
    fn spawn_configured(
        directory: PathBuf,
        definition: ModelDefinition,
        native: impl FnOnce(NativeEndpoint) -> Result<NativeService, String> + Send + 'static,
        refinement: Option<RefinementConfig>,
    ) -> Result<Self, ApplicationError> {
        let refinement_setup = refinement
            .as_ref()
            .map(|config| {
                crate::refinement_setup::Setup::new(config.models.clone(), config.worker.clone())
                    .map(Arc::new)
            })
            .transpose()
            .map_err(worker_error)?;
        let setup = Arc::new(SetupService::new(directory, definition));
        let snapshot = setup.snapshot().map_err(worker_error)?;
        let state = Arc::new(Mutex::new(State {
            snapshot: Snapshot {
                revision: 1,
                running: true,
                closing: false,
                setup: snapshot,
                refinement_setup: refinement_setup
                    .as_ref()
                    .map(|setup| setup.snapshot())
                    .transpose()
                    .map_err(worker_error)?,
                runtime_available: false,
                error: None,
            },
            runtime: None,
            setup_epoch: 0,
            refinement_epoch: 0,
            refinement_pending: false,
        }));
        let (requests, receive) = bounded(8);
        let client = Client {
            state: Arc::clone(&state),
            setup: Arc::clone(&setup),
            requests,
            refinement: refinement_setup.clone(),
        };
        let worker = thread::Builder::new()
            .name("application-owner".into())
            .spawn(move || {
                let _exit = Exit(Arc::clone(&state));
                let mut runtime: Option<Runtime> = None;
                let mut native_service: Option<NativeService> = None;
                let mut factory = Some(native);
                let result = (|| {
                    {
                        let state = state
                            .lock()
                            .map_err(|_| ApplicationError::Synchronization)?;
                        if !state.snapshot.closing {
                            setup.start(false).map_err(worker_error)?;
                            if let Some(refinement) = &refinement_setup {
                                refinement.start(false).map_err(worker_error)?;
                            }
                        }
                    }
                    loop {
                        {
                            let mut state = state
                                .lock()
                                .map_err(|_| ApplicationError::Synchronization)?;
                            if state.snapshot.closing {
                                break;
                            }
                            let setup_snapshot = setup.snapshot().map_err(worker_error)?;
                            if setup_snapshot.revision != state.snapshot.setup.revision {
                                state.snapshot.setup = setup_snapshot;
                                state.snapshot.revision = state.snapshot.revision.saturating_add(1);
                            }
                            if let Some(refinement) = &refinement_setup {
                                let snapshot = refinement.snapshot().map_err(worker_error)?;
                                if state.snapshot.refinement_setup.as_ref() != Some(&snapshot) {
                                    state.snapshot.refinement_setup = Some(snapshot);
                                    state.snapshot.revision =
                                        state.snapshot.revision.saturating_add(1);
                                }
                            }
                            if runtime.is_none()
                                && let Some(inference) = setup.take_ready().map_err(worker_error)?
                            {
                                // Startup admission shares the shutdown lock. Once admitted, ownership
                                // stays on this thread even if shutdown is requested immediately after.
                                let (port, endpoint) = runtime::native_mailbox();
                                let (started, refinement_error) =
                                    start_runtime(inference, port, refinement.as_ref())?;
                                runtime = Some(started);
                                if let Some(error) = refinement_error {
                                    state.snapshot.error = Some(error);
                                }
                                let start_native = factory.take().ok_or_else(|| {
                                    worker_error(
                                        "Der Dienst für die Übergabe wurde bereits eingerichtet.",
                                    )
                                })?;
                                native_service =
                                    Some(start_native(endpoint).map_err(worker_error)?);
                                state.runtime = runtime.as_ref().map(Runtime::client);
                                state.snapshot.runtime_available = true;
                                state.snapshot.revision = state.snapshot.revision.saturating_add(1);
                            }
                            if let Some(runtime) = &runtime {
                                let snapshot = runtime.client().snapshot().map_err(worker_error)?;
                                if !snapshot.running {
                                    return Err(worker_error(snapshot.error.unwrap_or_else(
                                        || "Der Diktierdienst wurde unerwartet beendet.".into(),
                                    )));
                                }
                            }
                            if native_service
                                .as_ref()
                                .is_some_and(NativeService::is_finished)
                            {
                                return Err(worker_error(
                                    "Der Dienst für die Übergabe wurde unerwartet beendet.",
                                ));
                            }
                        }
                        match receive.recv_timeout(Duration::from_millis(10)) {
                            Ok(SetupRequest::Asr(epoch)) => {
                                let mut state = state
                                    .lock()
                                    .map_err(|_| ApplicationError::Synchronization)?;
                                if !state.snapshot.closing && state.setup_epoch == epoch {
                                    match setup.start(true) {
                                        Ok(()) => state.snapshot.error = None,
                                        Err(error) => {
                                            state.snapshot.error = Some(error.to_string())
                                        }
                                    }
                                    state.snapshot.revision =
                                        state.snapshot.revision.saturating_add(1);
                                }
                            }
                            Ok(SetupRequest::Refinement(epoch)) => {
                                let mut state = state
                                    .lock()
                                    .map_err(|_| ApplicationError::Synchronization)?;
                                if !state.snapshot.closing
                                    && state.refinement_epoch == epoch
                                    && let Some(refinement) = &refinement_setup
                                {
                                    state.snapshot.error =
                                        refinement.start(true).err().map(|error| error.to_string());
                                    state.snapshot.revision =
                                        state.snapshot.revision.saturating_add(1);
                                    state.refinement_pending = false;
                                }
                            }
                            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    Ok(())
                })();
                // Revoke published access before waiting; never join while holding the GUI-facing lock.
                if let Ok(mut state) = state.lock() {
                    state.runtime = None;
                    state.snapshot.runtime_available = false;
                    state.snapshot.closing = true;
                }
                if let Some(native) = &native_service {
                    native.stop();
                }
                let runtime_cleanup = match runtime {
                    Some(runtime) => runtime.join().map_err(worker_error),
                    None => Ok(()),
                };
                let native_cleanup = match native_service {
                    Some(native) => native.join().map_err(worker_error),
                    None => Ok(()),
                };
                let setup_cleanup = setup.shutdown().map_err(worker_error);
                let refinement_cleanup = match &refinement_setup {
                    Some(setup) => setup.shutdown().map_err(worker_error),
                    None => Ok(()),
                };
                let result = finish_application([
                    result,
                    runtime_cleanup,
                    native_cleanup,
                    setup_cleanup,
                    refinement_cleanup,
                ]);
                if let Err(error) = &result
                    && let Ok(mut state) = state.lock()
                {
                    state.snapshot.error = Some(error.to_string());
                }
                result
            })
            .map_err(worker_error)?;
        Ok(Self {
            client,
            worker: Some(worker),
        })
    }
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    /// Call outside the GUI while its event loop remains available for clipboard cleanup.
    pub fn join(mut self) -> Result<(), ApplicationError> {
        let requested = self.client.shutdown();
        let joined = self
            .worker
            .take()
            .ok_or(ApplicationError::Closed)?
            .join()
            .map_err(|_| {
                ApplicationError::Worker("Die Anwendungssteuerung wurde unerwartet beendet.".into())
            })?;
        joined.and(requested)
    }
}

fn start_runtime(
    inference: crate::inference_service::InferenceService,
    port: runtime::NativePort,
    refinement: Option<&RefinementConfig>,
) -> Result<(Runtime, Option<String>), ApplicationError> {
    let prepared = refinement.map(|config| {
        let spec = config.worker.clone();
        crate::refinement_service::scratch::prepare(&spec.runtime_directory)
            .map_err(worker_error)?;
        crate::refinement_service::scratch::recover(&spec.runtime_directory)
            .map_err(worker_error)?;
        crate::refinement_service::owner::Owner::spawn(
            spec,
            crate::refinement_service::Limits::default(),
            Duration::from_secs(120),
        )
        .map_err(worker_error)
    });
    let (owner, error) = match prepared {
        Some(Ok(owner)) => (Some(owner), None),
        Some(Err(error)) => (
            None,
            Some(format!("Textbereinigung nicht verfügbar: {error}")),
        ),
        None => (None, None),
    };
    let runtime = match owner {
        Some(owner) => Runtime::spawn_with_refinement(inference, 1, port, owner),
        None => Runtime::spawn_with_policy(inference, 1, port, crate::pipeline::APP_POLICY),
    }
    .map_err(worker_error)?;
    Ok((runtime, error))
}

impl Drop for AppSupervisor {
    fn drop(&mut self) {
        if self.client.shutdown().is_err() { /* owner already stopped */ }
    }
}

/// Joins have already run independently. Keep their specific failures even if
/// the polling loop first observed only a generic "worker ended" condition.
fn finish_application<const N: usize>(
    results: [Result<(), ApplicationError>; N],
) -> Result<(), ApplicationError> {
    let mut errors = Vec::new();
    for error in results.into_iter().filter_map(Result::err) {
        let message = error.to_string();
        if !errors.contains(&message) {
            errors.push(message);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApplicationError::Worker(errors.join("; ")))
    }
}

fn worker_error(error: impl std::fmt::Display) -> ApplicationError {
    ApplicationError::Worker(error.to_string())
}
struct Exit(Arc<Mutex<State>>);
impl Drop for Exit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(runtime) = &state.runtime
                && let Err(error) = runtime.shutdown()
            {
                state.snapshot.error = Some(error.to_string());
            }
            if !state.snapshot.closing && state.snapshot.error.is_none() {
                state.snapshot.error =
                    Some("Der Hintergrunddienst der Anwendung wurde unerwartet beendet.".into());
            }
            state.runtime = None;
            state.snapshot.runtime_available = false;
            state.snapshot.running = false;
            state.snapshot.closing = true;
            state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::REFERENCE;
    use std::time::Instant;
    fn test_worker(root: &std::path::Path) -> crate::refinement_service::WorkerSpec {
        crate::refinement_service::WorkerSpec {
            executable: root.join("unused-worker"),
            model_directory: root.join("unused-model"),
            runtime_directory: root.join("unused-runtime"),
            identity: crate::refinement_service::ExpectedIdentity {
                generation: std::num::NonZeroU64::MIN,
                model_manifest_sha256: crate::model::refinement::RefinementModel::fingerprint(),
                prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
                policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
            },
        }
    }
    #[test]
    fn accepted_text_download_is_busy_before_dispatch_and_cancel_revokes_queue()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let setup = Arc::new(SetupService::new(root.path().join("asr"), REFERENCE));
        let refinement = Arc::new(crate::refinement_setup::Setup::new(
            root.path().join("models"),
            test_worker(root.path()),
        )?);
        let (requests, receive) = bounded(8);
        let state = Arc::new(Mutex::new(State {
            snapshot: Snapshot {
                revision: 1,
                running: true,
                closing: false,
                setup: setup.snapshot()?,
                refinement_setup: Some(refinement.snapshot()?),
                runtime_available: false,
                error: None,
            },
            runtime: None,
            setup_epoch: 0,
            refinement_epoch: 0,
            refinement_pending: false,
        }));
        let client = Client {
            state,
            setup,
            requests,
            refinement: Some(refinement),
        };
        client
            .state
            .lock()
            .map_err(|_| "state")?
            .snapshot
            .refinement_setup
            .as_mut()
            .ok_or("snapshot")?
            .phase = setup::Phase::Ready;
        let (actual, busy) = client.refinement_status()?;
        assert_eq!(actual.ok_or("actual setup")?.phase, setup::Phase::Missing);
        assert!(!busy);
        client.download_refinement()?;
        let (actual, busy) = client.refinement_status()?;
        assert_eq!(actual.ok_or("actual setup")?.phase, setup::Phase::Missing);
        assert!(busy);
        assert!(client.refinement_busy()?);
        assert!(matches!(
            client.download_refinement(),
            Err(ApplicationError::Busy)
        ));
        client.cancel_refinement_setup()?;
        assert!(!client.refinement_busy()?);
        let SetupRequest::Refinement(epoch) = receive.try_recv()? else {
            return Err("wrong setup request".into());
        };
        assert_ne!(
            epoch,
            client.state.lock().map_err(|_| "state")?.refinement_epoch
        );
        client.shutdown()?;
        Ok(())
    }
    #[test]
    fn optional_text_recovery_runs_on_startup_and_keeps_asr_available()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Decoder;
        impl crate::inference_service::Decoder for Decoder {
            fn decode(
                &mut self,
                _: &[f32],
                _: crate::transcription::Language,
            ) -> Result<String, crate::transcription::InferenceError> {
                Ok("Ready.".into())
            }
        }
        for case in 0..4 {
            let root = tempfile::tempdir()?;
            let path = root.path().join("text-runtime");
            let foreign = root.path().join("foreign");
            std::fs::write(&foreign, b"preserve")?;
            match case {
                0 => std::fs::write(&path, b"preserve")?,
                1 => std::os::unix::fs::symlink(&foreign, &path)?,
                2 => {
                    std::fs::create_dir(&path)?;
                    std::os::unix::fs::symlink(&foreign, path.join(".text-worker.lock"))?;
                }
                _ => {
                    let abandoned = path.join("text-worker-v1-abandoned");
                    std::fs::create_dir_all(&abandoned)?;
                    std::fs::write(abandoned.join(".owner.lock"), b"everlast-text-owner-v1\n")?;
                    std::fs::write(abandoned.join("snapshot"), b"abandoned model")?;
                }
            }
            let config = RefinementConfig {
                models: root.path().join("models"),
                worker: crate::refinement_service::WorkerSpec {
                    executable: root.path().join("unused-worker"),
                    model_directory: root.path().join("models"),
                    runtime_directory: path.clone(),
                    identity: crate::refinement_service::ExpectedIdentity {
                        generation: std::num::NonZeroU64::MIN,
                        model_manifest_sha256:
                            crate::model::refinement::RefinementModel::fingerprint(),
                        prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
                        policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
                    },
                },
            };
            let inference = crate::inference_service::InferenceService::with_decoder(Decoder)?;
            let (port, _endpoint) = runtime::native_mailbox();
            let (runtime, error) = start_runtime(inference, port, Some(&config))?;
            assert_eq!(
                error.is_some(),
                case != 3,
                "optional failure must remain visible"
            );
            if case == 3 {
                assert!(!path.join("text-worker-v1-abandoned").exists());
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let snapshot = runtime.client().snapshot()?;
                assert!(snapshot.running);
                if snapshot.model_ready {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("ASR did not become ready".into());
                }
                thread::sleep(Duration::from_millis(1));
            }
            runtime.join()?;
            assert_eq!(std::fs::read(&foreign)?, b"preserve");
            if case < 2 {
                assert_eq!(std::fs::read(&path)?, b"preserve");
            }
        }
        Ok(())
    }
    #[test]
    fn text_setup_checks_local_cache_without_downloading_or_blocking_the_application()
    -> Result<(), Box<dyn std::error::Error>> {
        for invalid in [false, true] {
            let root = tempfile::tempdir()?;
            let models = root.path().join("models");
            if invalid {
                std::fs::create_dir(&models)?;
                std::fs::write(models.join("text-refinement"), b"not a directory")?;
            }
            let model = crate::model::refinement::RefinementModel::bundled()?;
            let config = RefinementConfig {
                models: models.clone(),
                worker: crate::refinement_service::WorkerSpec {
                    executable: root.path().join("no-worker-without-asr"),
                    model_directory: model.cache_directory(&models),
                    runtime_directory: root.path().join("text-runtime"),
                    identity: crate::refinement_service::ExpectedIdentity {
                        generation: std::num::NonZeroU64::MIN,
                        model_manifest_sha256:
                            crate::model::refinement::RefinementModel::fingerprint(),
                        prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
                        policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
                    },
                },
            };
            let supervisor = AppSupervisor::spawn_configured(
                models.clone(),
                REFERENCE,
                |_| Err("ASR is absent; no native owner should start".into()),
                Some(config),
            )?;
            let client = supervisor.client();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let snapshot = client.snapshot()?;
                let text = snapshot.refinement_setup.ok_or("text setup missing")?;
                if text.revision > 1 && !matches!(text.phase, setup::Phase::Checking) {
                    assert!(snapshot.running);
                    assert!(!snapshot.runtime_available);
                    assert_eq!(
                        text.phase,
                        if invalid {
                            setup::Phase::Failed
                        } else {
                            setup::Phase::Missing
                        }
                    );
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("text setup did not settle".into());
                }
                thread::sleep(Duration::from_millis(1));
            }
            if invalid {
                assert_eq!(
                    std::fs::read(models.join("text-refinement"))?,
                    b"not a directory"
                );
            } else {
                assert!(!models.exists());
            }
            supervisor.join()?;
            assert!(matches!(
                client.download_refinement(),
                Err(ApplicationError::Closed)
            ));
            assert!(matches!(
                client.cancel_refinement_setup(),
                Err(ApplicationError::Closed)
            ));
        }
        Ok(())
    }
    #[test]
    fn invalid_cache_remains_visible_and_is_not_replaced_automatically()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(REFERENCE.file_name());
        std::fs::write(&path, b"invalid model")?;
        let supervisor = AppSupervisor::spawn(directory.path().into(), REFERENCE, |_| {
            Err("Invalid cache must not start native services".into())
        })?;
        let client = supervisor.client();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = client.snapshot()?;
            if matches!(snapshot.setup.phase, setup::Phase::Failed) {
                assert_eq!(snapshot.setup.failure, Some(setup::Failure::Size));
                assert!(!snapshot.runtime_available);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "invalid cache failure did not arrive"
            );
            thread::yield_now();
        }
        assert_eq!(std::fs::read(path)?, b"invalid model");
        supervisor.join()?;
        assert!(matches!(
            client.cancel_setup(),
            Err(ApplicationError::Closed)
        ));
        client.shutdown()?;
        Ok(())
    }

    #[test]
    fn failed_native_join_retains_its_cause_after_generic_worker_observation() {
        let result = finish_application([
            Err(worker_error(
                "Der Dienst für die Übergabe wurde unerwartet beendet.",
            )),
            Ok(()),
            Err(worker_error("AX worker disconnected during cleanup")),
            Ok(()),
        ]);
        assert!(
            matches!(result, Err(ApplicationError::Worker(message)) if message.contains("unerwartet beendet") && message.contains("AX worker disconnected during cleanup"))
        );
    }

    #[test]
    fn closed_runtime_access_retains_the_specific_service_cause()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let supervisor = AppSupervisor::spawn(directory.path().join("models"), REFERENCE, |_| {
            Err("No native service needed".into())
        })?;
        let client = supervisor.client();
        client.shutdown()?;
        {
            let mut state = client.state.lock().map_err(|_| "state poisoned")?;
            state.snapshot.error = Some("Native delivery worker failed".into());
        }
        assert!(
            matches!(client.runtime(), Err(ApplicationError::Worker(message)) if message == "Native delivery worker failed")
        );
        supervisor.join()?;
        Ok(())
    }

    #[test]
    fn missing_cache_does_not_download_or_start_runtime_and_shutdown_is_terminal()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let supervisor = AppSupervisor::spawn(directory.path().join("models"), REFERENCE, |_| {
            Err("Native services must not start without a verified model".into())
        })?;
        let client = supervisor.client();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = client.snapshot()?;
            if matches!(snapshot.setup.phase, setup::Phase::Missing) && snapshot.revision > 1 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "cache inspection did not complete"
            );
            thread::yield_now();
        }
        assert!(client.runtime()?.is_none());
        assert!(!directory.path().join("models").exists());
        client.shutdown()?;
        assert!(matches!(client.download(), Err(ApplicationError::Closed)));
        assert!(matches!(client.runtime(), Err(ApplicationError::Closed)));
        supervisor.join()?;
        assert!(!client.snapshot()?.running);
        Ok(())
    }
}
