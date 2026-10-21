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

// This second scenario is shared by the model-free harness test and the explicit
// native test. The barrier is BEFORE inner.decode: it proves normal Runtime start
// admission while the old serial worker is occupied, never kernel interruption.
struct HeldRestartDecoder<D> {
    inner: D,
    calls: usize,
    entered: Sender<(usize, usize)>,
    completed: Sender<Result<(), String>>,
    release: Receiver<()>,
    blocked: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}
impl<D: Decoder> Decoder for HeldRestartDecoder<D> {
    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        self.calls += 1;
        if self.calls == 1 {
            self.blocked.store(true, Ordering::Release);
        }
        self.entered
            .send((self.calls, audio.len()))
            .map_err(|_| InferenceError::InvalidAudio)?;
        if self.calls == 1 {
            self.release
                .recv_timeout(Duration::from_secs(120))
                .map_err(|_| InferenceError::InvalidAudio)?;
            self.blocked.store(false, Ordering::Release);
        }
        let result = self.inner.decode(audio, language);
        self.completed
            .send(result.as_ref().map(|_| ()).map_err(ToString::to_string))
            .map_err(|_| InferenceError::InvalidAudio)?;
        result
    }
}
impl<D> Drop for HeldRestartDecoder<D> {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn held_old_decode_then_short_restart(
    decoder: impl Decoder + Send,
    long: Vec<f32>,
    short: Vec<f32>,
    expected: &str,
    completion_budget: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let long_frames = long.len();
    let short_frames = short.len();
    let (release, released) = bounded(1);
    let (entered, decoding) = bounded(3);
    let (completed, results) = bounded(3);
    let blocked = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut cleanup = RestartCleanup {
        release,
        fixture: Some(LifecycleFixture::with_decoder(HeldRestartDecoder {
            inner: decoder,
            calls: 0,
            entered,
            completed,
            release: released,
            blocked: Arc::clone(&blocked),
            dropped: Arc::clone(&dropped),
        })?),
    };
    // Return errors through this closure so explicit shutdown is still checked;
    // the existing RAII guard also releases the barrier on assertion unwinding.
    let scenario = (|| -> Result<(), Box<dyn std::error::Error>> {
        let fixture = cleanup.fixture.as_ref().ok_or("restart fixture missing")?;
        let client = fixture.runtime.client();
        let old = start_recording(fixture, 1)?;
        client.stop(1)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(long),
            maximum: false,
        })?;
        assert_eq!(
            decoding.recv_timeout(Duration::from_secs(2))?,
            (1, long_frames)
        );
        assert!(blocked.load(Ordering::Acquire));
        client.cancel(1)?;
        wait_for(&client, |snapshot| {
            snapshot.session.phase == Phase::Idle
                && snapshot.session.outcome == Some(Outcome::Cancelled)
        })?;
        assert!(!old.is_pending());

        let current = start_recording(fixture, 2)?;
        // Recording ACK is observed BEFORE release, not after waiting for decoder
        // quiescence. Audio here is controlled; no CPAL/native recipient is used.
        assert!(blocked.load(Ordering::Acquire));
        assert!(matches!(results.try_recv(), Err(TryRecvError::Empty)));
        client.stop(2)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 2);
        fixture.pause()?;
        fixture.audio.send(CaptureEvent::Finished {
            session: 2,
            result: Ok(short),
            maximum: false,
        })?;
        let processed = client.clear_text()?;
        fixture.proceed()?;
        accepted(processed)?;
        let pending = client.snapshot()?;
        assert_eq!(pending.session.session, Some(2));
        assert_eq!(pending.session.phase, Phase::Finalizing);
        assert!(pending.session.text.is_none());
        assert!(blocked.load(Ordering::Acquire));
        assert!(matches!(decoding.try_recv(), Err(TryRecvError::Empty)));

