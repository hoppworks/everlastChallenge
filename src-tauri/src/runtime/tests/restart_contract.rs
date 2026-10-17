// A12 host wiring with a real serial InferenceService and controlled audio/native
// boundaries. Vec -> Arc journal ownership is private; this does not measure
// journal deallocation, native device release, or SDK inference cancellation.
use super::*;

struct RestartDecoder {
    first: bool,
    entered: Sender<Vec<f32>>,
    release: Receiver<()>,
    old_blocked: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}
impl Decoder for RestartDecoder {
    fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
        let first = self.first;
        self.first = false;
        if first {
            self.old_blocked.store(true, Ordering::Release);
        }
        self.entered
            .send(audio.to_vec())
            .map_err(|_| InferenceError::InvalidAudio)?;
        if first {
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| InferenceError::InvalidAudio)?;
            self.old_blocked.store(false, Ordering::Release);
            Ok("obsolete session one".into())
        } else {
            Ok("current session three".into())
        }
    }
}
impl Drop for RestartDecoder {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

struct RestartCleanup {
    release: Sender<()>,
    fixture: Option<LifecycleFixture>,
}
impl Drop for RestartCleanup {
    fn drop(&mut self) {
        if self.release.try_send(()).is_err() { /* released or worker gone */ }
        if let Some(fixture) = self.fixture.take() {
            if fixture.continue_poll.try_send(()).is_err() { /* not paused or owner gone */ }
            if fixture.runtime.join().is_err() { /* preserve the original test failure */ }
        }
    }
}

fn start_recording(
    fixture: &LifecycleFixture,
    session_id: u64,
) -> Result<DeliveryToken, Box<dyn std::error::Error>> {
    let client = fixture.runtime.client();
    accepted(client.start(settings(), None, true, false)?)?;
    let token = loop {
        match fixture
            .native
            .requests
            .recv_timeout(Duration::from_secs(1))?
        {
            NativeCommand::Cancel(id) if id < session_id => {}
            NativeCommand::Capture { session, token, .. } if session.get() == session_id => {
                break token;
            }
            _ => return Err("unexpected native command before current target capture".into()),
        }
    };
    fixture.native.responses.send(NativeResult::Captured {
        session: session_id,
        result: Ok(()),
    })?;
    assert_eq!(
        fixture.starts.recv_timeout(Duration::from_secs(1))?,
        session_id
    );
    fixture.audio.send(CaptureEvent::Started {
        session: session_id,
        format: AudioFormat {
            sample_rate: 16_000,
            channels: 1,
        },
        started: Instant::now(),
    })?;
    wait_for(&client, |snapshot| {
        snapshot.session.session == Some(session_id) && snapshot.session.phase == Phase::Recording
    })?;
    Ok(token)
}

#[test]
fn cancel_restart_cancel_restart_keeps_audio_barriers_and_only_delivers_current_text()
-> Result<(), Box<dyn std::error::Error>> {
    let (release, released) = bounded(1);
    let (entered, decoding) = bounded(3);
    let old_blocked = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut cleanup = RestartCleanup {
        release,
        fixture: None,
    };
    cleanup.fixture = Some(LifecycleFixture::with_decoder(RestartDecoder {
        first: true,
        entered,
        release: released,
        old_blocked: Arc::clone(&old_blocked),
        dropped: Arc::clone(&dropped),
    })?);
    let fixture = cleanup.fixture.as_ref().ok_or("fixture missing")?;
    let client = fixture.runtime.client();

    let first_token = start_recording(fixture, 1)?;
    client.stop(1)?;
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Ok(vec![0.1; 800]),
        maximum: false,
    })?;
    assert_eq!(
        decoding.recv_timeout(Duration::from_secs(1))?,
        vec![0.1; 800]
    );
    assert!(old_blocked.load(Ordering::Acquire));
    client.cancel(1)?;
    wait_for(&client, |snapshot| snapshot.session.phase == Phase::Idle)?;
    assert!(!first_token.is_pending());

    let second_token = start_recording(fixture, 2)?;
    client.cancel(2)?;
    assert!(!second_token.is_pending());
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 2);
    wait_for(&client, |snapshot| {
        snapshot.session.phase == Phase::Cancelling
    })?;
    // Acknowledged rejection is a controller barrier, not a timed absence.
    assert!(acknowledged(client.start(settings(), None, true, false)?)?.is_err());
    assert_eq!(client.snapshot()?.session.session, Some(2));
    assert!(matches!(
        fixture.starts.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(old_blocked.load(Ordering::Acquire));
    assert!(matches!(decoding.try_recv(), Err(TryRecvError::Empty)));

    // Accepted audio arriving at the cancelled recording's real release
    // boundary must be discarded. An obsolete duplicate cannot release it.
    fixture.pause()?;
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Ok(vec![0.9; 800]),
        maximum: false,
    })?;
    let stale_audio_processed = client.clear_text()?;
    fixture.proceed()?;
    accepted(stale_audio_processed)?;
    assert_eq!(client.snapshot()?.session.phase, Phase::Cancelling);
    assert_eq!(client.snapshot()?.session.session, Some(2));
    fixture.audio.send(CaptureEvent::Finished {
        session: 2,
        result: Ok(vec![0.2; 800]),
        maximum: false,
    })?;
    wait_for(&client, |snapshot| {
        snapshot.session.phase == Phase::Idle
            && snapshot.session.outcome == Some(Outcome::Cancelled)
    })?;

    let third_token = start_recording(fixture, 3)?;
    assert!(third_token.is_pending());
    assert!(old_blocked.load(Ordering::Acquire));
    client.stop(3)?;
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 3);
    // Hold the real actor at its audio boundary, queue Finished plus an
    // acknowledged request, then resume: the audio is processed before that ack.
    fixture.pause()?;
    fixture.audio.send(CaptureEvent::Finished {
        session: 3,
        result: Ok(vec![0.3; 800]),
        maximum: false,
    })?;
    let audio_processed = client.clear_text()?;
    fixture.proceed()?;
    accepted(audio_processed)?;
    let finalizing = client.snapshot()?;
    assert_eq!(finalizing.session.session, Some(3));
    assert_eq!(finalizing.session.phase, Phase::Finalizing);
    assert_eq!(finalizing.session.text, None);
    assert!(old_blocked.load(Ordering::Acquire));
    assert!(matches!(decoding.try_recv(), Err(TryRecvError::Empty)));

    cleanup.release.send(())?;
    assert_eq!(
        decoding.recv_timeout(Duration::from_secs(1))?,
        vec![0.3; 800]
    );
    let delivery_token = loop {
        match fixture
            .native
            .requests
            .recv_timeout(Duration::from_secs(1))?
        {
            NativeCommand::Cancel(id) if id == 1 || id == 2 => {}
            NativeCommand::Deliver {
                session: 3,
                text,
                token,
            } => {
                assert_eq!(text, "current session three");
                break token;
            }
            _ => return Err("old session delivered or capture repeated".into()),
        }
    };
    assert!(delivery_token.same_work(&third_token));
    assert!(!old_blocked.load(Ordering::Acquire));
    assert!(!first_token.is_pending());
    assert!(!second_token.is_pending());
    // Late native results are also session-tagged and cannot finish session 3.
    fixture.pause()?;
    fixture.native.responses.send(NativeResult::Delivered {
        session: 1,
        result: DeliveryResult::Sent,
    })?;
    fixture.proceed()?;
    // Re-entering the next poll proves the previous native-result drain and
    // snapshot publication have completed, without clearing the current text.
    fixture.pause()?;
    let after_stale = client.snapshot()?;
    assert_eq!(after_stale.session.phase, Phase::Delivering);
    assert_eq!(after_stale.session.session, Some(3));
    assert_eq!(
        after_stale.session.text.as_deref(),
        Some("current session three")
    );
    assert!(third_token.is_pending());
    fixture.proceed()?;
    delivery_token.try_commit()?;
    fixture.native.responses.send(NativeResult::Delivered {
        session: 3,
        result: DeliveryResult::Sent,
    })?;
    let finished = wait_for(&client, |snapshot| {
        snapshot.session.phase == Phase::Idle && snapshot.session.outcome == Some(Outcome::Sent)
    })?;
    assert_eq!(
        finished.session.text.as_deref(),
        Some("current session three")
    );
    assert_eq!(finished.error, None);
    assert!(matches!(decoding.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(
        fixture.native.requests.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(
        fixture.starts.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(fixture.stops.try_recv(), Err(TryRecvError::Empty)));

    let fixture = cleanup
        .fixture
        .take()
        .ok_or("fixture missing at shutdown")?;
    fixture.runtime.join()?;
    assert!(fixture.closed.load(Ordering::Acquire));
    assert!(dropped.load(Ordering::Acquire));
    assert!(matches!(
        decoding.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    Ok(())
}
