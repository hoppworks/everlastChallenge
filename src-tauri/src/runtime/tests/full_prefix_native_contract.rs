//! Device-free native-owner contract: real DSP, controller, service and native
//! mailbox; device construction/OS delivery are controlled, not accepted here.
use super::*;
use crate::pipeline::Policy;
type TestResult = Result<(), Box<dyn std::error::Error>>;

struct OwnerAudio {
    owner: CpalAudio,
    ready: Sender<crate::audio::CapturePort>,
}
impl AudioDriver for OwnerAudio {
    fn policy(&self) -> Policy {
        self.owner.policy
    }
    fn start(&mut self, sid: u64, _: Option<String>) -> Result<(), String> {
        if self.owner.active.is_some() {
            return Err("owner still active".into());
        }
        let handle = CaptureHandle::start_test_source(
            sid,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            self.ready.clone(),
            self.policy(),
        )
        .map_err(|e| e.to_string())?;
        self.owner.active = Some((sid, handle));
        Ok(())
    }
    fn stop(&self, sid: u64) {
        self.owner.stop(sid);
    }
    fn poll(&mut self) -> Vec<CaptureEvent> {
        self.owner.poll()
    }
    fn allow_seal(&self, epoch: u64) {
        self.owner.allow_seal(epoch);
    }
    fn shutdown(&mut self) -> Result<(), String> {
        self.owner.shutdown()
    }
}
struct Held {
    entered: Sender<Vec<f32>>,
    release: Receiver<()>,
    first: bool,
}
impl Decoder for Held {
    fn decode(&mut self, pcm: &[f32], _: Language) -> Result<String, InferenceError> {
        self.entered
            .send(pcm.to_vec())
            .map_err(|_| InferenceError::Decode)?;
        if self.first {
            self.first = false;
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| InferenceError::Decode)?;
        }
        Ok(if pcm.len() == 19200 {
            "internal revision"
        } else {
            "whole final"
        }
        .into())
    }
}
struct Fixture {
    runtime: Option<Runtime>,
    native: NativeEndpoint,
    ready: Receiver<crate::audio::CapturePort>,
    entries: Receiver<Vec<f32>>,
    release: Sender<()>,
}
impl Fixture {
    fn new(policy: Policy) -> Result<Self, Box<dyn std::error::Error>> {
        let (ready, input) = bounded(1);
        let (entered, entries) = bounded(4);
        let (release, gate) = bounded(1);
        let (port, native) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(Held {
                entered,
                release: gate,
                first: true,
            })?,
            1,
            port,
            OwnerAudio {
                owner: CpalAudio {
                    policy,
                    ..CpalAudio::default()
                },
                ready,
            },
        )?;
        wait_for(&runtime.client(), |s| s.model_ready)?;
        Ok(Self {
            runtime: Some(runtime),
            native,
            ready: input,
            entries,
            release,
        })
    }
    fn client(&self) -> Result<Client, Box<dyn std::error::Error>> {
        Ok(self.runtime.as_ref().ok_or("missing runtime")?.client())
    }
    fn start(
        &self,
        sid: u64,
    ) -> Result<(crate::audio::CapturePort, DeliveryToken), Box<dyn std::error::Error>> {
        let client = self.client()?;
        accepted(client.start(settings(), None, true, false)?)?;
        let token = loop {
            match self.native.requests.recv_timeout(Duration::from_secs(2))? {
                NativeCommand::Cancel(old) if old < sid => {}
                NativeCommand::Capture { session, token, .. } if session.get() == sid => {
                    break token;
                }
                _ => return Err("unexpected native command before capture".into()),
            }
        };
        assert!(
            self.ready.is_empty(),
            "capture acknowledgement precedes audio owner"
        );
        self.native.responses.send(NativeResult::Captured {
            session: sid,
            result: Ok(()),
        })?;
        let input = self.ready.recv_timeout(Duration::from_secs(2))?;
        wait_for(&client, |s| {
            s.session.phase == Phase::Recording && s.session.session == Some(sid)
        })?;
        Ok((input, token))
    }
    fn finish(&mut self) -> TestResult {
        if self.release.try_send(()).is_err() { /* worker already released */ }
        self.runtime.take().ok_or("missing runtime")?.join()?;
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.release.try_send(()).is_err() { /* worker released/disconnected */ }
        if let Some(runtime) = self.runtime.take()
            && runtime.join().is_err()
        { /* preserve original assertion/error */ }
    }
}
fn feed(port: &mut crate::audio::CapturePort, pcm: &[f32]) {
    for chunk in pcm.chunks(320) {
        port.publish(chunk);
        thread::sleep(Duration::from_millis(1));
    }
}
fn voice() -> Vec<f32> {
    let mut pcm = vec![0.1; 16000];
    pcm.resize(24000, 0.0);
    pcm
}
fn expect_delivery(f: &Fixture, sid: u64, token: &DeliveryToken) -> TestResult {
    let delivered = loop {
        match f.native.requests.recv_timeout(Duration::from_secs(2))? {
            NativeCommand::Cancel(old) if old < sid => {}
            NativeCommand::Deliver {
                session,
                text,
                token,
                ..
            } if session == sid => {
                assert_eq!(text, "whole final");
                break token;
            }
            _ => return Err("unexpected/stale native delivery".into()),
        }
    };
    assert!(delivered.same_work(token));
    delivered.try_commit()?;
    f.native.responses.send(NativeResult::Delivered {
        session: sid,
        result: DeliveryResult::Sent,
    })?;
    let state = wait_for(&f.client()?, |s| s.session.phase == Phase::Idle)?;
    assert_eq!(state.session.outcome, Some(Outcome::Sent));
    assert!(f.entries.is_empty());
    Ok(())
}
#[test]
fn shared_full_prefix_owner_keeps_revisions_internal_and_delivers_only_whole_final() -> TestResult {
    let mut f = Fixture::new(Policy::FullPrefix)?;
    let client = f.client()?;
    let (mut input, token) = f.start(1)?;
    let mut pcm = voice();
    feed(&mut input, &pcm);
    assert_eq!(
        f.entries.recv_timeout(Duration::from_secs(2))?,
        pcm[..19200]
    );
    assert_eq!(client.snapshot()?.session.phase, Phase::Recording);
    assert_eq!(client.snapshot()?.session.text, None);
    assert!(f.native.requests.is_empty());
    input.publish(&[0.3125; 137]);
    pcm.extend_from_slice(&[0.3125; 137]);
    client.stop(1)?;
    wait_for(&client, |s| s.session.phase == Phase::Finalizing)?;
    assert!(f.entries.is_empty());
    assert!(f.native.requests.is_empty());
    f.release.send(())?;
    assert_eq!(f.entries.recv_timeout(Duration::from_secs(2))?, pcm);
    expect_delivery(&f, 1, &token)?;
    f.finish()
}
#[test]
fn cancelled_old_prefix_cannot_deliver_or_authorize_another_snapshot_before_actual_release()
-> TestResult {
    let mut f = Fixture::new(Policy::FullPrefix)?;
    let client = f.client()?;
    let (mut first, old) = f.start(1)?;
    feed(&mut first, &voice());
    f.entries.recv_timeout(Duration::from_secs(2))?;
    client.cancel(1)?;
    wait_for(&client, |s| s.session.phase == Phase::Idle)?;
    assert!(!old.is_pending());
    let (mut next, current) = f.start(2)?;
    let pcm = voice();
    feed(&mut next, &pcm);
    assert!(f.entries.recv_timeout(Duration::from_millis(60)).is_err());
    assert_eq!(client.snapshot()?.session.phase, Phase::Recording);
    assert_eq!(client.snapshot()?.session.text, None);
    f.release.send(())?;
    assert_eq!(
        f.entries.recv_timeout(Duration::from_secs(2))?,
        pcm[..19200]
    );
    client.stop(2)?;
    assert_eq!(f.entries.recv_timeout(Duration::from_secs(2))?, pcm);
    expect_delivery(&f, 2, &current)?;
    f.finish()
}
#[test]
fn reference_owner_has_only_the_final_job_and_preserves_the_native_delivery_contract() -> TestResult
{
    let mut f = Fixture::new(Policy::Reference)?;
    let client = f.client()?;
    let (mut input, token) = f.start(1)?;
    let pcm = voice();
    feed(&mut input, &pcm);
    assert!(f.entries.recv_timeout(Duration::from_millis(40)).is_err());
    client.stop(1)?;
    assert_eq!(f.entries.recv_timeout(Duration::from_secs(2))?, pcm);
    f.release.send(())?;
    expect_delivery(&f, 1, &token)?;
    f.finish()
}