        cleanup.release.send(())?;
        // Actual canceled decode must finish successfully too: discarded errors
        // cannot silently count as a successful native lifecycle observation.
        results.recv_timeout(completion_budget)??;
        assert_eq!(
            decoding.recv_timeout(Duration::from_secs(2))?,
            (2, short_frames)
        );
        results.recv_timeout(completion_budget)??;
        let token = loop {
            match fixture
                .native
                .requests
                .recv_timeout(Duration::from_secs(2))?
            {
                NativeCommand::Cancel(1) => {}
                NativeCommand::Deliver {
                    session: 2,
                    text,
                    token,
                } => {
                    if text != expected {
                        return Err(
                            "session 2 text differs from the pinned repeatability baseline".into(),
                        );
                    }
                    break token;
                }
                _ => return Err("obsolete result adopted or unexpected native request".into()),
            }
        };
        assert!(token.same_work(&current));
        assert!(!old.is_pending());
        token.try_commit()?;
        fixture.native.responses.send(NativeResult::Delivered {
            session: 2,
            result: DeliveryResult::Sent,
        })?;
        let finished = wait_for(&client, |snapshot| {
            snapshot.session.phase == Phase::Idle && snapshot.session.outcome == Some(Outcome::Sent)
        })?;
        if finished.session.text.as_deref() != Some(expected) {
            return Err("final snapshot lost the current session text".into());
        }
        assert_eq!(finished.error, None);
        assert!(matches!(
            fixture.native.requests.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert!(matches!(decoding.try_recv(), Err(TryRecvError::Empty)));
        assert!(matches!(results.try_recv(), Err(TryRecvError::Empty)));
        Ok(())
    })();
    // A failed scenario must release a still-held worker before join. Unlike the
    // unwind-only fallback, explicit cleanup preserves join errors in the result.
    if cleanup.release.try_send(()).is_err() { /* released or worker gone */ }
    let fixture = cleanup
        .fixture
        .take()
        .ok_or("restart fixture missing at join")?;
    if fixture.continue_poll.try_send(()).is_err() { /* not paused or worker gone */ }
    let joined = fixture.runtime.join();
    let owners_released = fixture.closed.load(Ordering::Acquire) && dropped.load(Ordering::Acquire);
    if let Err(error) = joined {
        return Err(format!(
            "restart scenario error: {}; cleanup join failed: {error}",
            scenario
                .as_ref()
                .err()
                .map_or_else(|| "none".into(), ToString::to_string)
        )
        .into());
    }
    if !owners_released {
        return Err("restart cleanup did not release both audio and decoder owners".into());
    }
    scenario?;
    assert!(matches!(
        decoding.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    assert!(matches!(
        results.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
    Ok(())
}

#[test]
fn held_decoder_does_not_block_new_recording_and_old_result_cannot_deliver()
-> Result<(), Box<dyn std::error::Error>> {
    struct TwoResults(usize);
    impl Decoder for TwoResults {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            self.0 += 1;
            match self.0 {
                1 => Ok("obsolete long result".into()),
                2 => Ok("current short result".into()),
                _ => Err(InferenceError::InvalidAudio),
            }
        }
    }
    held_old_decode_then_short_restart(
        TwoResults(0),
        vec![0.1; 800],
        vec![0.2; 320],
        "current short result",
        Duration::from_secs(2),
    )
}

/// Explicit native counterpart: exactly one 300-s and one 8.192-s decode in one
/// loaded Engine. No microphone, paste, timing/RSS qualification or gold accuracy;
/// the short reference is a prior ASR repeatability baseline. The long result's
/// content is deliberately not asserted, but its actual decode must succeed.
#[test]
#[ignore = "requires pinned local reference/VAD models and authorized long/short fixtures"]
fn native_300s_cancel_admits_short_recording_before_old_decode_finishes()
-> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("EVERLAST_RUN_NATIVE_RUNTIME_RESTART").as_deref() != Ok("1") {
        return Err(
            "set EVERLAST_RUN_NATIVE_RUNTIME_RESTART=1 for this explicit native test".into(),
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    let manifest = pinned_restart_bytes(
        &root.join("artifacts/diagnostic-speech/long-control-v1/manifest.json"),
        1_048_576,
        "d03a4aa23d2f5a5fe67671ddb298801b64af4368e8eab90510eafc30bcf2ed37",
    )?;
    // Keep the manifest pin independently checked; references are never inferred
    // from model output or changed by this test.
    drop(manifest);
    let long = pinned_restart_wav(
        &root.join("artifacts/diagnostic-speech/long-control-v1/controlled-300s.wav"),
        "80b79393de147e78e9b9a86bad3d88998dae4056a9fe10a080ef2468c92dc25b",
        4_800_000,
    )?;
    let short = pinned_restart_wav(
        &root.join("artifacts/replay/neue-aufnahme.wav"),
        "9a108246c7b1196e8203ee22ebacabe7927d18f7a37a5f2e7647437db7df069c",
        131_072,
    )?;
    let reference = String::from_utf8(pinned_restart_bytes(
        &root.join("artifacts/replay/neue-aufnahme-baseline.txt"),
        65_536,
        "6e0700c02c1f08629895d05206fbeeb924afaea30fa6d3e49e3356136622cc67",
    )?)?;
    if reference.is_empty() {
        return Err("empty native repeatability reference".into());
    }
    let models = root.join("artifacts/models");
    let model = models.join(crate::model::REFERENCE.file_name());
    crate::model::verify_file(&model, crate::model::REFERENCE)?;
    crate::model::verify_file(
        &models.join(crate::model::VAD.file_name()),
        crate::model::VAD,
    )?;
    let engine = crate::transcription::Engine::load(&model, crate::model::REFERENCE)?;
    held_old_decode_then_short_restart(engine, long, short, &reference, Duration::from_secs(120))
}

fn pinned_restart_bytes(
    path: &std::path::Path,
    maximum: u64,
    expected_hash: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum || format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
        return Err("pinned native restart fixture size/hash mismatch".into());
    }
    Ok(bytes)
}

fn pinned_restart_wav(
    path: &std::path::Path,
    expected_hash: &str,
    frames: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let bytes = pinned_restart_bytes(path, 16 * 1024 * 1024, expected_hash)?;
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes))?;
    let spec = reader.spec();
    if (
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample,
        spec.sample_format,
    ) != (1, 16_000, 16, hound::SampleFormat::Int)
        || reader.duration() != frames
    {
        return Err("native restart fixture is not the pinned PCM16 mono 16k frame count".into());
    }
    Ok(reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32768.0))
        .collect::<Result<Vec<_>, _>>()?)
}
