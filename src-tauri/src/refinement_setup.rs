//! Text-model installation and explicit startup probing on a separate owner.
use crate::{
    model::{self, refinement::RefinementModel},
    refinement_service::{self, Worker, WorkerSpec},
    setup::{Failure, Phase, SetupError},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use tokio_util::sync::CancellationToken;

const PROBE_INPUT: &str = "Ähm, wir schicken den Bericht morgen.";
const PROBE_OUTPUT: &str = "Wir schicken den Bericht morgen.";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub phase: Phase,
    pub probe_passed: bool,
    pub received: u64,
    pub total: u64,
    pub failure: Option<Failure>,
}
impl Snapshot {
    pub fn is_busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Checking | Phase::Downloading | Phase::Loading | Phase::Cancelling
        )
    }
}
struct State {
    closed: bool,
    cancel: Option<Cancellation>,
    snapshot: Snapshot,
}
#[derive(Clone, Default)]
struct Cancellation {
    download: CancellationToken,
    worker: refinement_service::Cancellation,
}
impl Cancellation {
    fn cancel(&self) {
        self.worker.cancel();
        self.download.cancel();
    }
    fn is_cancelled(&self) -> bool {
        self.download.is_cancelled()
    }
}

enum Error {
    Model(model::refinement::Error),
    Worker(refinement_service::Error),
}
impl From<model::refinement::Error> for Error {
    fn from(error: model::refinement::Error) -> Self {
        Self::Model(error)
    }
}
impl From<refinement_service::Error> for Error {
    fn from(error: refinement_service::Error) -> Self {
        Self::Worker(error)
    }
}
impl State {
    fn update(&mut self, phase: Phase, received: u64, failure: Option<Failure>) {
        self.snapshot.revision = self.snapshot.revision.saturating_add(1);
        self.snapshot.phase = phase;
        if phase != Phase::Ready {
            self.snapshot.probe_passed = false;
        }
        self.snapshot.received = received.min(self.snapshot.total);
        self.snapshot.failure = failure;
    }
}
trait Backend: Send + Sync + 'static {
    fn probe(
        &self,
        cancel: &crate::refinement_service::Cancellation,
    ) -> Result<(), crate::refinement_service::Error>;
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
struct NativeBackend {
    model: RefinementModel,
    worker: WorkerSpec,
}
impl Backend for NativeBackend {
    fn probe(
        &self,
        cancel: &refinement_service::Cancellation,
    ) -> Result<(), refinement_service::Error> {
        refinement_service::scratch::prepare(&self.worker.runtime_directory)
            .and_then(|()| refinement_service::scratch::recover(&self.worker.runtime_directory))
            .map_err(|_| refinement_service::Error::from(refinement_service::Failure::Spawn))?;
        let mut worker = Worker::spawn(
            self.worker.clone(),
            refinement_service::Limits::default(),
            cancel,
        )?;
        let original = dictation_core::refinement::OriginalTranscript::new(PROBE_INPUT.into())
            .map_err(|_| refinement_service::Failure::InvalidRequest)?;
        let result = worker
            .run(
                text_refinement_protocol::JobKey {
                    session: std::num::NonZeroU64::MIN,
                    generation: self.worker.identity.generation,
                    request: std::num::NonZeroU64::MIN,
                },
                &original,
                text_refinement_protocol::Language::German,
                cancel,
            )
            .and_then(|output| {
                if output.as_str() == PROBE_OUTPUT {
                    Ok(())
                } else {
                    Err(refinement_service::Failure::InvalidCandidate.into())
                }
            });
        let cleanup = worker.close();
        if let Err(mut error) = result {
            error.cleanup_failed |= cleanup.is_err();
            return Err(error);
        }
        let termination = cleanup?;
        if !termination.forced && !termination.status.success() {
            return Err(refinement_service::Failure::Exited.into());
        }
        Ok(())
    }
    fn inspect(
        &self,
        models: &Path,
        cancel: &CancellationToken,
    ) -> Result<bool, model::refinement::Error> {
        self.model
            .inspect(models, cancel)
            .map(|path| path.is_some())
    }
    fn install(
        &self,
        models: &Path,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), model::refinement::Error> {
        self.model
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
    pub fn new(models: PathBuf, worker: WorkerSpec) -> Result<Self, model::refinement::Error> {
        let model = RefinementModel::bundled()?;
        let total = model.installation_bytes();
        Ok(Self::with_backend(
            models,
            total,
            Arc::new(NativeBackend { model, worker }),
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
                    probe_passed: false,
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
    /// missing/corrupt assets and probe GPU startup. Startup inspection stays lazy.
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
        let cancel = Cancellation::default();
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
                            Ok(true) => {
                                state.snapshot.probe_passed = download;
                                state.update(Phase::Ready, total, None);
                            }
                            Ok(false) => state.update(Phase::Missing, 0, None),
                            Err(Error::Model(model::refinement::Error::Model(
                                model::ModelError::Cancelled,
                            ))) => state.update(Phase::Cancelled, received, None),
                            Err(Error::Worker(error))
                                if error.reason == refinement_service::Failure::Cancelled
                                    && !error.cleanup_failed =>
                            {
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
    cancel: &Cancellation,
    state: &Mutex<State>,
) -> Result<bool, Error> {
    let cached = backend.inspect(models, &cancel.download);
    match cached {
        Ok(true) => {}
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
            return Err(error.into());
        }
        _ => {
            if cancel.is_cancelled() {
                return Err(model::refinement::Error::from(model::ModelError::Cancelled).into());
            }
            if let Ok(mut state) = state.lock() {
                state.update(Phase::Downloading, 0, None);
            }
            backend.install(models, &cancel.download, &mut |received| {
                if let Ok(mut state) = state.lock()
                    && !cancel.is_cancelled()
                {
                    state.update(Phase::Downloading, received, None);
                }
            })?;
        }
    }
    if download {
        if cancel.is_cancelled() {
            return Err(model::refinement::Error::from(model::ModelError::Cancelled).into());
        }
        if let Ok(mut state) = state.lock() {
            let total = state.snapshot.total;
            state.update(Phase::Loading, total, None);
        }
        backend.probe(&cancel.worker)?;
    }
    Ok(true)
}
fn failure(error: Error) -> Failure {
    match error {
        Error::Model(error) => model_failure(error),
        Error::Worker(error) if error.cleanup_failed => Failure::Worker,
        Error::Worker(error) if error.reason == refinement_service::Failure::StartupTimeout => {
            Failure::Timeout
        }
        Error::Worker(_) => Failure::Initialization,
    }
}
fn model_failure(error: model::refinement::Error) -> Failure {
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
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    struct Controlled {
        cached: AtomicBool,
        damaged: AtomicBool,
        installs: AtomicUsize,
        probes: AtomicUsize,
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
    }
    impl Backend for Controlled {
        fn probe(
            &self,
            _: &crate::refinement_service::Cancellation,
        ) -> Result<(), crate::refinement_service::Error> {
            self.probes.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }
        fn inspect(
            &self,
            _: &Path,
            _: &CancellationToken,
        ) -> Result<bool, model::refinement::Error> {
            if self.damaged.load(Ordering::Acquire) {
                return Err(model::refinement::Error::IncompleteModel);
            }
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
            self.damaged.store(false, Ordering::Release);
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
            damaged: AtomicBool::new(false),
            installs: AtomicUsize::new(0),
            probes: AtomicUsize::new(0),
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
                Phase::Checking | Phase::Downloading | Phase::Loading | Phase::Cancelling
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
            assert_eq!(backend.probes.load(Ordering::Acquire), 0);
            assert!(!snapshot.probe_passed);
            setup.shutdown()?;
        }
        Ok(())
    }
    #[test]
    fn explicit_setup_probes_even_an_already_cached_model() -> TestResult {
        let (setup, backend, _, _) = controlled(true);
        setup.start(true)?;
        assert_eq!(settled(&setup)?.phase, Phase::Ready);
        assert_eq!(backend.installs.load(Ordering::Acquire), 0);
        assert_eq!(backend.probes.load(Ordering::Acquire), 1);
        assert!(setup.snapshot()?.probe_passed);
        setup.shutdown()?;
        Ok(())
    }

    #[test]
    fn damaged_cache_requires_explicit_repair_before_probe_and_activation() -> TestResult {
        let (setup, backend, entered, release) = controlled(true);
        backend.damaged.store(true, Ordering::Release);
        setup.start(false)?;
        let passive = settled(&setup)?;
        assert_eq!(passive.phase, Phase::Failed);
        assert_eq!(passive.failure, Some(Failure::Hash));
        assert!(!passive.probe_passed);
        assert_eq!(backend.installs.load(Ordering::Acquire), 0);
        assert_eq!(backend.probes.load(Ordering::Acquire), 0);

        start_after_finish(&setup, true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        let repairing = setup.snapshot()?;
        assert_eq!(repairing.phase, Phase::Downloading);
        assert!(!repairing.probe_passed);
        release.send(())?;
        let repaired = settled(&setup)?;
        assert_eq!(repaired.phase, Phase::Ready);
        assert!(repaired.probe_passed);
        assert_eq!(backend.installs.load(Ordering::Acquire), 1);
        assert_eq!(backend.probes.load(Ordering::Acquire), 1);
        setup.shutdown()?;
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
        assert_eq!(backend.probes.load(Ordering::Acquire), 1);
        setup.shutdown()?;
        assert!(matches!(setup.start(true), Err(SetupError::Closed)));
        Ok(())
    }
    #[test]
    fn native_missing_cache_inspection_creates_no_download_or_directory() -> TestResult {
        let root = tempfile::tempdir()?;
        let models = root.path().join("absent");
        let setup = Setup::new(
            models.clone(),
            WorkerSpec {
                executable: root.path().join("unused-worker"),
                model_directory: models.clone(),
                runtime_directory: root.path().join("unused-runtime"),
                identity: refinement_service::ExpectedIdentity {
                    generation: std::num::NonZeroU64::MIN,
                    model_manifest_sha256: RefinementModel::fingerprint(),
                    prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
                    policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
                },
            },
        )?;
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

    struct GatedProbe {
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
        fail: AtomicBool,
    }
    impl Backend for GatedProbe {
        fn inspect(
            &self,
            _: &Path,
            _: &CancellationToken,
        ) -> Result<bool, model::refinement::Error> {
            Ok(true)
        }
        fn install(
            &self,
            _: &Path,
            _: &CancellationToken,
            _: &mut dyn FnMut(u64),
        ) -> Result<(), model::refinement::Error> {
            Err(model::refinement::Error::InvalidCache)
        }
        fn probe(
            &self,
            cancel: &refinement_service::Cancellation,
        ) -> Result<(), refinement_service::Error> {
            self.entered
                .send(())
                .map_err(|_| refinement_service::Failure::Transport)?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if cancel.is_cancelled() {
                    return Err(refinement_service::Failure::Cancelled.into());
                }
                match self.release.recv_timeout(Duration::from_millis(1)) {
                    Ok(()) => break,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout)
                        if Instant::now() < deadline => {}
                    _ => return Err(refinement_service::Failure::StartupTimeout.into()),
                }
            }
            if self.fail.load(Ordering::Acquire) {
                Err(refinement_service::Failure::Exited.into())
            } else {
                Ok(())
            }
        }
    }
    fn gated_probe() -> (
        Setup,
        Arc<GatedProbe>,
        crossbeam_channel::Receiver<()>,
        crossbeam_channel::Sender<()>,
    ) {
        let (entered, seen) = crossbeam_channel::bounded(1);
        let (release, wait) = crossbeam_channel::bounded(1);
        let backend = Arc::new(GatedProbe {
            entered,
            release: wait,
            fail: AtomicBool::new(false),
        });
        (
            Setup::with_backend(PathBuf::from("/unused-test-cache"), 10, backend.clone()),
            backend,
            seen,
            release,
        )
    }
    #[test]
    fn loading_is_busy_until_success_and_a_failed_probe_can_retry() -> TestResult {
        let (setup, backend, entered, release) = gated_probe();
        backend.fail.store(true, Ordering::Release);
        setup.start(true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(setup.snapshot()?.phase, Phase::Loading);
        assert!(setup.snapshot()?.is_busy());
        assert!(matches!(setup.start(true), Err(SetupError::Busy)));
        release.send(())?;
        let failed = settled(&setup)?;
        assert_eq!(failed.phase, Phase::Failed);
        assert_eq!(failed.failure, Some(Failure::Initialization));
        assert!(!failed.probe_passed);
        backend.fail.store(false, Ordering::Release);
        start_after_finish(&setup, true)?;
        entered.recv_timeout(Duration::from_secs(2))?;
        release.send(())?;
        assert_eq!(settled(&setup)?.phase, Phase::Ready);
        setup.shutdown()?;
        Ok(())
    }
    #[test]
    fn cancellation_and_shutdown_revoke_the_model_probe() -> TestResult {
        for shutdown in [false, true] {
            let (setup, _, entered, _release) = gated_probe();
            setup.start(true)?;
            entered.recv_timeout(Duration::from_secs(2))?;
            if shutdown {
                setup.shutdown()?;
            } else {
                setup.cancel()?;
            }
            assert_eq!(settled(&setup)?.phase, Phase::Cancelled);
            setup.shutdown()?;
        }
        Ok(())
    }

    #[test]
    fn shutdown_is_terminal_for_cached_and_explicit_setup() -> TestResult {
        let (setup, backend, _, _) = controlled(true);
        setup.shutdown()?;
        for explicit in [false, true] {
            assert!(matches!(setup.start(explicit), Err(SetupError::Closed)));
            let snapshot = setup.snapshot()?;
            assert_eq!(snapshot.phase, Phase::Missing);
            assert!(!snapshot.probe_passed);
        }
        assert_eq!(backend.installs.load(Ordering::Acquire), 0);
        assert_eq!(backend.probes.load(Ordering::Acquire), 0);
        Ok(())
    }

    struct RejectedProbe {
        reason: refinement_service::Failure,
        cleanup_failed: bool,
    }
    impl Backend for RejectedProbe {
        fn inspect(
            &self,
            _: &Path,
            _: &CancellationToken,
        ) -> Result<bool, model::refinement::Error> {
            Ok(true)
        }
        fn install(
            &self,
            _: &Path,
            _: &CancellationToken,
            _: &mut dyn FnMut(u64),
        ) -> Result<(), model::refinement::Error> {
            Err(model::refinement::Error::IncompleteModel)
        }
        fn probe(
            &self,
            _: &refinement_service::Cancellation,
        ) -> Result<(), refinement_service::Error> {
            Err(refinement_service::Error {
                reason: self.reason,
                cleanup_failed: self.cleanup_failed,
            })
        }
    }

    #[test]
    fn probe_errors_never_grant_activation_and_cleanup_failure_is_not_cancellation() -> TestResult {
        use refinement_service::Failure as WorkerFailure;
        for (reason, cleanup_failed, phase, failure) in [
            (
                WorkerFailure::StartupTimeout,
                false,
                Phase::Failed,
                Some(Failure::Timeout),
            ),
            (
                WorkerFailure::InvalidCandidate,
                false,
                Phase::Failed,
                Some(Failure::Initialization),
            ),
            (WorkerFailure::Cancelled, false, Phase::Cancelled, None),
            (
                WorkerFailure::Cancelled,
                true,
                Phase::Failed,
                Some(Failure::Worker),
            ),
        ] {
            let setup = Setup::with_backend(
                PathBuf::from("/unused-test-cache"),
                10,
                Arc::new(RejectedProbe {
                    reason,
                    cleanup_failed,
                }),
            );
            setup.start(true)?;
            let snapshot = settled(&setup)?;
            assert_eq!(snapshot.phase, phase);
            assert_eq!(snapshot.failure, failure);
            assert!(!snapshot.probe_passed);
            setup.shutdown()?;
        }
        Ok(())
    }

    #[test]
    fn native_probe_rejects_invalid_runtime_and_missing_executable() -> TestResult {
        let root = tempfile::tempdir()?;
        let runtime = root.path().join("runtime");
        let backend = NativeBackend {
            model: RefinementModel::bundled()?,
            worker: WorkerSpec {
                executable: root.path().join("missing-worker"),
                model_directory: root.path().join("models"),
                runtime_directory: runtime.clone(),
                identity: refinement_service::ExpectedIdentity {
                    generation: std::num::NonZeroU64::MIN,
                    model_manifest_sha256: "0".repeat(64),
                    prompt_version: 1,
                    policy_version: 1,
                },
            },
        };
        std::fs::write(&runtime, b"not a directory")?;
        let error = backend
            .probe(&refinement_service::Cancellation::new())
            .err()
            .ok_or("probe accepted a runtime file")?;
        assert_eq!(error.reason, refinement_service::Failure::Spawn);
        std::fs::remove_file(&runtime)?;
        std::fs::create_dir(&backend.worker.model_directory)?;
        let error = backend
            .probe(&refinement_service::Cancellation::new())
            .err()
            .ok_or("probe accepted a missing executable")?;
        assert_eq!(error.reason, refinement_service::Failure::InvalidPath);
        assert!(!error.cleanup_failed);
        assert_eq!(
            std::fs::metadata(&runtime)?.permissions().mode() & 0o777,
            0o700
        );
        assert!(
            std::fs::read_dir(&runtime)?.all(|entry| entry.is_ok_and(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with("text-worker-v1-")))
        );
        Ok(())
    }
}
