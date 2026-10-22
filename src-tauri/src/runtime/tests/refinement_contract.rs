use super::super::refinement::{Completed, Service};
use super::*;
use crate::{controller::RefinementStatus, refinement_service::owner};
use std::sync::atomic::AtomicUsize;
type TestResult = Result<(), Box<dyn std::error::Error>>;

struct TextDecoder;
impl Decoder for TextDecoder {
    fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
        Ok("Das ist, äh, ein Test.".into())
    }
}
struct ControlledService {
    requests: Sender<owner::Job>,
    results: Receiver<Completed>,
    closed: Arc<AtomicBool>,
    unloads: Arc<AtomicUsize>,
    active: Mutex<Option<crate::refinement_service::Cancellation>>,
}
impl Service for ControlledService {
    fn submit(&self, job: owner::Job) -> Result<(), String> {
        *self.active.lock().map_err(|_| "poisoned")? = Some(job.cancel.clone());
        self.requests
            .try_send(job)
            .map_err(|_| "request slot occupied".into())
    }
    fn poll(&self) -> Result<Option<Completed>, String> {
        match self.results.try_recv() {
            Ok(done) => Ok(Some(done)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("controlled refinement closed".into()),
        }
    }
    fn unload(&self) -> Result<(), String> {
        self.unloads.fetch_add(1, Ordering::AcqRel);
        if let Some(cancel) = self.active.lock().map_err(|_| "poisoned")?.as_ref() {
            cancel.cancel();
        }
        Ok(())
    }
    fn shutdown(self: Box<Self>) -> Result<(), String> {
        self.unload()?;
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}
struct Controls {
    jobs: Receiver<owner::Job>,
    results: Sender<Completed>,
    closed: Arc<AtomicBool>,
    unloads: Arc<AtomicUsize>,
}
fn service() -> (Box<dyn Service>, Controls) {
    let (requests, jobs) = bounded(1);
    let (results, responses) = bounded(1);
    let closed = Arc::new(AtomicBool::new(false));
    let unloads = Arc::new(AtomicUsize::new(0));
    (
        Box::new(ControlledService {
            requests,
            results: responses,
            closed: closed.clone(),
            unloads: unloads.clone(),
            active: Mutex::new(None),
        }),
        Controls {
            jobs,
            results,
            closed,
            unloads,
        },
    )
}
fn start_refinement(
    fixture: &LifecycleFixture,
    controls: &Controls,
) -> Result<owner::Job, Box<dyn std::error::Error>> {
    let mut settings = settings();
    settings.refinement = true;
    fixture.record_with(settings)?;
    fixture.runtime.client().stop(1)?;
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Ok(vec![0.1; 800]),
        maximum: false,
    })?;
    let job = controls.jobs.recv_timeout(Duration::from_secs(2))?;
    wait_for(&fixture.runtime.client(), |s| {
        matches!(s.session.refinement.status, RefinementStatus::Processing)
    })?;
    Ok(job)
}
fn completed(job: owner::Job) -> Result<Completed, Box<dyn std::error::Error>> {
    use dictation_core::refinement::{EditKind, ProposedEdit, validate};
    let accepted = validate(
        &job.original,
        &[ProposedEdit {
            kind: EditKind::Filler,
            before: ", äh,".into(),
            after: "".into(),
            left_context: None,
            right_context: None,
        }],
        job.language.into(),
    )?;
    Ok(Completed {
        session: job.session.get(),
        original: job.original,
        result: Ok(accepted),
    })
}
fn settle_polls(client: &Client) -> TestResult {
    // Each acknowledged request is handled after a complete actor poll. Two
    // round trips also cover a first request arriving while the actor waited.
    for _ in 0..2 {
        assert!(
            acknowledged(client.clear_text_if_unchanged("not the displayed text".into())?)?
                .is_err()
        );
    }
    Ok(())
}
#[test]
fn runtime_waits_for_refinement_then_delivers_only_the_validated_text() -> TestResult {
    let (service, controls) = service();
    let fixture = LifecycleFixture::with_services(TextDecoder, Some(service))?;
    let job = start_refinement(&fixture, &controls)?;
    assert_eq!(job.original.as_str(), "Das ist, äh, ein Test.");
    assert!(matches!(
        fixture.native.requests.try_recv(),
        Err(TryRecvError::Empty)
    ));
    controls.results.send(completed(job)?)?;
    match fixture
        .native
        .requests
        .recv_timeout(Duration::from_secs(2))?
    {
        NativeCommand::Deliver {
            session,
            text,
            token,
        } => {
            assert_eq!(session, 1);
            assert_eq!(text, "Das ist ein Test.");
            token.try_commit()?;
            fixture.native.responses.send(NativeResult::Delivered {
                session,
                result: DeliveryResult::Sent,
            })?;
        }
        _ => return Err("unexpected native command".into()),
    }
    wait_for(&fixture.runtime.client(), |s| {
        s.session.outcome == Some(Outcome::Sent)
    })?;
    fixture.runtime.join()?;
    assert!(controls.closed.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn failed_refinement_service_keeps_the_original_and_the_asr_runtime_alive() -> TestResult {
    for revoked_by_failure in [false, true] {
        let (service, controls) = service();
        let fixture = LifecycleFixture::with_services(TextDecoder, Some(service))?;
        let job = start_refinement(&fixture, &controls)?;
        if revoked_by_failure {
            job.cancel.cancel();
        }
        let Controls {
            results, closed, ..
        } = controls;
        drop(results);
        let snapshot = wait_for(&fixture.runtime.client(), |s| s.error.is_some())?;
        assert!(snapshot.running && snapshot.model_ready);
        assert_eq!(snapshot.session.phase, Phase::Idle);
        assert_eq!(
            snapshot.session.text.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        assert!(matches!(
            snapshot.session.refinement.status,
            RefinementStatus::Failed(_)
        ));
        assert!(job.cancel.is_cancelled());
        assert!(closed.load(Ordering::Acquire));
        for request in fixture.native.requests.try_iter() {
            assert!(!matches!(request, NativeCommand::Deliver { .. }));
        }
        accepted(
            fixture
                .runtime
                .client()
                .start(settings(), None, false, false)?,
        )?;
        fixture.runtime.join()?;
    }
    Ok(())
}

#[test]
fn sleep_revokes_refinement_unloads_once_and_discards_its_late_result() -> TestResult {
    let (service, controls) = service();
    let fixture = LifecycleFixture::with_services(TextDecoder, Some(service))?;
    let job = start_refinement(&fixture, &controls)?;
    let cancel = job.cancel.clone();
    let client = fixture.runtime.client();
    client.suspend()?;
    assert!(cancel.is_cancelled());
    controls.results.send(completed(job)?)?;
    settle_polls(&client)?;
    let snapshot = wait_for(&client, |s| s.session.phase == Phase::Idle)?;
    assert_eq!(
        snapshot.session.text.as_deref(),
        Some("Das ist, äh, ein Test.")
    );
    assert!(matches!(
        snapshot.session.refinement.status,
        RefinementStatus::Cancelled
    ));
    assert_eq!(controls.unloads.load(Ordering::Acquire), 1);
    client.suspend()?;
    accepted(client.clear_text()?)?;
    assert_eq!(controls.unloads.load(Ordering::Acquire), 1);
    assert!(
        fixture
            .native
            .requests
            .try_iter()
            .all(|request| !matches!(request, NativeCommand::Deliver { .. }))
    );
    client.resume()?;
    fixture.runtime.join()?;
    assert!(controls.closed.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn caller_cancellation_wins_over_a_simultaneous_service_failure() -> TestResult {
    let (service, controls) = service();
    let fixture = LifecycleFixture::with_services(TextDecoder, Some(service))?;
    let job = start_refinement(&fixture, &controls)?;
    let client = fixture.runtime.client();
    client.cancel(1)?;
    drop(controls.results);
    let snapshot = wait_for(&client, |s| {
        s.error.is_some() && s.session.phase == Phase::Idle
    })?;
    assert!(snapshot.running && snapshot.model_ready);
    assert_eq!(snapshot.session.outcome, Some(Outcome::Cancelled));
    assert!(matches!(
        snapshot.session.refinement.status,
        RefinementStatus::Cancelled
    ));
    assert!(job.cancel.is_cancelled());
    fixture.runtime.join()?;
    Ok(())
}

#[test]
fn missing_refinement_service_does_not_silently_release_original_text() -> TestResult {
    let fixture = LifecycleFixture::with_decoder(TextDecoder)?;
    let mut preferences = settings();
    preferences.refinement = true;
    fixture.record_with(preferences)?;
    let client = fixture.runtime.client();
    client.stop(1)?;
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Ok(vec![0.1; 800]),
        maximum: false,
    })?;
    let snapshot = wait_for(&client, |s| s.session.phase == Phase::Idle)?;
    assert!(matches!(
        snapshot.session.refinement.status,
        RefinementStatus::Failed(_)
    ));
    assert_eq!(
        snapshot.session.text.as_deref(),
        Some("Das ist, äh, ein Test.")
    );
    assert!(
        fixture
            .native
            .requests
            .try_iter()
            .all(|request| !matches!(request, NativeCommand::Deliver { .. }))
    );
    fixture.runtime.join()?;
    Ok(())
}

#[test]
fn a_late_old_result_cannot_enter_the_next_recording() -> TestResult {
    let (service, controls) = service();
    let fixture = LifecycleFixture::with_services(TextDecoder, Some(service))?;
    let old = start_refinement(&fixture, &controls)?;
    let client = fixture.runtime.client();
    client.cancel(1)?;
    wait_for(&client, |s| s.session.phase == Phase::Idle)?;
    accepted(client.start(settings(), None, false, false)?)?;
    assert_eq!(fixture.starts.recv_timeout(Duration::from_secs(1))?, 2);
    fixture.audio.send(CaptureEvent::Started {
        session: 2,
        format: AudioFormat {
            sample_rate: 16_000,
            channels: 1,
        },
        started: Instant::now(),
    })?;
    wait_for(&client, |s| {
        s.session.session == Some(2) && s.session.phase == Phase::Recording
    })?;
    controls.results.send(completed(old)?)?;
    settle_polls(&client)?;
    let snapshot = client.snapshot()?;
    assert_eq!(snapshot.session.session, Some(2));
    assert_eq!(snapshot.session.phase, Phase::Recording);
    assert_eq!(
        snapshot.session.text.as_deref(),
        Some("Das ist, äh, ein Test.")
    );
    assert!(
        fixture
            .native
            .requests
            .try_iter()
            .all(|request| !matches!(request, NativeCommand::Deliver { .. }))
    );
    fixture.runtime.join()?;
    Ok(())
}

struct FailingAudio;
impl AudioDriver for FailingAudio {
    fn start(&mut self, _: u64, _: Option<String>) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self, _: u64) {}
    fn poll(&mut self) -> Vec<CaptureEvent> {
        Vec::new()
    }
    fn shutdown(&mut self) -> Result<(), String> {
        Err("audio cleanup failed".into())
    }
}
struct HeldCleanup {
    entered: Sender<()>,
    release: Receiver<()>,
}
impl Service for HeldCleanup {
    fn submit(&self, _: owner::Job) -> Result<(), String> {
        Err("unused".into())
    }
    fn poll(&self) -> Result<Option<Completed>, String> {
        Ok(None)
    }
    fn unload(&self) -> Result<(), String> {
        Ok(())
    }
    fn shutdown(self: Box<Self>) -> Result<(), String> {
        self.entered.send(()).map_err(|_| "observer closed")?;
        self.release
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "cleanup release missing")?;
        Err("text cleanup failed".into())
    }
}
#[test]
fn shutdown_waits_for_text_owner_after_audio_failure_and_preserves_both_errors() -> TestResult {
    let (entered, waiting) = bounded(1);
    let (release, released) = bounded(1);
    let (native, _mailbox) = native_mailbox();
    let inference = InferenceService::with_decoder(TextDecoder)?;
    let runtime = Runtime::with_services(
        inference,
        1,
        native,
        FailingAudio,
        Some(Box::new(HeldCleanup {
            entered,
            release: released,
        })),
    )?;
    let client = runtime.client();
    let (finished, result) = bounded(1);
    let join = thread::spawn(move || {
        if finished.send(runtime.join()).is_err() { /* test observer already failed */ }
    });
    let _cleanup = ReleaseOwners {
        gates: vec![release.clone()],
        join: Some(join),
    };
    waiting.recv_timeout(Duration::from_secs(2))?;
    assert!(
        matches!(result.try_recv(), Err(TryRecvError::Empty)),
        "join returned before text owner cleanup"
    );
    release.send(())?;
    assert!(result.recv_timeout(Duration::from_secs(2))?.is_err());
    let snapshot = client.snapshot()?;
    assert!(!snapshot.running);
    let error = snapshot.error.ok_or("missing cleanup errors")?;
    assert!(error.contains("audio cleanup failed"), "{error}");
    assert!(error.contains("text cleanup failed"), "{error}");
    Ok(())
}
