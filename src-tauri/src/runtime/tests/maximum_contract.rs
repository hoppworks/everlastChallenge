use super::*;
use std::sync::atomic::AtomicUsize;

struct CountedMaximumDecoder(Arc<AtomicUsize>);
impl Decoder for CountedMaximumDecoder {
    fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        if audio != [0.25_f32; 800] {
            return Err(InferenceError::InvalidAudio);
        }
        Ok("maximum recording words".into())
    }
}

#[test]
fn automatic_maximum_finalizes_once_without_a_user_stop_and_ignores_duplicate_finish()
-> Result<(), Box<dyn std::error::Error>> {
    // This verifies the real host actor/service wiring using controlled audio
    // facts. It does not wait 300 seconds or open a microphone device.
    let calls = Arc::new(AtomicUsize::new(0));
    let fixture = LifecycleFixture::with_decoder(CountedMaximumDecoder(Arc::clone(&calls)))?;
    let client = fixture.runtime.client();
    let captured = fixture.record()?;
    fixture
        .audio
        .send(CaptureEvent::MaximumWarning { session: 1 })?;
    wait_for(&client, |state| {
        state.maximum_warning && state.session.phase == Phase::Recording
    })?;
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(matches!(fixture.stops.try_recv(), Err(TryRecvError::Empty)));

    // The owner ends capture itself. No Client::stop precedes this terminal fact.
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Ok(vec![0.25; 800]),
        maximum: true,
    })?;
    // A late duplicate error must not replace the accepted complete journal.
    fixture.audio.send(CaptureEvent::Finished {
        session: 1,
        result: Err(crate::audio::CaptureError::Stream),
        maximum: true,
    })?;
    let delivered = match fixture
        .native
        .requests
        .recv_timeout(Duration::from_secs(1))?
    {
        NativeCommand::Deliver {
            session,
            text,
            token,
        } => {
            assert_eq!(session, 1);
            assert_eq!(text, "maximum recording words");
            assert!(token.same_work(&captured));
            token
        }
        _ => return Err("automatic maximum did not produce the expected delivery".into()),
    };
    assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
    assert_eq!(calls.load(Ordering::Acquire), 1);
    // A user Stop arriving after automatic finalization is harmless too.
    client.stop(1)?;
    delivered.try_commit()?;
    fixture.native.responses.send(NativeResult::Delivered {
        session: 1,
        result: DeliveryResult::Sent,
    })?;
    let finished = wait_for(&client, |state| {
        state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Sent)
    })?;
    assert_eq!(
        finished.session.text.as_deref(),
        Some("maximum recording words")
    );
    assert!(finished.error.is_none());
    assert!(finished.maximum_reached);
    assert!(finished.maximum_warning);

    // This acknowledged request crosses another actor poll after the duplicate
    // and late Stop, so empty queues are not merely an early observation.
    accepted(client.clear_text()?)?;
    assert!(client.snapshot()?.maximum_reached);
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(matches!(
        fixture.native.requests.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(fixture.stops.try_recv(), Err(TryRecvError::Empty)));
    fixture.runtime.join()?;
    assert!(fixture.closed.load(Ordering::Acquire));
    assert!(!client.snapshot()?.running);
    Ok(())
}
