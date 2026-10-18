//! Serial inference with one pending audio journal and immediate discard.

use crate::{
    model::ModelDefinition,
    transcription::{Engine, InferenceError, Language},
};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::{
    ops::Range,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

#[cfg(feature = "measurement")]
pub(crate) mod observation;

pub trait Decoder: Send + 'static {
    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError>;
}

impl Decoder for Engine {
    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        self.transcribe(audio, language)
    }
}

#[derive(Clone)]
pub struct WorkToken(Arc<AtomicBool>);
impl Default for WorkToken {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}
impl WorkToken {
    pub fn cancel(&self) {
        self.0.store(false, Ordering::Release);
    }
    pub fn is_valid(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub struct Job {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    /// Local slice in immutable storage (which may physically retain a suffix).
    pub sample_range: Range<usize>,
    /// Global, disjoint recording frames; equal length, independent origin.
    pub timeline_range: Range<usize>,
    // Arc<Vec> moves the existing journal buffer without copying its samples.
    pub audio: Arc<Vec<f32>>,
    pub language: Language,
    pub token: WorkToken,
}

/// Entire actual Decoder::decode call, including Engine preprocessing/VAD/ASR.
/// Exists only for developer measurements; never queue or stop-to-text time.
#[cfg(feature = "measurement")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeTiming {
    pub elapsed: std::time::Duration,
    pub input_frames: usize,
}
#[cfg(feature = "measurement")]
impl DecodeTiming {
    pub fn rtf(self) -> Option<f64> {
        (self.input_frames != 0)
            .then(|| self.elapsed.as_secs_f64() * 16000.0 / self.input_frames as f64)
    }
}

pub struct Completed {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    /// Local slice in immutable storage (which may physically retain a suffix).
    pub sample_range: Range<usize>,
    /// Global, disjoint recording frames; equal length, independent origin.
    pub timeline_range: Range<usize>,
    pub result: Result<String, InferenceError>,
    #[cfg(feature = "measurement")]
    pub decode_timing: Option<DecodeTiming>,
    token: WorkToken,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Inference worker is not ready")]
    NotReady,
    #[error("Inference pending slot is occupied")]
    Busy,
    #[error("Invalid inference job")]
    InvalidJob,
    #[error("Inference synchronization failed")]
    Poisoned,
    #[error("Inference worker is stopped")]
    Stopped,
    #[error("Inference worker could not be started or joined")]
    Worker,
    #[error("Unconsumed valid inference result would be overwritten")]
    ResultBackpressure,
    #[error(transparent)]
    Backend(#[from] InferenceError),
}

#[derive(Default)]
struct Slots {
    pending: Option<Job>,
    active: Option<(u64, WorkToken)>,
    completed: Option<Completed>,
}

#[derive(Default)]
struct Shared {
    ready: AtomicBool,
    shutdown: AtomicBool,
    slots: Mutex<Slots>,
    #[cfg(feature = "measurement")]
    observer: Mutex<Option<Arc<dyn observation::Observer>>>,
    #[cfg(test)]
    completion_barrier: Mutex<Option<CompletionBarrier>>,
    #[cfg(test)]
    before_decode_barrier: Mutex<Option<CompletionBarrier>>,
}
#[cfg(test)]
struct CompletionBarrier {
    reached: Sender<()>,
    release: Receiver<()>,
}

struct ReadyGuard(Arc<Shared>);
impl Drop for ReadyGuard {
    fn drop(&mut self) {
        self.0.ready.store(false, Ordering::Release);
    }
}

pub struct InferenceService {
    shared: Arc<Shared>,
    wake: Sender<()>,
    worker: Option<JoinHandle<Result<(), ServiceError>>>,
}

impl InferenceService {
    pub fn load(path: PathBuf, definition: ModelDefinition) -> Result<Self, ServiceError> {
        Self::spawn(move || Engine::load(&path, definition))
    }

    /// Controlled decoder seam for deterministic worker/lifecycle verification.
    pub fn with_decoder(decoder: impl Decoder) -> Result<Self, ServiceError> {
        Self::spawn(move || Ok(decoder))
    }

