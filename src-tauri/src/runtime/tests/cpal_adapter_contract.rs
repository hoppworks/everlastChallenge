//! Device-free connector contract: real CaptureHandle/RecordingOwner, CpalAudio
//! poll/join and Runtime actor. No CPAL device, native decoder or OS delivery.
use super::*;
use std::sync::atomic::AtomicUsize;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct DeviceFreeCpal {
    owner: CpalAudio,
    ready: Sender<crate::audio::CapturePort>,
    reaped: Sender<(u64, bool)>,
    shutdown: Arc<AtomicBool>,
}
impl AudioDriver for DeviceFreeCpal {
    fn start(&mut self, session: u64, _: Option<String>) -> Result<(), String> {
        if self.owner.active.is_some() {
            return Err("previous capture owner is still occupied".into());
        }
        let handle = CaptureHandle::start_test_source(
            session,
            AudioFormat {
                sample_rate: 16_000,
                channels: 1,
            },
            self.ready.clone(),
            crate::pipeline::Policy::Reference,
        )
        .map_err(|error| error.to_string())?;
        self.owner.active = Some((session, handle));
        Ok(())
    }
    fn stop(&self, session: u64) {
        self.owner.stop(session);
    }
    fn poll(&mut self) -> Vec<CaptureEvent> {
        let events = self.owner.poll();
        // Observe only after the real adapter has drained and joined. This does
        // not replace its polling logic or inject a synthetic terminal event.
        for event in &events {
            if let CaptureEvent::Finished { session, .. } = event {
                assert!(
                    self.reaped
                        .try_send((*session, self.owner.active.is_none()))
                        .is_ok()
                );
            }
        }
        events
    }
    fn take_progress(&mut self) -> Option<crate::audio::CaptureProgress> {
        self.owner.take_progress()
    }
    fn shutdown(&mut self) -> Result<(), String> {
        let result = self.owner.shutdown();
        self.shutdown.store(true, Ordering::Release);
        result
    }
}

struct CountedDecoder {
    calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}
impl Decoder for CountedDecoder {
    fn decode(&mut self, samples: &[f32], _: Language) -> Result<String, InferenceError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if samples != [0.25_f32; 3_200] {
            return Err(InferenceError::InvalidAudio);
        }
        Ok("previous completed transcript".into())
    }
}
impl Drop for CountedDecoder {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

struct Fixture {
    runtime: Option<Runtime>,
    ready: Receiver<crate::audio::CapturePort>,
    reaped: Receiver<(u64, bool)>,
    native: NativeEndpoint,
    calls: Arc<AtomicUsize>,
    audio_shutdown: Arc<AtomicBool>,
    decoder_dropped: Arc<AtomicBool>,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let (ready, input) = bounded(1);
        let (reaped, observed) = bounded(3);
        let (port, native) = native_mailbox();
        let calls = Arc::new(AtomicUsize::new(0));
        let audio_shutdown = Arc::new(AtomicBool::new(false));
        let decoder_dropped = Arc::new(AtomicBool::new(false));
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(CountedDecoder {
                calls: Arc::clone(&calls),
                dropped: Arc::clone(&decoder_dropped),
            })?,
            1,
            port,
            DeviceFreeCpal {
                owner: CpalAudio::default(),
                ready,
                reaped,
                shutdown: Arc::clone(&audio_shutdown),
            },
        )?;
        let fixture = Self {
            runtime: Some(runtime),
            ready: input,
            reaped: observed,
            native,
            calls,
            audio_shutdown,
            decoder_dropped,
        };
        wait_for(&fixture.client()?, |state| state.model_ready)?;
        Ok(fixture)
    }
    fn client(&self) -> TestResult<Client> {
        Ok(self
            .runtime
            .as_ref()
            .ok_or("runtime already joined")?
            .client())
    }
    fn recording(&self, session: u64) -> TestResult<crate::audio::CapturePort> {
        let input = self.ready.recv_timeout(Duration::from_secs(2))?;
        wait_for(&self.client()?, |state| {
            state.session.session == Some(session) && state.session.phase == Phase::Recording
        })?;
        Ok(input)
    }
    fn only_cleanup_notifications(&self) -> TestResult {
        while let Ok(command) = self.native.requests.try_recv() {
            if !matches!(command, NativeCommand::Cancel(_)) {
                return Err("unexpected native work after capture failure".into());
            }
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take()
            && runtime.join().is_err()
        { /* Preserve the original test failure, but always request/join cleanup. */
        }
    }
}

