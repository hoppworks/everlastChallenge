use dictation_core::session::{Effect, Fact, Machine, Outcome, Phase};

#[test]
fn failures_wait_for_release_and_allow_a_new_session_after_cleanup()
-> Result<(), Box<dyn std::error::Error>> {
    use dictation_core::session::SessionError;
    let mut machine = Machine::default();
    assert_eq!(machine.start(false, true), Err(SessionError::NotReady));
    assert_eq!(machine.start(true, true)?, vec![Effect::Capture(1)]);
    assert_eq!(machine.start(true, true), Err(SessionError::Busy));
    assert_eq!(
        machine.accept(Fact::Failed {
            id: 1,
            message: "stream_lost".into()
        })?,
        vec![Effect::Revoke(1), Effect::StopAudio(1)]
    );
    assert_eq!(machine.phase(), Phase::Cancelling);
    machine.accept(Fact::AudioReleased(1))?;
    assert_eq!(machine.phase(), Phase::Idle);
    assert_eq!(
        machine.outcome(),
        Some(&Outcome::Failed("stream_lost".into()))
    );
    machine.start(true, false)?;
    machine.accept(Fact::AudioStarted(2))?;
    machine.stop(2)?;
    machine.accept(Fact::AudioReleased(2))?;
    assert_eq!(
        machine.accept(Fact::Failed {
            id: 2,
            message: "decoder_failed".into()
        })?,
        vec![Effect::Revoke(2)]
    );
    assert_eq!(machine.phase(), Phase::Idle);
    Ok(())
}

#[test]
fn stop_during_start_and_duplicate_worker_facts_complete_only_once()
-> Result<(), Box<dyn std::error::Error>> {
    use dictation_core::session::{DeliveryResult, SessionError};
    let mut machine = Machine::default();
    machine.start(true, true)?;
    assert!(machine.stop(1)?.is_empty());
    assert!(
        machine
            .accept(Fact::Transcript {
                id: 1,
                text: "zu früh".into()
            })?
            .is_empty()
    );
    assert_eq!(
        machine.accept(Fact::AudioStarted(1))?,
        vec![Effect::StopAudio(1)]
    );
    assert_eq!(
        machine.accept(Fact::AudioReleased(1))?,
        vec![Effect::Transcribe(1)]
    );
    assert!(machine.accept(Fact::AudioReleased(1))?.is_empty());
    assert_eq!(
        machine.accept(Fact::Transcript {
            id: 1,
            text: "genau einmal".into()
        })?,
        vec![Effect::Deliver(1)]
    );
    assert!(
        machine
            .accept(Fact::Transcript {
                id: 1,
                text: "doppelt".into()
            })?
            .is_empty()
    );
    assert_eq!(
        machine.cancel(1, true),
        Err(SessionError::DeliveryCommitted)
    );
    machine.accept(Fact::Delivered {
        id: 1,
        result: DeliveryResult::Sent,
    })?;
    assert_eq!(machine.outcome(), Some(&Outcome::Sent));
    assert_eq!(machine.last_text(), Some("genau einmal"));
    assert_eq!(machine.phase(), Phase::Idle);
    Ok(())
}

#[test]
fn cancel_waits_only_for_microphone_release_and_never_delivers_stale_text()
-> Result<(), Box<dyn std::error::Error>> {
    let mut machine = Machine::default();
    machine.start(true, true)?;
    assert_eq!(
        machine.cancel(1, false)?,
        vec![Effect::Revoke(1), Effect::StopAudio(1)]
    );
    assert_eq!(machine.phase(), Phase::Cancelling);
    assert!(machine.start(true, true).is_err());
    assert_eq!(
        machine.accept(Fact::AudioStarted(1))?,
        vec![Effect::StopAudio(1)]
    );
    assert!(machine.accept(Fact::AudioReleased(1))?.is_empty());
    assert_eq!(machine.phase(), Phase::Idle);
    assert_eq!(machine.start(true, true)?, vec![Effect::Capture(2)]);
    assert!(
        machine
            .accept(Fact::Transcript {
                id: 1,
                text: "alte Inferenz".into()
            })?
            .is_empty()
    );
    assert_eq!(machine.phase(), Phase::Starting);
    assert_eq!(machine.last_text(), None);
    Ok(())
}

#[test]
fn recording_finalizes_after_release_and_keeps_the_window_result()
-> Result<(), Box<dyn std::error::Error>> {
    let mut machine = Machine::default();
    assert_eq!(machine.start(true, false)?, vec![Effect::Capture(1)]);
    assert_eq!(machine.phase(), Phase::Starting);
    machine.accept(Fact::AudioStarted(1))?;
    assert_eq!(machine.phase(), Phase::Recording);
    assert_eq!(machine.stop(1)?, vec![Effect::StopAudio(1)]);
    assert_eq!(machine.phase(), Phase::Finalizing);
    assert_eq!(
        machine.accept(Fact::AudioReleased(1))?,
        vec![Effect::Transcribe(1)]
    );
    assert!(
        machine
            .accept(Fact::Transcript {
                id: 1,
                text: "Vielen Dank fürs Zuschauen".into()
            })?
            .is_empty()
    );
    assert_eq!(machine.phase(), Phase::Idle);
    assert_eq!(machine.last_text(), Some("Vielen Dank fürs Zuschauen"));
    assert_eq!(machine.outcome(), Some(&Outcome::Local));
    Ok(())
}

#[test]
fn clearing_retained_text_does_not_cancel_a_new_recording() -> Result<(), Box<dyn std::error::Error>>
{
    let mut machine = Machine::default();
    machine.start(true, false)?;
    machine.accept(Fact::AudioStarted(1))?;
    machine.stop(1)?;
    machine.accept(Fact::AudioReleased(1))?;
    machine.accept(Fact::Transcript {
        id: 1,
        text: "previous words".into(),
    })?;
    machine.start(true, false)?;
    machine.clear_text();
    assert_eq!(machine.last_text(), None);
    assert_eq!(machine.phase(), Phase::Starting);
    assert_eq!(machine.session_id(), Some(2));
    machine.accept(Fact::AudioStarted(2))?;
    assert_eq!(machine.phase(), Phase::Recording);
    Ok(())
}
