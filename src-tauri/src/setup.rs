//! Owns model I/O and initialization outside the GUI. No inference context is loaded twice concurrently.
use crate::{
    inference_service::InferenceService,
    model::{self, ModelDefinition, ModelError},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Missing,
    Checking,
    Downloading,
    Loading,
    Cancelling,
    Cancelled,
    Ready,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Failure {
    Busy,
    Permission,
    DiskFull,
    Io,
    Size,
    Hash,
    Network,
    Tls,
    Timeout,
    Redirect,
    Http { status: u16 },
    Initialization,
    Worker,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub phase: Phase,
    pub model: ModelDefinition,
    pub received: u64,
    pub total: u64,
    pub failure: Option<Failure>,
}
#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("Model setup is busy or already ready")]
    Busy,
    #[error("Model setup has shut down")]
    Closed,
    #[error("Model setup synchronization failed")]
    Synchronization,
    #[error("Model setup worker could not start or join")]
    Worker,
}
struct State {
    closed: bool,
    snapshot: Snapshot,
    cancel: Option<CancellationToken>,
    ready: Option<InferenceService>,
}
impl State {
    fn update(&mut self, phase: Phase, received: u64, failure: Option<Failure>) {
        if self.snapshot.phase != phase
            || self.snapshot.received != received
            || self.snapshot.failure != failure
        {
            self.snapshot.revision = self.snapshot.revision.saturating_add(1);
            self.snapshot.phase = phase;
            self.snapshot.received = received;
            self.snapshot.failure = failure;
        }
    }
}
trait Backend: Send + Sync + 'static {
    fn inspect(
        &self,
        directory: &Path,
        definition: ModelDefinition,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, ModelError>;
    fn download(
        &self,
        directory: &Path,
        definition: ModelDefinition,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> Result<PathBuf, ModelError>;
    fn load(&self, path: PathBuf, definition: ModelDefinition)
    -> Result<InferenceService, Failure>;
}
struct NativeBackend;
impl Backend for NativeBackend {
    fn inspect(
        &self,
        directory: &Path,
        definition: ModelDefinition,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, ModelError> {
        model::inspect_cache(directory, definition, cancel)
    }

    fn download(
        &self,
        directory: &Path,
        definition: ModelDefinition,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> Result<PathBuf, ModelError> {
        model::download_model(directory, definition, cancel, progress)
    }
    fn load(
        &self,
        path: PathBuf,
        definition: ModelDefinition,
    ) -> Result<InferenceService, Failure> {
        InferenceService::load(path, definition).map_err(|_| Failure::Initialization)
    }
}

struct WorkerExit(Arc<Mutex<State>>);
impl Drop for WorkerExit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock()
            && state.cancel.take().is_some()
        {
            let received = state.snapshot.received;
            state.update(Phase::Failed, received, Some(Failure::Worker));
        }
    }
}

pub struct SetupService {
    directory: PathBuf,
    definition: ModelDefinition,
    backend: Arc<dyn Backend>,
    state: Arc<Mutex<State>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl SetupService {
    pub fn new(directory: PathBuf, definition: ModelDefinition) -> Self {
        Self::with_backend(directory, definition, Arc::new(NativeBackend))
    }
    fn with_backend(
        directory: PathBuf,
        definition: ModelDefinition,
        backend: Arc<dyn Backend>,
    ) -> Self {
        Self {
            directory,
            definition,
            backend,
            state: Arc::new(Mutex::new(State {
                closed: false,
                snapshot: Snapshot {
                    revision: 1,
                    phase: Phase::Missing,
                    model: definition,
                    received: 0,
                    total: definition.bytes() + model::VAD.bytes(),
                    failure: None,
                },
                cancel: None,
                ready: None,
            })),
            worker: Mutex::new(None),
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, SetupError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SetupError::Synchronization)?
            .snapshot
            .clone())
    }
    /// Check a cached model automatically; network access requires download=true.
    pub fn start(&self, download: bool) -> Result<(), SetupError> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| SetupError::Synchronization)?;
        if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
            return Err(SetupError::Busy);
        }
        if let Some(previous) = worker.take() {
            previous.join().map_err(|_| SetupError::Worker)?;
        }
        let cancel = CancellationToken::new();
        {
            let mut state = self.state.lock().map_err(|_| SetupError::Synchronization)?;
            if state.closed {
                return Err(SetupError::Closed);
            }
            if state.snapshot.phase == Phase::Ready {
                return Err(SetupError::Busy);
            }
            state.update(Phase::Checking, 0, None);
            state.cancel = Some(cancel.clone());
        }
        let state = Arc::clone(&self.state);
        let backend = Arc::clone(&self.backend);
        let directory = self.directory.clone();
        let definition = self.definition;
        *worker = Some(
            thread::Builder::new()
                .name("model-setup".into())
                .spawn(move || {
                    let _exit = WorkerExit(Arc::clone(&state));
                    run(
                        backend.as_ref(),
                        &directory,
                        definition,
                        download,
                        &cancel,
                        &state,
                    );
                })
                .map_err(|_| {
                    if let Ok(mut state) = self.state.lock() {
                        state.update(Phase::Failed, 0, Some(Failure::Worker));
                        state.cancel = None;
                    }
                    SetupError::Worker
                })?,
        );
        Ok(())
    }
    pub fn cancel(&self) -> Result<(), SetupError> {
        let mut state = self.state.lock().map_err(|_| SetupError::Synchronization)?;
        if state.closed {
            return Err(SetupError::Closed);
        }
        if let Some(cancel) = &state.cancel {
            cancel.cancel();
            let received = state.snapshot.received;
            state.update(Phase::Cancelling, received, None);
        }
        Ok(())
    }
    /// Moves the already warm serial service to the application owner exactly once.
    pub fn take_ready(&self) -> Result<Option<InferenceService>, SetupError> {
        let mut state = self.state.lock().map_err(|_| SetupError::Synchronization)?;
        if state.closed {
            return Err(SetupError::Closed);
        }
        Ok(state.ready.take())
    }
    /// Wait outside the GUI. Model initialization may have to finish before it can be discarded.
    pub fn shutdown(&self) -> Result<(), SetupError> {
        {
            let mut state = self.state.lock().map_err(|_| SetupError::Synchronization)?;
            state.closed = true;
            if let Some(cancel) = &state.cancel {
                cancel.cancel();
                let received = state.snapshot.received;
                state.update(Phase::Cancelling, received, None);
            }
        }
        let worker = self
            .worker
            .lock()
            .map_err(|_| SetupError::Synchronization)?
            .take();
        if let Some(worker) = worker {
            worker.join().map_err(|_| SetupError::Worker)?;
        }
        let ready = self
            .state
            .lock()
            .map_err(|_| SetupError::Synchronization)?
            .ready
            .take();
        if let Some(ready) = ready {
            ready.shutdown().map_err(|_| SetupError::Worker)?;
        }
        Ok(())
    }
}
impl Drop for SetupService {
    fn drop(&mut self) {
        if let Ok(state) = self.state.lock()
            && let Some(cancel) = &state.cancel
        {
            cancel.cancel();
        }
    }
}

