//! Public PCM -> actual reference capture/actor/service connector. The decoder
//! and native endpoint are controlled: no model, device, AX, clipboard or keys.
use super::*;
use macos_integration::delivery::CommitError;
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::atomic::AtomicUsize};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TEXT: &str = "controlled public-fixture transcript, not ASR";
const PCM_HASH: &str = "ebd52851100536db02d12c49fddd010372dcdc70243562e057553d476b706ae0";
const FRAMES: usize = 176_000;

fn public_pcm() -> TestResult<Vec<i16>> {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/jfk.wav"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e"
    );
    let mut wav = hound::WavReader::new(Cursor::new(bytes))?;
    assert_eq!(wav.spec().channels, 1);
    assert_eq!(wav.spec().sample_rate, 16_000);
    assert_eq!(wav.spec().bits_per_sample, 16);
    assert_eq!(wav.spec().sample_format, hound::SampleFormat::Int);
    assert_eq!(wav.duration() as usize, FRAMES);
    let pcm = wav.samples::<i16>().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(pcm.len(), FRAMES);
    Ok(pcm)
}

struct ReferenceAudio {
    owner: CpalAudio,
    ready: Sender<crate::audio::CapturePort>,
    reaped: Sender<(u64, bool)>,
}
impl AudioDriver for ReferenceAudio {
    fn start(&mut self, session: u64, _: Option<String>) -> Result<(), String> {
        if self.owner.active.is_some() {
            return Err("previous capture owner has not joined".into());
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
    fn take_progress(&mut self) -> Option<CaptureProgress> {
        self.owner.take_progress()
    }
    fn shutdown(&mut self) -> Result<(), String> {
        self.owner.shutdown()
    }
}

struct CheckedDecoder {
    entered: Sender<()>,
    release: Receiver<()>,
    calls: Arc<AtomicUsize>,
}
impl Decoder for CheckedDecoder {
    fn decode(&mut self, pcm: &[f32], language: Language) -> Result<String, InferenceError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        let mut hash = Sha256::new();
        for sample in pcm {
            hash.update(sample.to_le_bytes());
        }
        // Independent literal binding: PCM16 normalized by 32768, generated
        // from the public fixture without invoking the production DSP.
        if pcm.len() != FRAMES
            || format!("{:x}", hash.finalize()) != PCM_HASH
            || language != Language::English
        {
            return Err(InferenceError::InvalidAudio);
        }
        self.entered.send(()).map_err(|_| InferenceError::Decode)?;
        self.release
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| InferenceError::Decode)?;
        Ok(TEXT.into())
    }
}