#[test]
fn actual_capture_error_crosses_cpal_poll_retains_text_and_releases_for_restart() -> TestResult {
    assert_capture_failure(
        |port| port.publish(&[f32::NAN]),
        crate::audio::CaptureError::InvalidSamples,
    )
}

#[test]
fn device_unavailable_callback_crosses_cpal_poll_and_releases_for_restart() -> TestResult {
    assert_capture_failure(
        |port| {
            let mut callback = port.stream_error_callback();
            callback(cpal::StreamError::DeviceNotAvailable);
        },
        crate::audio::CaptureError::Stream,
    )
}

fn assert_capture_failure(
    fail: fn(&mut crate::audio::CapturePort),
    error: crate::audio::CaptureError,
) -> TestResult {
    let mut fixture = Fixture::new()?;
    let client = fixture.client()?;

    // Establish retained text through the real owner/DSP/service/actor path,
    // rather than editing Controller internals or injecting an old Transcript.
    accepted(client.start(settings(), None, false, false)?)?;
    let mut first = fixture.recording(1)?;
    first.publish(&[0.25_f32; 3_200]);
    client.stop(1)?;
    let completed = wait_for(&client, |state| {
        state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Local)
    })?;
    assert_eq!(
        completed.session.text.as_deref(),
        Some("previous completed transcript")
    );
    assert_eq!(
        fixture.reaped.recv_timeout(Duration::from_secs(2))?,
        (1, true)
    );
    assert_eq!(fixture.calls.load(Ordering::Acquire), 1);
    fixture.only_cleanup_notifications()?;

    // Give session 2 a successful controlled target receipt. A missing target
    // cannot therefore be the reason no delivery follows its capture failure.
    accepted(client.start(settings(), None, true, false)?)?;
    let token = loop {
        match fixture
            .native
            .requests
            .recv_timeout(Duration::from_secs(2))?
        {
            NativeCommand::Cancel(old) if old < 2 => {}
            NativeCommand::Capture { session, token, .. } if session.get() == 2 => break token,
            _ => return Err("missing session 2 target capture".into()),
        }
    };
    fixture.native.responses.send(NativeResult::Captured {
        session: 2,
        result: Ok(()),
    })?;
    let mut failed = fixture.recording(2)?;
    fail(&mut failed);
    let failed_state = wait_for(&client, |state| {
        state.session.phase == Phase::Idle
            && matches!(state.session.outcome, Some(Outcome::Failed(_)))
    })?;
    let expected_error = error.to_string();
    assert_eq!(
        failed_state.session.outcome,
        Some(Outcome::Failed(expected_error))
    );
    assert_eq!(
        failed_state.session.text.as_deref(),
        Some("previous completed transcript")
    );
    assert!(!token.is_pending());
    assert_eq!(
        fixture.reaped.recv_timeout(Duration::from_secs(2))?,
        (2, true)
    );
    assert_eq!(fixture.calls.load(Ordering::Acquire), 1);
    fixture.only_cleanup_notifications()?;

    // A new actual CaptureHandle/owner starts before old producer ports are
    // dropped; the real CpalAudio active slot has already been reaped by poll.
    accepted(client.start(settings(), None, false, false)?)?;
    let _third = fixture.recording(3)?;
    client.cancel(3)?;
    let restarted = wait_for(&client, |state| {
        state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Cancelled)
    })?;
    assert_eq!(
        fixture.reaped.recv_timeout(Duration::from_secs(2))?,
        (3, true)
    );
    assert_eq!(
        restarted.session.text.as_deref(),
        Some("previous completed transcript")
    );
    fixture.runtime.take().ok_or("runtime missing")?.join()?;
    fixture.only_cleanup_notifications()?;
    assert_eq!(fixture.calls.load(Ordering::Acquire), 1);
    assert!(fixture.audio_shutdown.load(Ordering::Acquire));
    assert!(fixture.decoder_dropped.load(Ordering::Acquire));
    assert!(!client.snapshot()?.running);
    Ok(())
}
