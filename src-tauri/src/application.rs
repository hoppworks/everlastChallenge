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
}
#[derive(Clone)]
pub struct Client {
    state: Arc<Mutex<State>>,
    setup: Arc<SetupService>,
    requests: Sender<u64>,
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
            .try_send(state.setup_epoch)
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
        runtime_result.and(setup_result)
    }
}

pub struct AppSupervisor {
    client: Client,
    worker: Option<JoinHandle<Result<(), ApplicationError>>>,
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
        Self::spawn(
            definition.cache_directory(&directory),
            definition,
            move |endpoint| NativeService::for_app(endpoint, app),
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
        let setup = Arc::new(SetupService::new(directory, definition));
        let snapshot = setup.snapshot().map_err(worker_error)?;
        let state = Arc::new(Mutex::new(State {
            snapshot: Snapshot {
                revision: 1,
                running: true,
                closing: false,
                setup: snapshot,
                runtime_available: false,
                error: None,
            },
            runtime: None,
            setup_epoch: 0,
        }));
        let (requests, receive) = bounded(8);
        let client = Client {
            state: Arc::clone(&state),
            setup: Arc::clone(&setup),
            requests,
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
                            if runtime.is_none()
                                && let Some(inference) = setup.take_ready().map_err(worker_error)?
                            {
                                // Startup admission shares the shutdown lock. Once admitted, ownership
                                // stays on this thread even if shutdown is requested immediately after.
                                let (port, endpoint) = runtime::native_mailbox();
                                runtime = Some(
                                    Runtime::spawn_with_policy(
                                        inference,
                                        1,
                                        port,
                                        crate::pipeline::APP_POLICY,
                                    )
                                    .map_err(worker_error)?,
                                );
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
                            Ok(epoch) => {
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
                let result =
                    finish_application([result, runtime_cleanup, native_cleanup, setup_cleanup]);
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
