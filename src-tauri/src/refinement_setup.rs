//! Complete text-model installation on a separate I/O owner; no GPU loading.
use crate::{
    model::{self, refinement::RefinementModel},
    setup::{Failure, Phase, SetupError},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub phase: Phase,
    pub received: u64,
    pub total: u64,
    pub failure: Option<Failure>,
}
struct State {
    closed: bool,
    cancel: Option<CancellationToken>,
    snapshot: Snapshot,
}
impl State {
    fn update(&mut self, phase: Phase, received: u64, failure: Option<Failure>) {
        self.snapshot.revision = self.snapshot.revision.saturating_add(1);
        self.snapshot.phase = phase;
        self.snapshot.received = received.min(self.snapshot.total);
        self.snapshot.failure = failure;
    }
}
trait Backend: Send + Sync + 'static {
    fn inspect(
        &self,
        models: &Path,
        cancel: &CancellationToken,
    ) -> Result<bool, model::refinement::Error>;
    fn install(
        &self,
        models: &Path,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), model::refinement::Error>;
}
struct NativeBackend(RefinementModel);
impl Backend for NativeBackend {
    fn inspect(
        &self,
        models: &Path,
        cancel: &CancellationToken,
    ) -> Result<bool, model::refinement::Error> {
        self.0.inspect(models, cancel).map(|path| path.is_some())
    }
    fn install(
        &self,
        models: &Path,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), model::refinement::Error> {
        self.0
            .install(models, cancel, |received, _| progress(received))
            .map(|_| ())
    }
}
pub struct Setup {
    models: PathBuf,
    backend: Arc<dyn Backend>,
    state: Arc<Mutex<State>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Setup {
    pub fn new(models: PathBuf) -> Result<Self, model::refinement::Error> {
        let model = RefinementModel::bundled()?;
        let total = model.installation_bytes();
        Ok(Self::with_backend(
            models,
            total,
            Arc::new(NativeBackend(model)),
        ))
    }
    fn with_backend(models: PathBuf, total: u64, backend: Arc<dyn Backend>) -> Self {
        Self {
            models,
            backend,
            state: Arc::new(Mutex::new(State {
                closed: false,
                cancel: None,
                snapshot: Snapshot {
                    revision: 1,
                    phase: Phase::Missing,
                    received: 0,
                    total,
                    failure: None,
                },
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
    /// A local inspection never downloads. Only explicit download=true may repair
    /// missing/corrupt assets. Ready means verified files, not a loaded GPU model.
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
            state.update(Phase::Checking, 0, None);
            state.cancel = Some(cancel.clone());
        }
        let state = self.state.clone();
        let backend = self.backend.clone();
        let models = self.models.clone();
        *worker = Some(
            thread::Builder::new()
                .name("text-model-setup".into())
                .spawn(move || {
                    let _exit = Exit(state.clone());
                    let result = run(backend.as_ref(), &models, download, &cancel, &state);
                    if let Ok(mut state) = state.lock() {
                        let total = state.snapshot.total;
                        let received = state.snapshot.received;
                        match result {
                            Ok(_) if cancel.is_cancelled() => {
                                state.update(Phase::Cancelled, received, None)
                            }
                            Ok(true) => state.update(Phase::Ready, total, None),
                            Ok(false) => state.update(Phase::Missing, 0, None),
                            Err(model::refinement::Error::Model(model::ModelError::Cancelled)) => {
                                state.update(Phase::Cancelled, received, None)
                            }
                            Err(error) => {
                                state.update(Phase::Failed, received, Some(failure(error)))
                            }
                        }
                        state.cancel = None;
                    }
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
    /// Called on the application owner, never the GUI thread.
    pub fn shutdown(&self) -> Result<(), SetupError> {
        {
            let mut state = self.state.lock().map_err(|_| SetupError::Synchronization)?;
            state.closed = true;
            if let Some(cancel) = &state.cancel {
                cancel.cancel();
            }
        }
        if let Some(worker) = self
            .worker
            .lock()
            .map_err(|_| SetupError::Synchronization)?
            .take()
        {
            worker.join().map_err(|_| SetupError::Worker)?;
        }
        Ok(())
    }
}
impl Drop for Setup {
    fn drop(&mut self) {
        if let Ok(state) = self.state.lock()
            && let Some(cancel) = &state.cancel
        {
            cancel.cancel();
        }
        // Explicit application shutdown owns joining; dropping never blocks a UI callback.
    }
}
struct Exit(Arc<Mutex<State>>);
impl Drop for Exit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock()
            && state.cancel.take().is_some()
        {
            let received = state.snapshot.received;
            state.update(Phase::Failed, received, Some(Failure::Worker));
        }
    }
}
fn run(
    backend: &dyn Backend,
    models: &Path,
    download: bool,
    cancel: &CancellationToken,
    state: &Mutex<State>,
) -> Result<bool, model::refinement::Error> {
    let cached = backend.inspect(models, cancel);
    match cached {
        Ok(true) => return Ok(true),
        Ok(false) if !download => return Ok(false),
        Err(error)
            if !download
                || !matches!(
                    error,
                    model::refinement::Error::IncompleteModel
                        | model::refinement::Error::Model(
                            model::ModelError::Size | model::ModelError::Hash
                        )
                ) =>
        {
            return Err(error);
        }
        _ => {}
    }
    if cancel.is_cancelled() {
        return Err(model::ModelError::Cancelled.into());
    }
    if let Ok(mut state) = state.lock() {
        state.update(Phase::Downloading, 0, None);
    }
    backend.install(models, cancel, &mut |received| {
        if let Ok(mut state) = state.lock()
            && !cancel.is_cancelled()
        {
            state.update(Phase::Downloading, received, None);
        }
    })?;
    Ok(true)
}
fn failure(error: model::refinement::Error) -> Failure {
    use model::refinement::Error;
    match error {
        Error::Model(error) => crate::setup::failure(error),
        Error::Io(error) => crate::setup::failure(model::ModelError::Io(error)),
        Error::InsufficientSpace { .. } => Failure::DiskFull,
        Error::IncompleteModel => Failure::Hash,
        Error::InvalidCache | Error::PublicationRecovery { .. } => Failure::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    struct Controlled {
        cached: AtomicBool,
        installs: AtomicUsize,
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
    }
    impl Backend for Controlled {
        fn inspect(
            &self,
            _: &Path,
            _: &CancellationToken,
        ) -> Result<bool, model::refinement::Error> {
            Ok(self.cached.load(Ordering::Acquire))
        }
        fn install(
            &self,
            _: &Path,
            cancel: &CancellationToken,
            progress: &mut dyn FnMut(u64),
        ) -> Result<(), model::refinement::Error> {
            self.installs.fetch_add(1, Ordering::AcqRel);
            progress(4);
            self.entered
                .send(())
                .map_err(|_| model::ModelError::Closed)?;
            self.release
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| model::ModelError::Timeout)?;
            if cancel.is_cancelled() {
                return Err(model::ModelError::Cancelled.into());
            }
            self.cached.store(true, Ordering::Release);
            progress(12);
            Ok(())
        }
    }
    fn controlled(
        cached: bool,
    ) -> (
        Setup,
        Arc<Controlled>,
        crossbeam_channel::Receiver<()>,
        crossbeam_channel::Sender<()>,
    ) {
        let (entered, seen) = crossbeam_channel::bounded(1);
        let (release, wait) = crossbeam_channel::bounded(1);
        let backend = Arc::new(Controlled {
            cached: AtomicBool::new(cached),
            installs: AtomicUsize::new(0),
            entered,
            release: wait,
        });
        (
            Setup::with_backend(PathBuf::from("/unused-test-cache"), 10, backend.clone()),
            backend,
            seen,
            release,
        )
    }
    fn settled(setup: &Setup) -> Result<Snapshot, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let snapshot = setup.snapshot()?;
            if !matches!(
                snapshot.phase,
                Phase::Checking | Phase::Downloading | Phase::Cancelling
            ) {
                return Ok(snapshot);
            }
            if Instant::now() > deadline {
                return Err("setup did not settle".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn start_after_finish(setup: &Setup, download: bool) -> TestResult {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match setup.start(download) {
                Ok(()) => return Ok(()),
                Err(SetupError::Busy) if Instant::now() < deadline => thread::yield_now(),
                Err(error) => return Err(error.into()),
            }
        }
    }
    #[test]
    fn cache_checks_never_download_and_ready_only_means_verified_assets() -> TestResult {
        for cached in [false, true] {
            let (setup, backend, _, _) = controlled(cached);
            setup.start(false)?;
            let snapshot = settled(&setup)?;
            assert_eq!(
                snapshot.phase,
                if cached { Phase::Ready } else { Phase::Missing }
            );
            assert_eq!(snapshot.received, if cached { 10 } else { 0 });
            assert_eq!(backend.installs.load(Ordering::Acquire), 0);
            setup.shutdown()?;
        }
        Ok(())
    }
    #[test]
    fn cancel_keeps_setup_busy_until_io_ends_and_retry_has_one_owner() -> TestResult {
        let (setup, backend, entered, release) = controlled(false);
        setup.start(true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(setup.snapshot()?.received, 4);
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        setup.cancel()?;
        assert_eq!(setup.snapshot()?.phase, Phase::Cancelling);
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        release.send(())?;
        assert_eq!(settled(&setup)?.phase, Phase::Cancelled);
        start_after_finish(&setup, true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        release.send(())?;
        let snapshot = settled(&setup)?;
        assert_eq!(snapshot.phase, Phase::Ready);
        assert_eq!(snapshot.received, 10);
        assert_eq!(backend.installs.load(Ordering::Acquire), 2);
        setup.shutdown()?;
        assert!(matches!(setup.start(true), Err(SetupError::Closed)));
        Ok(())
    }
    #[test]
    fn native_missing_cache_inspection_creates_no_download_or_directory() -> TestResult {
        let root = tempfile::tempdir()?;
        let models = root.path().join("absent");
        let setup = Setup::new(models.clone())?;
        assert_eq!(setup.snapshot()?.total, 2_278_970_666);
        setup.start(false)?;
        assert_eq!(settled(&setup)?.phase, Phase::Missing);
        assert!(!models.exists());
        setup.shutdown()?;
        Ok(())
    }
    #[test]
    fn shutdown_joins_pending_io_and_does_not_publish_ready_after_revocation() -> TestResult {
        let (setup, _, entered, release) = controlled(false);
        let setup = Arc::new(setup);
        setup.start(true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        // Cancel is the synchronous authority revocation; shutdown then joins
        // outside the caller's UI thread even if I/O has not returned yet.
        setup.cancel()?;
        let owned = setup.clone();
        let (finished, result) = crossbeam_channel::bounded(1);
        let join = thread::spawn(move || {
            if finished.send(owned.shutdown()).is_err() { /* observer gone */ }
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match setup.cancel() {
                Err(SetupError::Closed) => break,
                Ok(()) if Instant::now() < deadline => thread::yield_now(),
                _ => return Err("shutdown did not close admission".into()),
            }
        }
        assert!(matches!(
            result.try_recv(),
            Err(crossbeam_channel::TryRecvError::Empty)
        ));
        release.send(())?;
        result.recv_timeout(Duration::from_secs(2))??;
        join.join().map_err(|_| "shutdown thread panicked")?;
        assert_eq!(setup.snapshot()?.phase, Phase::Cancelled);
        assert!(matches!(setup.start(false), Err(SetupError::Closed)));
        Ok(())
    }
}
