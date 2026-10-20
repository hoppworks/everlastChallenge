//! Scenario runner against the public service; deterministic tests use Decoder.
use crossbeam_channel::{Receiver, Sender, TrySendError};
use everlast_dictation::{
    inference_service::{Decoder, InferenceService, Job, WorkToken},
    transcription::{InferenceError, Language},
};
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

type Error = Box<dyn std::error::Error>;
const GENERATION: u64 = 71;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    reason: String,
    cleanup_error: Option<String>,
}
impl Failure {
    pub fn before_worker(reason: &str) -> Self {
        Self {
            reason: reason.into(),
            cleanup_error: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pending_audio_freed_before_release: bool,
    active_audio_held_before_release: bool,
    third_submit_micros: Option<u128>,
    wrapper_entries: usize,
    inner_decoder_calls: usize,
    completed_session: Option<u64>,
    completed_generation: Option<u64>,
    completed_job_id: Option<u64>,
    result_exactly_once: Option<bool>,
    baseline_text_equal: Option<bool>,
    cleanup_micros: u128,
    shutdown_observed_blocked_millis: Option<u64>,
}

struct Wrapped<D> {
    inner: D,
    entered: Sender<usize>,
    release: Receiver<()>,
    entries: Arc<AtomicUsize>,
    inner_calls: Arc<AtomicUsize>,
    budget: Duration,
}
impl<D: Decoder> Decoder for Wrapped<D> {
    fn decode(&mut self, samples: &[f32], language: Language) -> Result<String, InferenceError> {
        let ordinal = self.entries.fetch_add(1, Ordering::SeqCst) + 1;
        self.entered
            .try_send(ordinal)
            .map_err(|_| InferenceError::Decode)?;
        if ordinal == 1 {
            self.release
                .recv_timeout(self.budget)
                .map_err(|_| InferenceError::Decode)?;
        }
        self.inner_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.decode(samples, language)
    }
}

struct Release(Sender<()>);
impl Release {
    fn now(&self) {
        match self.0.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        }
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.now();
    }
}

fn job(session: u64, audio: Arc<Vec<f32>>) -> Job {
    Job {
        session,
        generation: GENERATION,
        job_id: session * 101,
        sample_range: 0..audio.len(),
        timeline_range: 0..audio.len(),
        audio,
        language: Language::German,
        token: WorkToken::default(),
    }
}
fn check(condition: bool, reason: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(reason.into())
    }
}
fn wait_until(
    budget: Duration,
    mut condition: impl FnMut() -> Result<bool, Error>,
) -> Result<(), Error> {
    let deadline = Instant::now() + budget;
    while !condition()? {
        if Instant::now() >= deadline {
            return Err("bounded lifecycle observation timed out".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
fn ready(service: &InferenceService, budget: Duration) -> Result<(), Error> {
    wait_until(budget, || {
        check(!service.is_finished(), "worker exited before readiness")?;
        Ok(service.ready())
    })
}

struct Cleanup {
    thread: thread::JoinHandle<()>,
    received: Receiver<Result<(), everlast_dictation::inference_service::ServiceError>>,
    started: Instant,
    observed: Option<Result<(), everlast_dictation::inference_service::ServiceError>>,
}
impl Cleanup {
    fn start(service: InferenceService) -> Result<Self, String> {
        let started = Instant::now();
        let (sent, received) = crossbeam_channel::bounded(1);
        let thread = thread::Builder::new()
            .name("probe-inference-join".into())
            .spawn(move || {
                let result = service.shutdown();
                match sent.send(result) {
                    Ok(()) | Err(_) => {}
                }
            })
            .map_err(|e| format!("cleanup observer spawn failed; worker join unverified: {e}"))?;
        Ok(Self {
            thread,
            received,
            started,
            observed: None,
        })
    }
    fn expect_incomplete(&mut self) -> Result<(), Error> {
        match self.received.recv_timeout(Duration::from_millis(100)) {
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => Ok(()),
            Ok(result) => {
                self.observed = Some(result);
                Err("shutdown completed before the held decoder was released".into())
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err("shutdown observer disconnected before decoder release".into())
            }
        }
    }
    fn finish(self, budget: Duration) -> Result<u128, String> {
        let observed = self
            .observed
            .map_or_else(|| self.received.recv_timeout(budget), Ok)
            .map_err(|e| {
                format!("cleanup observation failed; backend may still run, join unverified: {e}")
            })?;
        wait_until(budget, || Ok(self.thread.is_finished())).map_err(|e| e.to_string())?;
        self.thread
            .join()
            .map_err(|_| "cleanup observer panicked".to_string())?;
        observed.map_err(|e| format!("service shutdown failed: {e}"))?;
        Ok(self.started.elapsed().as_micros())
    }
}
fn shutdown_bounded(service: InferenceService, budget: Duration) -> Result<u128, String> {
    Cleanup::start(service)?.finish(budget)
}

fn conclude(
    result: Result<Evidence, Error>,
    cleanup: Result<u128, String>,
) -> Result<Evidence, Failure> {
    match (result, cleanup) {
        (Ok(mut evidence), Ok(micros)) => {
            evidence.cleanup_micros = micros;
            Ok(evidence)
        }
        (Err(reason), cleanup) => Err(Failure {
            reason: reason.to_string(),
            cleanup_error: cleanup.err(),
        }),
        (Ok(_), Err(reason)) => Err(Failure {
            reason: "scenario passed but cleanup failed".into(),
            cleanup_error: Some(reason),
        }),
    }
}

pub fn restart<D: Decoder + Send>(
    decoder: D,
    samples: Vec<f32>,
    budget: Duration,
) -> Result<Evidence, Failure> {
    let baseline_thread = thread::Builder::new()
        .name("probe-uncancelled-baseline".into())
        .spawn(move || {
            let mut decoder = decoder;
            let result = decoder.decode(&samples, Language::German);
            (decoder, samples, result)
        })
        .map_err(|e| Failure::before_worker(&format!("baseline worker spawn failed: {e}")))?;
    if let Err(reason) = wait_until(budget, || Ok(baseline_thread.is_finished())) {
        return Err(Failure {
            reason: reason.to_string(),
            cleanup_error: Some("baseline decoder may still run; join unverified".into()),
        });
    }
    let (decoder, samples, baseline) = baseline_thread
        .join()
        .map_err(|_| Failure::before_worker("baseline worker panicked"))?;
    let baseline = baseline
        .map_err(|e| Failure::before_worker(&format!("uncancelled baseline failed: {e}")))?;
    let (entered, observed) = crossbeam_channel::bounded(4);
    let (release, released) = crossbeam_channel::bounded(1);
    let release = Release(release);
    let entries = Arc::new(AtomicUsize::new(0));
    let inner_calls = Arc::new(AtomicUsize::new(0));
    let service = InferenceService::with_decoder(Wrapped {
        inner: decoder,
        entered,
        release: released,
        entries: entries.clone(),
        inner_calls: inner_calls.clone(),
        budget,
    })
    .map_err(|e| Failure::before_worker(&e.to_string()))?;
    let first = Arc::new(samples);
    let second = Arc::new(first.as_ref().clone());
    let third = Arc::new(first.as_ref().clone());
    let first_weak = Arc::downgrade(&first);
    let second_weak = Arc::downgrade(&second);
    let third_weak = Arc::downgrade(&third);
    let frames = first.len();
    let outcome = (|| -> Result<Evidence, Error> {
        ready(&service, budget)?;
        service.submit(job(1, first))?;
        check(
            observed.recv_timeout(budget)? == 1,
            "unexpected first decoder entry",
        )?;
        service.cancel(1)?;
        service.submit(job(2, second))?;
        check(
            second_weak.upgrade().is_some(),
            "pending second audio was never held",
        )?;
        service.cancel(2)?;
        check(
            second_weak.upgrade().is_none(),
            "cancel did not synchronously release pending audio",
        )?;
        check(
            first_weak.upgrade().is_some(),
            "old active audio was released too early",
        )?;
        let tick = Instant::now();
        service.submit(job(3, third))?;
        let submit_micros = tick.elapsed().as_micros();
        check(
            inner_calls.load(Ordering::SeqCst) == 0,
            "old native-call barrier expired before third submission",
        )?;
        check(
            entries.load(Ordering::SeqCst) == 1,
            "pending work entered decoder before release",
        )?;
        release.now();
        check(
            observed.recv_timeout(budget)? == 2,
            "cancelled pending job entered decoder",
        )?;
        wait_until(budget, || Ok(!service.busy()?))?;
        let completed = service.take_result()?.ok_or("missing restart result")?;
        check(
            (completed.session, completed.generation, completed.job_id) == (3, GENERATION, 303),
            "stale or incorrect result identity",
        )?;
        check(
            completed.sample_range == (0..frames),
            "result range changed",
        )?;
        check(
            completed.result? == baseline,
            "restart differs from uncancelled same-input baseline",
        )?;
        check(service.take_result()?.is_none(), "duplicate result")?;
        wait_until(budget, || {
            Ok(first_weak.upgrade().is_none() && third_weak.upgrade().is_none())
        })?;
        check(
            entries.load(Ordering::SeqCst) == 2 && inner_calls.load(Ordering::SeqCst) == 2,
            "decoder call count proves unexpected work",
        )?;
        check(
            observed.try_recv().is_err(),
            "unexpected extra decoder entry",
        )?;
        Ok(Evidence {
            pending_audio_freed_before_release: true,
            active_audio_held_before_release: true,
            third_submit_micros: Some(submit_micros),
            wrapper_entries: 2,
            inner_decoder_calls: 2,
            completed_session: Some(3),
            completed_generation: Some(GENERATION),
            completed_job_id: Some(303),
            result_exactly_once: Some(true),
            baseline_text_equal: Some(true),
            cleanup_micros: 0,
            shutdown_observed_blocked_millis: None,
        })
    })();
    release.now();
    conclude(outcome, shutdown_bounded(service, budget))
}

pub fn shutdown_held<D: Decoder + Send>(
    decoder: D,
    samples: Vec<f32>,
    budget: Duration,
) -> Result<Evidence, Failure> {
    let (entered, observed) = crossbeam_channel::bounded(4);
    let (release, released) = crossbeam_channel::bounded(1);
    let release = Release(release);
    let entries = Arc::new(AtomicUsize::new(0));
    let inner_calls = Arc::new(AtomicUsize::new(0));
    let service = InferenceService::with_decoder(Wrapped {
        inner: decoder,
        entered,
        release: released,
        entries: entries.clone(),
        inner_calls: inner_calls.clone(),
        budget,
    })
    .map_err(|e| Failure::before_worker(&e.to_string()))?;
    let first = Arc::new(samples);
    let second = Arc::new(first.as_ref().clone());
    let first_weak = Arc::downgrade(&first);
    let second_weak = Arc::downgrade(&second);
    let mut service = Some(service);
    let mut cleanup = None;
    let outcome = (|| -> Result<Evidence, Error> {
        let worker = service.as_ref().ok_or("missing service")?;
        ready(worker, budget)?;
        worker.submit(job(4, first))?;
        check(
            observed.recv_timeout(budget)? == 1,
            "unexpected shutdown decoder entry",
        )?;
        worker.submit(job(5, second))?;
        let worker = service.take().ok_or("service already moved")?;
        cleanup = Some(Cleanup::start(worker));
        check(
            cleanup.as_ref().is_some_and(Result::is_ok),
            "cleanup observer could not start",
        )?;
        wait_until(budget, || Ok(second_weak.upgrade().is_none()))?;
        check(
            first_weak.upgrade().is_some(),
            "shutdown prematurely released active audio",
        )?;
        cleanup
            .as_mut()
            .and_then(|result| result.as_mut().ok())
            .ok_or("cleanup observer unavailable")?
            .expect_incomplete()?;
        check(
            inner_calls.load(Ordering::SeqCst) == 0,
            "native barrier expired before shutdown observation",
        )?;
        release.now();
        wait_until(budget, || Ok(first_weak.upgrade().is_none()))?;
        check(
            entries.load(Ordering::SeqCst) == 1 && inner_calls.load(Ordering::SeqCst) == 1,
            "shutdown decoded queued work or lost active execution",
        )?;
        check(
            observed.try_recv().is_err(),
            "pending decoder unexpectedly entered",
        )?;
        Ok(Evidence {
            pending_audio_freed_before_release: true,
            active_audio_held_before_release: true,
            third_submit_micros: None,
            wrapper_entries: 1,
            inner_decoder_calls: 1,
            completed_session: None,
            completed_generation: None,
            completed_job_id: None,
            result_exactly_once: None,
            baseline_text_equal: None,
            cleanup_micros: 0,
            shutdown_observed_blocked_millis: Some(100),
        })
    })();
    release.now();
    let cleanup_result = if let Some(worker) = service {
        shutdown_bounded(worker, budget)
    } else {
        match cleanup {
            Some(Ok(join)) => join.finish(budget),
            Some(Err(error)) => Err(error),
            None => Err("shutdown ownership missing; join unverified".into()),
        }
    };
    conclude(outcome, cleanup_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed {
        calls: Arc<AtomicUsize>,
        fail: usize,
    }
    impl Decoder for Fixed {
        fn decode(
            &mut self,
            samples: &[f32],
            language: Language,
        ) -> Result<String, InferenceError> {
            if language != Language::German || samples != [0.1, -0.2, 0.3] {
                return Err(InferenceError::InvalidAudio);
            }
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == self.fail {
                Err(InferenceError::Decode)
            } else {
                Ok("controlled same-input result".into())
            }
        }
    }
    #[test]
    fn restart_uses_real_service_cancellation_ownership_and_result_identity() -> Result<(), String>
    {
        let calls = Arc::new(AtomicUsize::new(0));
        let evidence = restart(
            Fixed {
                calls: calls.clone(),
                fail: 0,
            },
            vec![0.1, -0.2, 0.3],
            Duration::from_secs(2),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(
            evidence.pending_audio_freed_before_release
                && evidence.active_audio_held_before_release
        );
        assert_eq!(
            (
                evidence.completed_session,
                evidence.completed_generation,
                evidence.completed_job_id
            ),
            (Some(3), Some(71), Some(303))
        );
        assert_eq!(evidence.baseline_text_equal, Some(true));
        assert_eq!(evidence.result_exactly_once, Some(true));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        Ok(())
    }
    #[test]
    fn shutdown_releases_pending_audio_but_joins_after_the_old_decoder_executes()
    -> Result<(), String> {
        let calls = Arc::new(AtomicUsize::new(0));
        let evidence = shutdown_held(
            Fixed {
                calls: calls.clone(),
                fail: 0,
            },
            vec![0.1, -0.2, 0.3],
            Duration::from_secs(2),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert!(
            evidence.pending_audio_freed_before_release
                && evidence.active_audio_held_before_release
        );
        assert_eq!(evidence.wrapper_entries, 1);
        assert_eq!(evidence.shutdown_observed_blocked_millis, Some(100));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn premature_shutdown_result_is_rejected_and_its_failure_remains_available_to_cleanup()
    -> Result<(), String> {
        use everlast_dictation::inference_service::ServiceError;
        for fail in [false, true] {
            let (send, received) = crossbeam_channel::bounded(1);
            send.send(if fail {
                Err(ServiceError::Worker)
            } else {
                Ok(())
            })
            .map_err(|e| e.to_string())?;
            let thread = thread::spawn(|| {});
            let mut cleanup = Cleanup {
                thread,
                received,
                started: Instant::now(),
                observed: None,
            };
            let rejected = cleanup
                .expect_incomplete()
                .err()
                .ok_or("premature result accepted")?;
            assert!(
                rejected
                    .to_string()
                    .contains("before the held decoder was released")
            );
            let result = cleanup.finish(Duration::from_secs(1));
            if fail {
                assert_eq!(
                    result.err().as_deref(),
                    Some(
                        "service shutdown failed: Inference worker could not be started or joined"
                    )
                );
            } else {
                result?;
            }
        }
        Ok(())
    }
    #[test]
    fn backend_failure_is_reported_and_cleanup_still_completes() {
        let calls = Arc::new(AtomicUsize::new(0));
        let result = restart(
            Fixed {
                calls: calls.clone(),
                fail: 3,
            },
            vec![0.1, -0.2, 0.3],
            Duration::from_secs(2),
        );
        let failure = result
            .err()
            .unwrap_or_else(|| Failure::before_worker("unexpected success"));
        assert!(failure.reason.contains("transcription failed"));
        assert!(failure.cleanup_error.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