    fn spawn<D: Decoder>(
        initialize: impl FnOnce() -> Result<D, InferenceError> + Send + 'static,
    ) -> Result<Self, ServiceError> {
        let shared = Arc::new(Shared::default());
        let owned = Arc::clone(&shared);
        let (wake, awakened) = crossbeam_channel::bounded(1);
        let worker = thread::Builder::new()
            .name("local-inference".into())
            .spawn(move || {
                let _ready = ReadyGuard(Arc::clone(&owned));
                let mut decoder = initialize()?;
                if owned.shutdown.load(Ordering::Acquire) {
                    return Ok(());
                }
                owned.ready.store(true, Ordering::Release);
                inference_loop(&owned, &awakened, &mut decoder)
            })
            .map_err(|_| ServiceError::Worker)?;
        Ok(Self {
            shared,
            wake,
            worker: Some(worker),
        })
    }

    #[cfg(feature = "measurement")]
    pub(crate) fn observe_jobs(
        &self,
        observer: Arc<dyn observation::Observer>,
    ) -> Result<(), ServiceError> {
        let slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        if slots.pending.is_some() || slots.active.is_some() || slots.completed.is_some() {
            return Err(ServiceError::Busy);
        }
        *self
            .shared
            .observer
            .lock()
            .map_err(|_| ServiceError::Poisoned)? = Some(observer);
        Ok(())
    }
    #[cfg(all(test, feature = "measurement"))]
    pub(crate) fn pause_before_decode(
        &self,
        reached: Sender<()>,
        release: Receiver<()>,
    ) -> Result<(), ServiceError> {
        *self
            .shared
            .before_decode_barrier
            .lock()
            .map_err(|_| ServiceError::Poisoned)? = Some(CompletionBarrier { reached, release });
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.shared.ready.load(Ordering::Acquire) && !self.shared.shutdown.load(Ordering::Acquire)
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn busy(&self) -> Result<bool, ServiceError> {
        let slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        Ok(slots.pending.is_some() || slots.active.is_some())
    }

    pub fn submit(&self, job: Job) -> Result<(), ServiceError> {
        if !self.ready() {
            return Err(ServiceError::NotReady);
        }
        if job.session == 0
            || job.generation == 0
            || job.job_id == 0
            || job.sample_range.start > job.sample_range.end
            || job.sample_range.end > job.audio.len()
            || job.timeline_range.start > job.timeline_range.end
            || job.timeline_range.end > dictation_core::audio::MAX_SAMPLES
            || job.timeline_range.len() != job.sample_range.len()
            || !job.token.is_valid()
            || job.audio.len() > dictation_core::audio::MAX_SAMPLES
            || job.audio.iter().any(|sample| !sample.is_finite())
        {
            return Err(ServiceError::InvalidJob);
        }
        let mut slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        if slots
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token.is_valid())
        {
            return Err(ServiceError::Busy);
        }
        #[cfg(feature = "measurement")]
        self.shared.observe(&job, observation::Edge::Queued);
        slots.pending = Some(job);
        drop(slots);
        self.notify()
    }

    /// Call before allowing a new recording: releases pending audio now while
    /// one already-running backend call may finish on the same serial worker.
    pub fn cancel(&self, session: u64) -> Result<(), ServiceError> {
        let mut slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        if slots
            .pending
            .as_ref()
            .is_some_and(|job| job.session == session)
            && let Some(job) = slots.pending.take()
        {
            job.token.cancel();
            #[cfg(feature = "measurement")]
            self.shared
                .observe(&job, observation::Edge::CancelledPending);
        }
        if let Some((active, token)) = &slots.active
            && *active == session
        {
            token.cancel();
        }
        if slots
            .completed
            .as_ref()
            .is_some_and(|result| result.session == session)
            && let Some(result) = slots.completed.take()
        {
            result.token.cancel();
        }
        Ok(())
    }

    pub fn take_result(&self) -> Result<Option<Completed>, ServiceError> {
        let mut slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        Ok(slots
            .completed
            .take()
            .filter(|result| result.token.is_valid()))
    }