fn failure(error: ModelError) -> Failure {
    match error {
        ModelError::Busy => Failure::Busy,
        ModelError::Io(error) => match error.kind() {
            std::io::ErrorKind::PermissionDenied => Failure::Permission,
            std::io::ErrorKind::StorageFull => Failure::DiskFull,
            _ => Failure::Io,
        },
        ModelError::Size => Failure::Size,
        ModelError::Hash => Failure::Hash,
        ModelError::Timeout => Failure::Timeout,
        ModelError::Redirect => Failure::Redirect,
        ModelError::Network => Failure::Network,
        ModelError::Tls => Failure::Tls,
        ModelError::Http(status) => Failure::Http { status },
        ModelError::Closed | ModelError::Cancelled => Failure::Io,
    }
}
fn run(
    backend: &dyn Backend,
    directory: &Path,
    definition: ModelDefinition,
    download: bool,
    cancel: &CancellationToken,
    state: &Mutex<State>,
) {
    let result = (|| {
        let mut completed = 0;
        let mut asr_path = None;
        for asset in [definition, model::VAD] {
            let cached = match backend.inspect(directory, asset, cancel) {
                Ok(path) => path,
                Err(ModelError::Hash | ModelError::Size) if download => None,
                Err(error) => return Err(failure(error)),
            };
            if cancel.is_cancelled() {
                return Ok(None);
            }
            let path = match cached {
                Some(path) => path,
                None if !download => return Ok(None),
                None => {
                    {
                        let mut shared = state.lock().map_err(|_| Failure::Worker)?;
                        if cancel.is_cancelled() {
                            return Ok(None);
                        }
                        shared.update(Phase::Downloading, completed, None);
                    }
                    backend
                        .download(directory, asset, cancel, &mut |received| {
                            if let Ok(mut shared) = state.lock()
                                && !cancel.is_cancelled()
                            {
                                // Cached/finished assets count as verified available
                                // bytes; callbacks contribute bounded current-file bytes.
                                let received = (completed + received.min(asset.bytes()))
                                    .max(shared.snapshot.received);
                                shared.update(Phase::Downloading, received, None);
                            }
                        })
                        .map_err(failure)?
                }
            };
            if cancel.is_cancelled() {
                return Ok(None);
            }
            // Both inspect and download return only size/hash-verified files.
            completed += asset.bytes();
            {
                let mut shared = state.lock().map_err(|_| Failure::Worker)?;
                if cancel.is_cancelled() {
                    return Ok(None);
                }
                let phase = shared.snapshot.phase;
                shared.update(phase, completed, None);
            }
            if asr_path.is_none() {
                asr_path = Some(path);
            }
        }
        let path = asr_path.ok_or(Failure::Initialization)?;
        {
            let mut shared = state.lock().map_err(|_| Failure::Worker)?;
            if !cancel.is_cancelled() {
                shared.update(Phase::Loading, completed, None);
            }
        }
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let service = backend.load(path, definition)?;
        loop {
            if cancel.is_cancelled() {
                service.shutdown().map_err(|_| Failure::Initialization)?;
                return Ok(None);
            }
            if service.ready() {
                return Ok(Some(service));
            }
            if service.is_finished() {
                service.shutdown().map_err(|_| Failure::Initialization)?;
                return Err(Failure::Initialization);
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    let Ok(mut shared) = state.lock() else {
        return;
    };
    let received = shared.snapshot.received;
    shared.cancel = None;
    if cancel.is_cancelled() {
        // Closing a loaded context can wait for backend initialization; do not
        // hold the snapshot mutex while its owner joins.
        let (discard, cleanup_failure) = match result {
            Ok(Some(service)) => (Some(service), None),
            Err(error @ (Failure::Initialization | Failure::Worker)) => (None, Some(error)),
            _ => (None, None),
        };
        shared.update(Phase::Cancelling, received, None);
        drop(shared);
        if let Some(service) = discard
            && service.shutdown().is_err()
        {
            if let Ok(mut shared) = state.lock() {
                shared.update(Phase::Failed, received, Some(Failure::Initialization));
            }
            return;
        }
        if let Ok(mut shared) = state.lock() {
            if let Some(error) = cleanup_failure {
                shared.update(Phase::Failed, received, Some(error));
            } else {
                shared.update(Phase::Cancelled, received, None);
            }
        }
    } else {
        match result {
            Ok(Some(service)) => {
                shared.ready = Some(service);
                shared.update(Phase::Ready, received, None);
            }
            Ok(None) => shared.update(Phase::Missing, received, None),
            Err(error) => shared.update(Phase::Failed, received, Some(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        inference_service::Decoder,
        transcription::{InferenceError, Language},
    };
    struct EmptyDecoder;
    impl Decoder for EmptyDecoder {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            Ok(String::new())
        }
    }
    struct Fixture;
    impl Backend for Fixture {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            Ok(None)
        }
        fn download(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            progress(42);
            Ok(PathBuf::from("fixture"))
        }
        fn load(&self, _: PathBuf, _: ModelDefinition) -> Result<InferenceService, Failure> {
            InferenceService::with_decoder(EmptyDecoder).map_err(|_| Failure::Initialization)
        }
    }
    fn finished(service: &SetupService) -> Result<Snapshot, Box<dyn std::error::Error>> {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = service.snapshot()?;
            if matches!(
                snapshot.phase,
                Phase::Ready | Phase::Missing | Phase::Failed | Phase::Cancelled
            ) {
                return Ok(snapshot);
            }
            if std::time::Instant::now() >= deadline {
                return Err("setup did not finish".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn explicit_download_loads_and_transfers_one_warm_service()
    -> Result<(), Box<dyn std::error::Error>> {
        let setup = SetupService::with_backend(
            PathBuf::from("fixture"),
            model::REFERENCE,
            Arc::new(Fixture),
        );
        setup.start(true)?;
        let snapshot = finished(&setup)?;
        assert_eq!(snapshot.phase, Phase::Ready);
        assert_eq!(
            snapshot.received,
            model::REFERENCE.bytes() + model::VAD.bytes()
        );
        let ready = setup.take_ready()?.ok_or("warm service missing")?;
        assert!(ready.ready());
        assert!(setup.take_ready()?.is_none());
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        ready.shutdown()?;
        setup.shutdown()?;
        Ok(())
    }
    #[test]
    fn cache_check_never_downloads_implicitly() -> Result<(), Box<dyn std::error::Error>> {
        let setup = SetupService::with_backend(
            PathBuf::from("fixture"),
            model::REFERENCE,
            Arc::new(Fixture),
        );
        setup.start(false)?;
        assert_eq!(finished(&setup)?.phase, Phase::Missing);
        assert!(setup.take_ready()?.is_none());
        setup.shutdown()?;
        Ok(())
    }

    struct Controlled {
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
    }
    impl Backend for Controlled {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            Ok(None)
        }
        fn download(
            &self,
            directory: &Path,
            definition: ModelDefinition,
            cancel: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            if definition.id() == model::VAD.id() {
                return Fixture.download(directory, definition, cancel, progress);
            }
            progress(7);
            self.entered.send(()).map_err(|_| ModelError::Closed)?;
            loop {
                if cancel.is_cancelled() {
                    return Err(ModelError::Cancelled);
                }
                if self.release.recv_timeout(Duration::from_millis(1)).is_ok() {
                    return Ok(PathBuf::from("fixture"));
                }
            }
        }
        fn load(
            &self,
            path: PathBuf,
            definition: ModelDefinition,
        ) -> Result<InferenceService, Failure> {
            Fixture.load(path, definition)
        }
    }
    #[test]
    fn cancel_releases_setup_and_retry_cannot_run_two_downloads()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, began) = crossbeam_channel::bounded(1);
        let (release, waiting) = crossbeam_channel::bounded(1);
        let setup = SetupService::with_backend(
            PathBuf::from("fixture"),
            model::REFERENCE,
            Arc::new(Controlled {
                entered,
                release: waiting,
            }),
        );
        setup.start(true)?;
        began.recv_timeout(Duration::from_secs(1))?;
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        assert_eq!(setup.snapshot()?.received, 7);
        setup.cancel()?;
        assert_eq!(finished(&setup)?.phase, Phase::Cancelled);
        assert!(setup.take_ready()?.is_none());
        release.send(())?;
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            match setup.start(true) {
                Ok(()) => break,
                Err(SetupError::Busy) if std::time::Instant::now() < deadline => {
                    thread::yield_now()
                }
                Err(error) => return Err(error.into()),
            }
        }
        began.recv_timeout(Duration::from_secs(1))?;
        assert_eq!(finished(&setup)?.phase, Phase::Ready);
        setup.shutdown()?;
        Ok(())
    }
    struct BrokenOnCancel {
        cancel: Mutex<Option<CancellationToken>>,
        path: PathBuf,
    }
    impl Backend for BrokenOnCancel {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            cancel: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            if let Ok(mut token) = self.cancel.lock() {
                *token = Some(cancel.clone());
            }
            Ok(Some(self.path.clone()))
        }
        fn download(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
            _: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            Err(ModelError::Closed)
        }
        fn load(
            &self,
            path: PathBuf,
            definition: ModelDefinition,
        ) -> Result<InferenceService, Failure> {
            let service =
                InferenceService::load(path, definition).map_err(|_| Failure::Initialization)?;
            if let Ok(cancel) = self.cancel.lock()
                && let Some(cancel) = cancel.as_ref()
            {
                cancel.cancel();
            }
            Ok(service)
        }
    }
    #[test]
    fn failed_worker_shutdown_is_not_hidden_by_cancel() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let backend = BrokenOnCancel {
            cancel: Mutex::new(None),
            path: directory.path().join("missing.bin"),
        };
        let setup = SetupService::with_backend(
            directory.path().to_owned(),
            model::REFERENCE,
            Arc::new(backend),
        );
        setup.start(false)?;
        let state = finished(&setup)?;
        assert_eq!(state.phase, Phase::Failed);
        assert_eq!(state.failure, Some(Failure::Initialization));
        setup.shutdown()?;
        Ok(())
    }
    struct InvalidCache;
    impl Backend for InvalidCache {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            Err(ModelError::Hash)
        }
        fn download(
            &self,
            directory: &Path,
            definition: ModelDefinition,
            cancel: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            Fixture.download(directory, definition, cancel, progress)
        }
        fn load(
            &self,
            path: PathBuf,
            definition: ModelDefinition,
        ) -> Result<InferenceService, Failure> {
            Fixture.load(path, definition)
        }
    }
    #[test]
    fn invalid_cache_requires_explicit_repair_and_shutdown_rejects_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let setup = SetupService::with_backend(
            PathBuf::from("fixture"),
            model::REFERENCE,
            Arc::new(InvalidCache),
        );
        setup.start(false)?;
        let state = finished(&setup)?;
        assert_eq!(state.phase, Phase::Failed);
        assert_eq!(state.failure, Some(Failure::Hash));
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            match setup.start(true) {
                Ok(()) => break,
                Err(SetupError::Busy) if std::time::Instant::now() < deadline => {
                    thread::yield_now()
                }
                Err(error) => return Err(error.into()),
            }
        }
        assert_eq!(finished(&setup)?.phase, Phase::Ready);
        setup.shutdown()?;
        assert!(matches!(setup.start(true), Err(SetupError::Closed)));
        assert!(matches!(setup.take_ready(), Err(SetupError::Closed)));
        Ok(())
    }

    struct FailingBackend {
        error: Mutex<Option<ModelError>>,
        in_download: bool,
        inspections: std::sync::atomic::AtomicUsize,
        downloads: std::sync::atomic::AtomicUsize,
        loads: std::sync::atomic::AtomicUsize,
    }
    impl FailingBackend {
        fn failure(&self) -> ModelError {
            self.error
                .lock()
                .ok()
                .and_then(|mut error| error.take())
                .unwrap_or(ModelError::Closed)
        }
    }
    impl Backend for FailingBackend {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            self.inspections
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.in_download {
                Ok(None)
            } else {
                Err(self.failure())
            }
        }
        fn download(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            self.downloads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            progress(123);
            Err(self.failure())
        }
        fn load(&self, _: PathBuf, _: ModelDefinition) -> Result<InferenceService, Failure> {
            self.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Failure::Initialization)
        }
    }

    #[test]
    fn cache_and_download_failures_publish_classified_terminal_state_without_retry_or_load()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            io::{Error, ErrorKind},
            sync::atomic::{AtomicUsize, Ordering},
        };
        for in_download in [false, true] {
            for (error, expected) in [
                (
                    ModelError::Io(Error::from(ErrorKind::PermissionDenied)),
                    Failure::Permission,
                ),
                (
                    ModelError::Io(Error::from(ErrorKind::StorageFull)),
                    Failure::DiskFull,
                ),
                (
                    ModelError::Io(Error::other("controlled I/O failure")),
                    Failure::Io,
                ),
                (ModelError::Size, Failure::Size),
                (ModelError::Hash, Failure::Hash),
                (ModelError::Timeout, Failure::Timeout),
                (ModelError::Redirect, Failure::Redirect),
                (ModelError::Busy, Failure::Busy),
                (ModelError::Network, Failure::Network),
                (ModelError::Tls, Failure::Tls),
                (ModelError::Http(503), Failure::Http { status: 503 }),
            ] {
                let backend = Arc::new(FailingBackend {
                    error: Mutex::new(Some(error)),
                    in_download,
                    inspections: AtomicUsize::new(0),
                    downloads: AtomicUsize::new(0),
                    loads: AtomicUsize::new(0),
                });
                let setup = SetupService::with_backend(
                    PathBuf::from("controlled-failure"),
                    model::REFERENCE,
                    backend.clone(),
                );
                setup.start(in_download)?;
                let state = finished(&setup)?;
                assert_eq!(
                    state.phase,
                    Phase::Failed,
                    "{expected:?}, download={in_download}"
                );
                assert_eq!(state.failure, Some(expected));
                assert_eq!(state.received, if in_download { 123 } else { 0 });
                assert_eq!(state.total, model::REFERENCE.bytes() + model::VAD.bytes());
                assert!(state.revision > 1);
                assert!(setup.take_ready()?.is_none());
                for _ in 0..3 {
                    let observed = setup.snapshot()?;
                    assert_eq!(observed.phase, Phase::Failed);
                    assert_eq!(observed.revision, state.revision);
                }
                setup.shutdown()?;
                assert_eq!(backend.inspections.load(Ordering::SeqCst), 1);
                assert_eq!(
                    backend.downloads.load(Ordering::SeqCst),
                    usize::from(in_download)
                );
                assert_eq!(backend.loads.load(Ordering::SeqCst), 0);
            }
        }
        Ok(())
    }

    struct RetryBackend {
        inspect_gate: std::sync::atomic::AtomicBool,
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
        inspections: std::sync::atomic::AtomicUsize,
        downloads: std::sync::atomic::AtomicUsize,
        loads: std::sync::atomic::AtomicUsize,
    }
    impl Backend for RetryBackend {
        fn inspect(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            use std::sync::atomic::Ordering;
            self.inspections.fetch_add(1, Ordering::SeqCst);
            if self.inspect_gate.swap(false, Ordering::AcqRel) {
                self.entered.send(()).map_err(|_| ModelError::Closed)?;
                self.release
                    .recv_timeout(Duration::from_secs(2))
                    .map_err(|_| ModelError::Closed)?;
            }
            Ok(None)
        }
        fn download(
            &self,
            _: &Path,
            _: ModelDefinition,
            _: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            progress(42);
            if self
                .downloads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                == 0
            {
                Err(ModelError::Timeout)
            } else {
                Ok(PathBuf::from("retried-fixture"))
            }
        }
        fn load(
            &self,
            path: PathBuf,
            definition: ModelDefinition,
        ) -> Result<InferenceService, Failure> {
            self.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Fixture.load(path, definition)
        }
    }

    fn start_after_previous_worker(
        setup: &SetupService,
        download: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match setup.start(download) {
                Ok(()) => return Ok(()),
                Err(SetupError::Busy) if std::time::Instant::now() < deadline => {
                    thread::yield_now()
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    #[test]
    fn failed_download_waits_for_explicit_retry_and_clears_failure_before_loading_once()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let (entered, began) = crossbeam_channel::bounded(1);
        let (release, wait) = crossbeam_channel::bounded(1);
        let backend = Arc::new(RetryBackend {
            inspect_gate: AtomicBool::new(false),
            entered,
            release: wait,
            inspections: AtomicUsize::new(0),
            downloads: AtomicUsize::new(0),
            loads: AtomicUsize::new(0),
        });
        let setup = SetupService::with_backend(
            PathBuf::from("controlled-retry"),
            model::REFERENCE,
            backend.clone(),
        );
        setup.start(false)?;
        assert_eq!(finished(&setup)?.phase, Phase::Missing);
        assert_eq!(backend.downloads.load(Ordering::SeqCst), 0);
        start_after_previous_worker(&setup, true)?;
        let failed = finished(&setup)?;
        assert_eq!(failed.phase, Phase::Failed);
        assert_eq!(failed.failure, Some(Failure::Timeout));
        assert_eq!(failed.received, 42);
        assert_eq!(backend.downloads.load(Ordering::SeqCst), 1);
        assert_eq!(backend.loads.load(Ordering::SeqCst), 0);
        assert!(setup.take_ready()?.is_none());

        backend.inspect_gate.store(true, Ordering::Release);
        start_after_previous_worker(&setup, true)?;
        began.recv_timeout(Duration::from_secs(1))?;
        let checking = setup.snapshot()?;
        assert_eq!(checking.phase, Phase::Checking);
        assert_eq!(checking.failure, None);
        assert_eq!(checking.received, 0);
        assert!(checking.revision > failed.revision);
        assert_eq!(backend.downloads.load(Ordering::SeqCst), 1);
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        release.send(())?;

        let ready = finished(&setup)?;
        assert_eq!(ready.phase, Phase::Ready);
        assert_eq!(ready.failure, None);
        assert_eq!(
            ready.received,
            model::REFERENCE.bytes() + model::VAD.bytes()
        );
        assert_eq!(backend.inspections.load(Ordering::SeqCst), 4);
        assert_eq!(backend.downloads.load(Ordering::SeqCst), 3);
        assert_eq!(backend.loads.load(Ordering::SeqCst), 1);
        let service = setup
            .take_ready()?
            .ok_or("warm service missing after retry")?;
        assert!(service.ready());
        assert!(setup.take_ready()?.is_none());
        service.shutdown()?;
        setup.shutdown()?;
        Ok(())
    }
    struct AssetBackend {
        vad_cached: bool,
        vad_corrupt: bool,
        fail_download: bool,
        calls: Mutex<Vec<String>>,
    }
    impl Backend for AssetBackend {
        fn inspect(
            &self,
            directory: &Path,
            definition: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            self.calls
                .lock()
                .map_err(|_| ModelError::Closed)?
                .push(format!("inspect:{}", definition.id()));
            if definition.id() == model::VAD.id() {
                if self.vad_corrupt {
                    return Err(ModelError::Hash);
                }
                if !self.vad_cached {
                    return Ok(None);
                }
            }
            Ok(Some(directory.join(definition.file_name())))
        }
        fn download(
            &self,
            directory: &Path,
            definition: ModelDefinition,
            _: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            self.calls
                .lock()
                .map_err(|_| ModelError::Closed)?
                .push(format!("download:{}", definition.id()));
            progress(17);
            if self.fail_download {
                return Err(ModelError::Network);
            }
            progress(definition.bytes());
            Ok(directory.join(definition.file_name()))
        }
        fn load(
            &self,
            path: PathBuf,
            definition: ModelDefinition,
        ) -> Result<InferenceService, Failure> {
            self.calls
                .lock()
                .map_err(|_| Failure::Worker)?
                .push(format!("load:{}", definition.id()));
            Fixture.load(path, definition)
        }
    }
    #[test]
    fn asr_cache_alone_is_not_ready_and_bad_vad_does_not_fall_back()
    -> Result<(), Box<dyn std::error::Error>> {
        for corrupt in [false, true] {
            let backend = Arc::new(AssetBackend {
                vad_cached: false,
                vad_corrupt: corrupt,
                fail_download: false,
                calls: Mutex::new(Vec::new()),
            });
            let setup =
                SetupService::with_backend(PathBuf::from("assets"), model::SMALL, backend.clone());
            setup.start(false)?;
            let snapshot = finished(&setup)?;
            assert_eq!(
                snapshot.phase,
                if corrupt {
                    Phase::Failed
                } else {
                    Phase::Missing
                }
            );
            assert_eq!(snapshot.failure, corrupt.then_some(Failure::Hash));
            assert_eq!(snapshot.total, model::SMALL.bytes() + model::VAD.bytes());
            assert_eq!(snapshot.received, model::SMALL.bytes());
            assert_eq!(snapshot.model.id(), model::SMALL.id());
            assert!(setup.take_ready()?.is_none());
            assert_eq!(
                *backend.calls.lock().map_err(|_| "calls")?,
                ["inspect:small-q5_1", "inspect:silero-v5.1.2"]
            );
            setup.shutdown()?;
        }
        Ok(())
    }
    #[test]
    fn both_assets_are_required_and_only_missing_or_corrupt_vad_is_downloaded()
    -> Result<(), Box<dyn std::error::Error>> {
        for (cached, corrupt) in [(true, false), (false, false), (true, true)] {
            let backend = Arc::new(AssetBackend {
                vad_cached: cached,
                vad_corrupt: corrupt,
                fail_download: false,
                calls: Mutex::new(Vec::new()),
            });
            let setup = SetupService::with_backend(
                PathBuf::from("assets"),
                model::REFERENCE,
                backend.clone(),
            );
            setup.start(!cached || corrupt)?;
            let snapshot = finished(&setup)?;
            assert_eq!(snapshot.phase, Phase::Ready);
            assert_eq!(
                snapshot.received,
                model::REFERENCE.bytes() + model::VAD.bytes()
            );
            assert_eq!(snapshot.total, snapshot.received);
            let mut expected = vec!["inspect:large-v3-turbo-q5_0", "inspect:silero-v5.1.2"];
            if !cached || corrupt {
                expected.push("download:silero-v5.1.2");
            }
            expected.push("load:large-v3-turbo-q5_0");
            assert_eq!(*backend.calls.lock().map_err(|_| "calls")?, expected);
            setup.take_ready()?.ok_or("ready")?.shutdown()?;
            setup.shutdown()?;
        }
        Ok(())
    }
    #[test]
    fn failed_second_asset_keeps_cumulative_bytes_and_never_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let backend = Arc::new(AssetBackend {
            vad_cached: false,
            vad_corrupt: false,
            fail_download: true,
            calls: Mutex::new(Vec::new()),
        });
        let setup =
            SetupService::with_backend(PathBuf::from("assets"), model::REFERENCE, backend.clone());
        setup.start(true)?;
        let snapshot = finished(&setup)?;
        assert_eq!(snapshot.phase, Phase::Failed);
        assert_eq!(snapshot.failure, Some(Failure::Network));
        assert_eq!(snapshot.received, model::REFERENCE.bytes() + 17);
        assert_eq!(
            *backend.calls.lock().map_err(|_| "calls")?,
            [
                "inspect:large-v3-turbo-q5_0",
                "inspect:silero-v5.1.2",
                "download:silero-v5.1.2"
            ]
        );
        assert!(setup.take_ready()?.is_none());
        setup.shutdown()?;
        Ok(())
    }

    struct PendingVad {
        entered: crossbeam_channel::Sender<()>,
        loads: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Backend for PendingVad {
        fn inspect(
            &self,
            directory: &Path,
            definition: ModelDefinition,
            _: &CancellationToken,
        ) -> Result<Option<PathBuf>, ModelError> {
            Ok(
                (definition.id() != model::VAD.id())
                    .then(|| directory.join(definition.file_name())),
            )
        }
        fn download(
            &self,
            _: &Path,
            _: ModelDefinition,
            cancel: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError> {
            progress(17);
            progress(3); // A regressing callback cannot rewind visible progress.
            self.entered.send(()).map_err(|_| ModelError::Closed)?;
            while !cancel.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            Err(ModelError::Cancelled)
        }
        fn load(&self, _: PathBuf, _: ModelDefinition) -> Result<InferenceService, Failure> {
            self.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Failure::Initialization)
        }
    }
    #[test]
    fn pending_vad_reports_monotone_cumulative_progress_and_cancel_never_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, began) = crossbeam_channel::bounded(1);
        let loads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let setup = SetupService::with_backend(
            PathBuf::from("assets"),
            model::REFERENCE,
            Arc::new(PendingVad {
                entered,
                loads: Arc::clone(&loads),
            }),
        );
        setup.start(true)?;
        began.recv_timeout(Duration::from_secs(1))?;
        let snapshot = setup.snapshot()?;
        assert_eq!(snapshot.phase, Phase::Downloading);
        assert_eq!(snapshot.received, model::REFERENCE.bytes() + 17);
        assert_eq!(
            snapshot.total,
            model::REFERENCE.bytes() + model::VAD.bytes()
        );
        assert!(setup.take_ready()?.is_none());
        setup.cancel()?;
        let cancelled = finished(&setup)?;
        assert_eq!(cancelled.phase, Phase::Cancelled);
        assert_eq!(cancelled.received, snapshot.received);
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(setup.take_ready()?.is_none());
        setup.shutdown()?;
        Ok(())
    }
}