struct Fixture {
    runtime: Option<Runtime>,
    native: NativeEndpoint,
    ready: Receiver<crate::audio::CapturePort>,
    reaped: Receiver<(u64, bool)>,
    entered: Receiver<()>,
    release: Sender<()>,
    calls: Arc<AtomicUsize>,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let (ready, input) = bounded(1);
        let (reaped, observed) = bounded(2);
        let (entered, entry) = bounded(2);
        let (release, gate) = bounded(1);
        let (port, native) = native_mailbox();
        let calls = Arc::new(AtomicUsize::new(0));
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(CheckedDecoder {
                entered,
                release: gate,
                calls: Arc::clone(&calls),
            })?,
            1,
            port,
            ReferenceAudio {
                owner: CpalAudio::default(),
                ready,
                reaped,
            },
        )?;
        wait_for(&runtime.client(), |state| state.model_ready)?;
        Ok(Self {
            runtime: Some(runtime),
            native,
            ready: input,
            reaped: observed,
            entered: entry,
            release,
            calls,
        })
    }
    fn client(&self) -> TestResult<Client> {
        Ok(self
            .runtime
            .as_ref()
            .ok_or("runtime already joined")?
            .client())
    }
    fn attach_native_owner(
        &mut self,
    ) -> TestResult<(
        crate::native_service::NativeService,
        crate::native_service::contract_support::Sink,
    )> {
        // Transfer the real endpoint into the production owner. From here no
        // test can manufacture Capture/Delivered replies on that endpoint.
        let unused = native_mailbox().1;
        let endpoint = std::mem::replace(&mut self.native, unused);
        let sink = crate::native_service::contract_support::Sink::new();
        let service = crate::native_service::contract_support::spawn(endpoint, sink.clone())?;
        Ok((service, sink))
    }
    fn start_with_native_owner(
        &self,
        has_start_process: bool,
    ) -> TestResult<crate::audio::CapturePort> {
        let client = self.client()?;
        let mut config = settings();
        config.language = Language::English;
        let process = has_start_process
            .then(|| ProcessIdentity::read(std::process::id() as i32))
            .transpose()?;
        accepted(client.start(config, process, true, false)?)?;
        let input = self.ready.recv_timeout(Duration::from_secs(2))?;
        wait_for(&client, |state| state.session.phase == Phase::Recording)?;
        Ok(input)
    }
    fn start(
        &self,
        target: Result<(), String>,
    ) -> TestResult<(crate::audio::CapturePort, DeliveryToken)> {
        let client = self.client()?;
        let mut config = settings();
        config.language = Language::English;
        accepted(client.start(config, None, true, false)?)?;
        let token = match self.native.requests.recv_timeout(Duration::from_secs(2))? {
            NativeCommand::Capture { session, token, .. } if session.get() == 1 => token,
            _ => return Err("target capture must precede recording".into()),
        };
        assert!(self.ready.is_empty());
        self.native.responses.send(NativeResult::Captured {
            session: 1,
            result: target,
        })?;
        let input = self.ready.recv_timeout(Duration::from_secs(2))?;
        wait_for(&client, |state| {
            state.session.session == Some(1) && state.session.phase == Phase::Recording
        })?;
        Ok((input, token))
    }
    fn feed_and_stop(&self, input: &mut crate::audio::CapturePort) -> TestResult {
        // Deliberately faster than real time; bounded callback chunks let the
        // actual one-second ring drain. This is not a latency measurement.
        for chunk in public_pcm()?.chunks(320) {
            input.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            self.entered.is_empty(),
            "reference must not decode before Stop"
        );
        self.client()?.stop(1)?;
        self.entered.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(self.reaped.recv_timeout(Duration::from_secs(2))?, (1, true));
        assert_eq!(self.calls.load(Ordering::Acquire), 1);
        Ok(())
    }
    fn delivery(&self, original: &DeliveryToken) -> TestResult<DeliveryToken> {
        match self.native.requests.recv_timeout(Duration::from_secs(2))? {
            NativeCommand::Deliver {
                session,
                text,
                token,
            } => {
                assert_eq!(session, 1);
                assert_eq!(text, TEXT);
                assert!(token.same_work(original));
                Ok(token)
            }
            _ => Err("expected current fixture delivery".into()),
        }
    }
    fn finish(&mut self, expected_calls: usize) -> TestResult {
        self.runtime
            .take()
            .ok_or("runtime already joined")?
            .join()?;
        assert_eq!(self.calls.load(Ordering::Acquire), expected_calls);
        for command in self.native.requests.try_iter() {
            assert!(
                matches!(command, NativeCommand::Cancel(1 | 2)),
                "no extra capture or delivery"
            );
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.release.try_send(()).is_err() { /* already released or disconnected */ }
        if let Some(runtime) = self.runtime.take()
            && runtime.join().is_err()
        { /* preserve primary failure */ }
    }
}

#[test]
fn public_fixture_stop_preserves_pcm_and_commits_exactly_one_current_delivery() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (mut input, original) = fixture.start(Ok(()))?;
    fixture.feed_and_stop(&mut input)?;
    fixture.client()?.stop(1)?; // Duplicate stop cannot create another final job.
    fixture.release.send(())?;
    let delivered = fixture.delivery(&original)?;
    delivered.try_commit()?;
    assert_eq!(original.try_commit(), Err(CommitError::AlreadyCommitted));
    fixture.native.responses.send(NativeResult::Delivered {
        session: 1,
        result: DeliveryResult::Sent,
    })?;
    let state = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle
    })?;
    assert_eq!(state.session.outcome, Some(Outcome::Sent));
    assert_eq!(state.session.text.as_deref(), Some(TEXT));
    fixture.finish(1)
}

#[test]
fn public_fixture_cancel_rejects_held_late_result_and_never_delivers() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (mut input, original) = fixture.start(Ok(()))?;
    fixture.feed_and_stop(&mut input)?;
    fixture.client()?.cancel(1)?;
    assert_eq!(original.try_commit(), Err(CommitError::Cancelled));
    let cancelled = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle
    })?;
    assert_eq!(cancelled.session.outcome, Some(Outcome::Cancelled));
    assert_eq!(cancelled.session.text, None);
    // Admit a real new owner while the old decode is still held. The second
    // decoder entry below proves the old result crossed the worker boundary
    // before shutdown; it must not become the new session's text or delivery.
    let client = fixture.client()?;
    let mut config = settings();
    config.language = Language::English;
    accepted(client.start(config, None, false, false)?)?;
    let mut next = fixture.ready.recv_timeout(Duration::from_secs(2))?;
    wait_for(&client, |state| {
        state.session.session == Some(2) && state.session.phase == Phase::Recording
    })?;
    fixture.release.send(())?;
    for chunk in public_pcm()?.chunks(320) {
        next.publish(chunk);
        thread::sleep(Duration::from_millis(1));
    }
    client.stop(2)?;
    fixture.entered.recv_timeout(Duration::from_secs(2))?;
    assert_eq!(
        fixture.reaped.recv_timeout(Duration::from_secs(2))?,
        (2, true)
    );
    let current = client.snapshot()?;
    assert_eq!(current.session.session, Some(2));
    assert_eq!(current.session.phase, Phase::Finalizing);
    assert_eq!(current.session.text, None);
    client.cancel(2)?;
    wait_for(&client, |state| state.session.phase == Phase::Idle)?;
    fixture.release.send(())?;
    fixture.finish(2)?;
    assert_eq!(client.snapshot()?.session.outcome, Some(Outcome::Cancelled));
    assert_eq!(client.snapshot()?.session.text, None);
    Ok(())
}