    fn notify(&self) -> Result<(), ServiceError> {
        match self.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(()),
            Err(TrySendError::Disconnected(())) => Err(ServiceError::Stopped),
        }
    }

    fn request_shutdown(&self) -> Result<(), ServiceError> {
        self.shared.shutdown.store(true, Ordering::Release);
        let mut slots = self
            .shared
            .slots
            .lock()
            .map_err(|_| ServiceError::Poisoned)?;
        if let Some(job) = slots.pending.take() {
            job.token.cancel();
            #[cfg(feature = "measurement")]
            self.shared
                .observe(&job, observation::Edge::CancelledPending);
        }
        if let Some((_, token)) = &slots.active {
            token.cancel();
        }
        slots.completed = None;
        drop(slots);
        self.notify()
    }

    /// Explicitly join outside the GUI. No unverified hard backend interrupt.
    pub fn shutdown(mut self) -> Result<(), ServiceError> {
        let requested = self.request_shutdown();
        let joined = self
            .worker
            .take()
            .ok_or(ServiceError::Worker)?
            .join()
            .map_err(|_| ServiceError::Worker)?;
        // Preserve the worker's actual failure even after its wake receiver
        // has closed; a disconnected notification is only secondary evidence.
        joined?;
        match requested {
            Ok(()) | Err(ServiceError::Stopped) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for InferenceService {
    fn drop(&mut self) {
        // Drop only requests release; GUI teardown never waits on inference.
        match self.request_shutdown() {
            Ok(()) | Err(ServiceError::Stopped) => {}
            Err(_) => eprintln!("inference-shutdown-failed"),
        }
    }
}

fn inference_loop(
    shared: &Shared,
    wake: &Receiver<()>,
    decoder: &mut impl Decoder,
) -> Result<(), ServiceError> {
    loop {
        if shared.shutdown.load(Ordering::Acquire) {
            return Ok(());
        }
        let job = {
            let mut slots = shared.slots.lock().map_err(|_| ServiceError::Poisoned)?;
            let job = slots.pending.take();
            if let Some(job) = &job {
                slots.active = Some((job.session, job.token.clone()));
            }
            job
        };
        let Some(job) = job else {
            wake.recv().map_err(|_| ServiceError::Stopped)?;
            continue;
        };
        #[cfg(test)]
        if let Some(barrier) = shared
            .before_decode_barrier
            .lock()
            .map_err(|_| ServiceError::Poisoned)?
            .take()
        {
            barrier
                .reached
                .send(())
                .map_err(|_| ServiceError::Stopped)?;
            barrier
                .release
                .recv_timeout(std::time::Duration::from_secs(3))
                .map_err(|_| ServiceError::Stopped)?;
        }
        #[cfg(feature = "measurement")]
        let mut decode_timing = None;
        let result = if job.token.is_valid() {
            let input = &job.audio[job.sample_range.clone()];
            #[cfg(feature = "measurement")]
            {
                shared.observe(&job, observation::Edge::BeforeDecode);
                let started = std::time::Instant::now();
                let result = decoder.decode(input, job.language);
                decode_timing = Some(DecodeTiming {
                    elapsed: started.elapsed(),
                    input_frames: input.len(),
                });
                result
            }
            #[cfg(not(feature = "measurement"))]
            decoder.decode(input, job.language)
        } else {
            Ok(String::new())
        };
        #[cfg(feature = "measurement")]
        shared.observe(
            &job,
            observation::Edge::AfterDecode(if !job.token.is_valid() {
                if decode_timing.is_some() {
                    "cancelled-after-decode"
                } else {
                    "cancelled-before-decode"
                }
            } else if shared.shutdown.load(Ordering::Acquire) {
                "discarded-closing"
            } else if result.is_ok() {
                "decoded-ok"
            } else {
                "decode-failed"
            }),
        );
        // Release the actual PCM before advertising completion or idle.
        drop(job.audio);
        let mut slots = shared.slots.lock().map_err(|_| ServiceError::Poisoned)?;
        slots.active = None;
        if job.token.is_valid() && !shared.shutdown.load(Ordering::Acquire) {
            if slots
                .completed
                .as_ref()
                .is_some_and(|completed| completed.token.is_valid())
            {
                return Err(ServiceError::ResultBackpressure);
            }
            slots.completed = Some(Completed {
                session: job.session,
                generation: job.generation,
                job_id: job.job_id,
                sample_range: job.sample_range,
                timeline_range: job.timeline_range,
                result,
                #[cfg(feature = "measurement")]
                decode_timing,
                token: job.token,
            });
        }
        drop(slots);
        #[cfg(test)]
        if let Some(barrier) = shared
            .completion_barrier
            .lock()
            .map_err(|_| ServiceError::Poisoned)?
            .take()
        {
            barrier
                .reached
                .send(())
                .map_err(|_| ServiceError::Stopped)?;
            barrier
                .release
                .recv_timeout(std::time::Duration::from_secs(2))
                .map_err(|_| ServiceError::Stopped)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn completed_publication_and_idle_never_retain_the_job_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Immediate;
        impl Decoder for Immediate {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok("done".into())
            }
        }
        let service = InferenceService::with_decoder(Immediate)?;
        wait_ready(&service)?;
        let (reached, published) = crossbeam_channel::bounded(1);
        let (release, released) = crossbeam_channel::bounded(1);
        *service
            .shared
            .completion_barrier
            .lock()
            .map_err(|_| "barrier poisoned")? = Some(CompletionBarrier {
            reached,
            release: released,
        });
        let work = job(1, 1);
        let storage = Arc::downgrade(&work.audio);
        service.submit(work)?;
        published.recv_timeout(Duration::from_secs(2))?;
        let idle = !service.busy()?;
        let completed = service.take_result()?.is_some();
        let reclaimed = storage.upgrade().is_none();
        release.send(())?;
        service.shutdown()?;
        assert!(
            idle && completed,
            "actual worker must have published completion while held"
        );
        assert!(
            reclaimed,
            "completion/idle must imply the worker released its PCM Arc"
        );
        Ok(())
    }

    #[test]
    fn initialization_failure_remains_observable_after_worker_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let service = InferenceService::load(
            directory.path().join("missing.bin"),
            crate::model::REFERENCE,
        )?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !service.is_finished() {
            if Instant::now() >= deadline {
                return Err("failed initialization did not terminate".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!service.ready());
        assert!(matches!(
            service.shutdown(),
            Err(ServiceError::Backend(InferenceError::Model(_)))
        ));
        Ok(())
    }

    fn job(session: u64, generation: u64) -> Job {
        Job {
            session,
            generation,
            job_id: session,
            sample_range: 0..320,
            timeline_range: 0..320,
            audio: Arc::new(vec![0.1; 320]),
            language: Language::German,
            token: WorkToken::default(),
        }
    }

    fn wait_idle(service: &InferenceService) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while service.busy()? {
            if Instant::now() >= deadline {
                return Err("worker did not become idle".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn selected_sample_range_and_job_identity_survive_the_worker_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        struct RangeDecoder;
        impl Decoder for RangeDecoder {
            fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
                if audio != [0.2, 0.3] {
                    return Err(InferenceError::InvalidAudio);
                }
                Ok("selected samples".into())
            }
        }
        let service = InferenceService::with_decoder(RangeDecoder)?;
        wait_ready(&service)?;
        service.submit(Job {
            audio: Arc::new(vec![0.1, 0.2, 0.3, 0.4]),
            job_id: 42,
            sample_range: 1..3,
            timeline_range: 100..102,
            ..job(7, 9)
        })?;
        wait_idle(&service)?;
        let completed = service.take_result()?.ok_or("missing range result")?;
        assert_eq!(
            (completed.session, completed.generation, completed.job_id),
            (7, 9, 42)
        );
        assert_eq!(completed.sample_range, 1..3);
        assert_eq!(completed.timeline_range, 100..102);
        #[cfg(feature = "measurement")]
        assert_eq!(
            completed
                .decode_timing
                .ok_or("actual decode timing missing")?
                .input_frames,
            2
        );
        assert_eq!(completed.result?, "selected samples");
        assert!(matches!(
            service.submit(Job {
                sample_range: 0..321,
                timeline_range: 0..321,
                ..job(8, 9)
            }),
            Err(ServiceError::InvalidJob)
        ));
        assert!(matches!(
            service.submit(Job {
                job_id: 0,
                ..job(8, 9)
            }),
            Err(ServiceError::InvalidJob)
        ));
        service.shutdown()?;
        Ok(())
    }

    #[test]
    fn one_running_and_one_pending_job_preserve_tags_and_results_in_serial_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = crossbeam_channel::bounded(2);
        let (release, finish) = crossbeam_channel::bounded(2);
        let service = InferenceService::with_decoder(ControlledDecoder {
            entered,
            release: finish,
        })?;
        wait_ready(&service)?;
        service.submit(job(1, 7))?;
        started.recv_timeout(Duration::from_secs(1))?;
        service.submit(job(2, 8))?;
        assert!(matches!(service.submit(job(3, 8)), Err(ServiceError::Busy)));
        release.send(())?;
        started.recv_timeout(Duration::from_secs(1))?;
        let first = service.take_result()?.ok_or("missing first result")?;
        assert_eq!((first.session, first.generation), (1, 7));
        assert_eq!((first.job_id, first.sample_range), (1, 0..320));
        assert_eq!(first.result?, "fixture result");
        release.send(())?;
        wait_idle(&service)?;
        let second = service.take_result()?.ok_or("missing second result")?;
        assert_eq!((second.session, second.generation), (2, 8));
        assert_eq!((second.job_id, second.sample_range), (2, 0..320));
        assert_eq!(second.result?, "fixture result");
        service.shutdown()?;
        Ok(())
    }

    #[test]
    fn cancellation_discards_a_published_result_without_touching_another_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = crossbeam_channel::bounded(2);
        let (release, finish) = crossbeam_channel::bounded(2);
        let service = InferenceService::with_decoder(ControlledDecoder {
            entered,
            release: finish,
        })?;
        wait_ready(&service)?;
        service.submit(job(10, 4))?;
        started.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        wait_idle(&service)?;
        service.cancel(9)?;
        let result = service
            .take_result()?
            .ok_or("unrelated cancellation lost result")?;
        assert_eq!(result.session, 10);
        service.submit(job(11, 4))?;
        started.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        wait_idle(&service)?;
        service.cancel(11)?;
        assert!(service.take_result()?.is_none());
        service.shutdown()?;
        Ok(())
    }

    #[test]
    fn unconsumed_result_is_preserved_and_backpressure_is_reported()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = crossbeam_channel::bounded(2);
        let (release, finish) = crossbeam_channel::bounded(2);
        let service = InferenceService::with_decoder(ControlledDecoder {
            entered,
            release: finish,
        })?;
        wait_ready(&service)?;
        service.submit(job(1, 1))?;
        started.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        wait_idle(&service)?;
        service.submit(job(2, 1))?;
        started.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !service.is_finished() {
            if Instant::now() >= deadline {
                return Err("backpressure did not terminate the worker".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!service.ready());
        assert_eq!(
            service
                .take_result()?
                .ok_or("original result lost")?
                .session,
            1
        );
        assert!(matches!(
            service.shutdown(),
            Err(ServiceError::ResultBackpressure)
        ));
        Ok(())
    }

    #[test]
    fn drop_clears_pending_audio_without_waiting_for_the_active_decoder()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = crossbeam_channel::bounded(2);
        let (release, finish) = crossbeam_channel::bounded(2);
        let service = InferenceService::with_decoder(ControlledDecoder {
            entered,
            release: finish,
        })?;
        wait_ready(&service)?;
        service.submit(job(1, 1))?;
        started.recv_timeout(Duration::from_secs(1))?;
        let queued = Arc::new(vec![0.1; 320]);
        let weak = Arc::downgrade(&queued);
        service.submit(Job {
            audio: queued,
            ..job(2, 1)
        })?;
        let started = Instant::now();
        drop(service);
        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(weak.upgrade().is_none());
        release.send(())?;
        Ok(())
    }

    struct ControlledDecoder {
        entered: crossbeam_channel::Sender<()>,
        release: crossbeam_channel::Receiver<()>,
    }
    impl Decoder for ControlledDecoder {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            self.entered.send(()).map_err(|_| InferenceError::Decode)?;
            self.release.recv().map_err(|_| InferenceError::Decode)?;
            Ok("fixture result".into())
        }
    }

    fn wait_ready(service: &InferenceService) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !service.ready() {
            if Instant::now() >= deadline {
                return Err("worker not ready".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn cancelling_queued_work_frees_its_audio_while_one_old_job_is_still_running()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = crossbeam_channel::bounded(2);
        let (release, finish) = crossbeam_channel::bounded(2);
        let service = InferenceService::with_decoder(ControlledDecoder {
            entered,
            release: finish,
        })?;
        wait_ready(&service)?;
        service.submit(Job {
            session: 1,
            generation: 1,
            job_id: 1,
            sample_range: 0..320,
            timeline_range: 0..320,
            audio: Arc::new(vec![0.1; 320]),
            language: Language::German,
            token: WorkToken::default(),
        })?;
        started.recv_timeout(Duration::from_secs(1))?;
        let queued = Arc::new(vec![0.2; 320]);
        let weak = Arc::downgrade(&queued);
        service.submit(Job {
            session: 2,
            generation: 1,
            job_id: 2,
            sample_range: 0..320,
            timeline_range: 0..320,
            audio: queued,
            language: Language::German,
            token: WorkToken::default(),
        })?;
        assert!(weak.upgrade().is_some());
        service.cancel(2)?;
        assert!(
            weak.upgrade().is_none(),
            "cancelled pending audio must be freed synchronously"
        );
        service.cancel(1)?;
        release.send(())?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while service.busy()? {
            if Instant::now() >= deadline {
                return Err("cancelled job did not finish".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(service.take_result()?.is_none());
        service.shutdown()?;
        Ok(())
    }
    #[cfg(feature = "measurement")]
    #[test]
    fn actual_decode_timing_spans_the_controlled_call_and_selected_slice()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Held {
            entered: crossbeam_channel::Sender<Instant>,
            release: crossbeam_channel::Receiver<()>,
        }
        impl Decoder for Held {
            fn decode(&mut self, samples: &[f32], _: Language) -> Result<String, InferenceError> {
                if samples != [0.2, 0.3] {
                    return Err(InferenceError::InvalidAudio);
                }
                self.entered
                    .send(Instant::now())
                    .map_err(|_| InferenceError::Decode)?;
                self.release
                    .recv_timeout(Duration::from_secs(2))
                    .map_err(|_| InferenceError::Decode)?;
                Ok("measured".into())
            }
        }
        let (entered, entry) = crossbeam_channel::bounded(1);
        let (release, resume) = crossbeam_channel::bounded(1);
        let service = InferenceService::with_decoder(Held {
            entered,
            release: resume,
        })?;
        wait_ready(&service)?;
        service.submit(Job {
            audio: Arc::new(vec![0.1, 0.2, 0.3, 0.4]),
            sample_range: 1..3,
            timeline_range: 1..3,
            ..job(7, 9)
        })?;
        let entered_at = entry.recv_timeout(Duration::from_secs(2))?;
        let released_at = Instant::now();
        release.send(())?;
        wait_idle(&service)?;
        let result = service.take_result()?.ok_or("missing completed call")?;
        let timing = result.decode_timing.ok_or("missing actual timing")?;
        assert_eq!(timing.input_frames, 2);
        assert!(timing.elapsed >= released_at.duration_since(entered_at));
        assert!(!timing.elapsed.is_zero());
        service.shutdown()?;
        assert_eq!(
            DecodeTiming {
                elapsed: Duration::from_millis(250),
                input_frames: 16000
            }
            .rtf(),
            Some(0.25)
        );
        assert_eq!(
            DecodeTiming {
                elapsed: Duration::from_secs(1),
                input_frames: 0
            }
            .rtf(),
            None
        );
        Ok(())
    }
}