#[test]
fn public_fixture_failed_target_retains_text_but_cannot_commit_delivery() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (mut input, original) = fixture.start(Err("controlled target unavailable".into()))?;
    assert_eq!(original.try_commit(), Err(CommitError::Cancelled));
    fixture.feed_and_stop(&mut input)?;
    fixture.release.send(())?;
    // Actual runtime contract: a revoked-token request can reach the native
    // mailbox. The native boundary must hold it, never authorize a key event.
    let delivered = fixture.delivery(&original)?;
    assert_eq!(delivered.try_commit(), Err(CommitError::Cancelled));
    fixture.native.responses.send(NativeResult::Delivered {
        session: 1,
        result: DeliveryResult::Held("controlled target unavailable".into()),
    })?;
    let state = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle
    })?;
    assert_eq!(
        state.session.outcome,
        Some(Outcome::Held("controlled target unavailable".into()))
    );
    assert_eq!(state.session.text.as_deref(), Some(TEXT));
    assert_eq!(
        state.error.as_deref(),
        Some("controlled target unavailable")
    );
    fixture.finish(1)
}

#[test]
fn joined_native_owner_delivers_public_pcm_once_and_restores_its_preparation() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (service, sink) = fixture.attach_native_owner()?;
    let mut input = fixture.start_with_native_owner(true)?;
    fixture.feed_and_stop(&mut input)?;
    fixture.client()?.stop(1)?;
    fixture.release.send(())?;
    let state = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Sent)
    })?;
    assert_eq!(state.session.text.as_deref(), Some(TEXT));
    fixture.finish(1)?;
    service.join()?;
    let (effects, delivered) = sink.observed()?;
    assert_eq!(delivered, [TEXT]);
    assert_eq!(
        effects,
        [
            "capture",
            "environment",
            "approve",
            "backup",
            "prepare",
            "environment",
            "approve",
            "install",
            "environment",
            "approve",
            "send",
            "cleanup",
            "shutdown",
        ]
    );
    Ok(())
}

#[test]
fn joined_native_owner_missing_start_process_preserves_text_without_platform_effects() -> TestResult
{
    let mut fixture = Fixture::new()?;
    let (service, sink) = fixture.attach_native_owner()?;
    let mut input = fixture.start_with_native_owner(false)?;
    fixture.feed_and_stop(&mut input)?;
    fixture.release.send(())?;
    let state = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle
    })?;
    assert!(matches!(state.session.outcome, Some(Outcome::Held(_))));
    assert_eq!(state.session.text.as_deref(), Some(TEXT));
    fixture.finish(1)?;
    service.join()?;
    let (effects, delivered) = sink.observed()?;
    assert!(delivered.is_empty());
    assert_eq!(effects, ["shutdown"]);
    Ok(())
}

#[test]
fn joined_native_owner_changed_target_retains_text_before_any_clipboard_effect() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (service, sink) = fixture.attach_native_owner()?;
    let mut input = fixture.start_with_native_owner(true)?;
    fixture.feed_and_stop(&mut input)?;
    // The original capture succeeded; the target disappears during decode.
    sink.set_target_available(false);
    fixture.release.send(())?;
    let state = wait_for(&fixture.client()?, |state| {
        state.session.phase == Phase::Idle
    })?;
    assert_eq!(
        state.session.outcome,
        Some(Outcome::Held("controlled target unavailable".into()))
    );
    assert_eq!(state.session.text.as_deref(), Some(TEXT));
    fixture.finish(1)?;
    service.join()?;
    let (effects, delivered) = sink.observed()?;
    assert!(delivered.is_empty());
    assert_eq!(effects, ["capture", "environment", "approve", "shutdown"]);
    Ok(())
}

#[test]
fn joined_native_owner_cancelled_decode_never_reaches_platform_delivery() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (service, sink) = fixture.attach_native_owner()?;
    let mut input = fixture.start_with_native_owner(true)?;
    fixture.feed_and_stop(&mut input)?;
    let client = fixture.client()?;
    client.cancel(1)?;
    let state = wait_for(&client, |state| state.session.phase == Phase::Idle)?;
    assert_eq!(state.session.outcome, Some(Outcome::Cancelled));
    assert_eq!(state.session.text, None);
    fixture.release.send(())?;
    // Joining waits for the held decoder to return, not just an early empty
    // mailbox observation. Its stale result must still have no native effect.
    fixture.finish(1)?;
    service.join()?;
    assert_eq!(client.snapshot()?.session.text, None);
    let (effects, delivered) = sink.observed()?;
    assert!(delivered.is_empty());
    assert_eq!(effects, ["capture", "shutdown"]);
    Ok(())
}
