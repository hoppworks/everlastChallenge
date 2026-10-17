//! P03 state-machine contracts. These tests do not simulate OS scheduling or worker shutdown.
use dictation_core::session::{
    DeliveryResult, Effect, Fact, Machine, Outcome, Phase, SessionError,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn in_phase(phase: Phase) -> Result<Machine, SessionError> {
    let mut machine = Machine::default();
    if phase == Phase::Idle {
        return Ok(machine);
    }
    machine.start(true, true)?;
    if phase == Phase::Starting {
        return Ok(machine);
    }
    machine.accept(Fact::AudioStarted(1))?;
    if phase == Phase::Recording {
        return Ok(machine);
    }
    if phase == Phase::Cancelling {
        machine.cancel(1, false)?;
        return Ok(machine);
    }
    machine.stop(1)?;
    if phase == Phase::Finalizing {
        return Ok(machine);
    }
    machine.accept(Fact::AudioReleased(1))?;
    machine.accept(Fact::Transcript {
        id: 1,
        text: "current words".into(),
    })?;
    Ok(machine)
}

#[test]
fn all_active_phases_reject_new_starts_without_consuming_identity() -> TestResult {
    let mut idle = in_phase(Phase::Idle)?;
    assert_eq!(idle.start(false, false), Err(SessionError::NotReady));
    assert_eq!(idle.start(true, false)?, vec![Effect::Capture(1)]);

    for phase in [
        Phase::Starting,
        Phase::Recording,
        Phase::Finalizing,
        Phase::Delivering,
        Phase::Cancelling,
    ] {
        let mut machine = in_phase(phase)?;
        assert_eq!(machine.phase(), phase);
        for ready in [false, true] {
            assert_eq!(
                machine.start(ready, true),
                Err(SessionError::Busy),
                "{phase:?}"
            );
            assert_eq!(machine.phase(), phase);
            assert_eq!(machine.session_id(), Some(1));
        }
        machine.cancel(1, false)?;
        machine.accept(Fact::AudioReleased(1))?;
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(
            machine.start(true, false)?,
            vec![Effect::Capture(2)],
            "{phase:?}"
        );
    }
    Ok(())
}

#[test]
fn stop_is_idempotent_in_every_phase_and_starting_remembers_it_once() -> TestResult {
    for phase in [
        Phase::Idle,
        Phase::Starting,
        Phase::Recording,
        Phase::Finalizing,
        Phase::Delivering,
        Phase::Cancelling,
    ] {
        let mut machine = in_phase(phase)?;
        assert!(machine.stop(99)?.is_empty(), "foreign stop in {phase:?}");
        let expected = if phase == Phase::Recording {
            vec![Effect::StopAudio(1)]
        } else {
            Vec::new()
        };
        assert_eq!(machine.stop(1)?, expected, "{phase:?}");
        assert!(machine.stop(1)?.is_empty(), "duplicate stop in {phase:?}");
        if phase == Phase::Starting {
            assert_eq!(
                machine.accept(Fact::AudioStarted(1))?,
                vec![Effect::StopAudio(1)]
            );
            assert_eq!(machine.phase(), Phase::Finalizing);
            assert!(machine.accept(Fact::AudioStarted(1))?.is_empty());
            assert_eq!(
                machine.accept(Fact::AudioReleased(1))?,
                vec![Effect::Transcribe(1)]
            );
            assert!(machine.accept(Fact::AudioReleased(1))?.is_empty());
        }
    }
    Ok(())
}

#[test]
fn cancellation_before_release_ignores_work_results_and_requires_the_audio_barrier() -> TestResult {
    for phase in [Phase::Starting, Phase::Recording, Phase::Finalizing] {
        let mut machine = in_phase(phase)?;
        assert_eq!(
            machine.cancel(1, false)?,
            vec![Effect::Revoke(1), Effect::StopAudio(1)],
            "{phase:?}"
        );
        assert_eq!(machine.phase(), Phase::Cancelling);
        assert!(machine.cancel(1, false)?.is_empty());
        assert!(machine.stop(1)?.is_empty());
        assert_eq!(machine.start(true, true), Err(SessionError::Busy));
        assert!(
            machine
                .accept(Fact::Transcript {
                    id: 1,
                    text: "cancelled words".into()
                })?
                .is_empty()
        );
        assert!(
            machine
                .accept(Fact::Delivered {
                    id: 1,
                    result: DeliveryResult::Sent
                })?
                .is_empty()
        );
        assert_eq!(machine.last_text(), None);
        assert_eq!(machine.phase(), Phase::Cancelling);
        assert!(machine.accept(Fact::AudioReleased(1))?.is_empty());
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(machine.outcome(), Some(&Outcome::Cancelled));
        assert_eq!(machine.start(true, false)?, vec![Effect::Capture(2)]);
    }
    Ok(())
}

#[test]
fn cancellation_after_audio_release_does_not_wait_for_inference_or_delivery() -> TestResult {
    for delivered_text in [false, true] {
        let mut machine = in_phase(Phase::Finalizing)?;
        assert_eq!(
            machine.accept(Fact::AudioReleased(1))?,
            vec![Effect::Transcribe(1)]
        );
        if delivered_text {
            assert_eq!(
                machine.accept(Fact::Transcript {
                    id: 1,
                    text: "held text".into()
                })?,
                vec![Effect::Deliver(1)]
            );
        }
        assert_eq!(machine.cancel(1, false)?, vec![Effect::Revoke(1)]);
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(machine.outcome(), Some(&Outcome::Cancelled));
        assert_eq!(machine.last_text(), delivered_text.then_some("held text"));
        assert_eq!(machine.start(true, true)?, vec![Effect::Capture(2)]);
        assert!(
            machine
                .accept(Fact::Transcript {
                    id: 1,
                    text: "late inference".into()
                })?
                .is_empty()
        );
        assert!(
            machine
                .accept(Fact::Delivered {
                    id: 1,
                    result: DeliveryResult::Sent
                })?
                .is_empty()
        );
        assert_eq!(machine.phase(), Phase::Starting);
        assert_eq!(machine.session_id(), Some(2));
    }
    Ok(())
}

fn visible(machine: &Machine) -> (Phase, Option<u64>, Option<String>, Option<Outcome>) {
    (
        machine.phase(),
        machine.session_id(),
        machine.last_text().map(str::to_owned),
        machine.outcome().cloned(),
    )
}

fn obsolete_facts_are_inert(machine: &mut Machine, obsolete: u64) -> TestResult {
    let before = visible(machine);
    assert!(machine.stop(obsolete)?.is_empty());
    assert!(machine.cancel(obsolete, false)?.is_empty());
    for fact in [
        Fact::AudioStarted(obsolete),
        Fact::AudioReleased(obsolete),
        Fact::Transcript {
            id: obsolete,
            text: "obsolete words".into(),
        },
        Fact::Delivered {
            id: obsolete,
            result: DeliveryResult::Sent,
        },
        Fact::Delivered {
            id: obsolete,
            result: DeliveryResult::Held("obsolete hold".into()),
        },
        Fact::Delivered {
            id: obsolete,
            result: DeliveryResult::Failed("obsolete delivery failure".into()),
        },
        Fact::Failed {
            id: obsolete,
            message: "obsolete worker failure".into(),
        },
    ] {
        assert!(machine.accept(fact)?.is_empty());
        assert_eq!(visible(machine), before);
    }
    Ok(())
}

#[test]
fn obsolete_controls_and_all_worker_fact_types_are_inert_in_every_phase() -> TestResult {
    for phase in [
        Phase::Idle,
        Phase::Starting,
        Phase::Recording,
        Phase::Finalizing,
        Phase::Delivering,
        Phase::Cancelling,
    ] {
        let mut machine = in_phase(phase)?;
        obsolete_facts_are_inert(&mut machine, 0)?;
        obsolete_facts_are_inert(&mut machine, 99)?;
    }
    Ok(())
}

#[test]
fn repeated_cancel_restart_sequences_never_adopt_old_work() -> TestResult {
    let mut machine = Machine::default();
    for current in 1..=6 {
        assert_eq!(machine.start(true, true)?, vec![Effect::Capture(current)]);
        for old in 1..current {
            obsolete_facts_are_inert(&mut machine, old)?;
        }
        if current % 2 == 0 {
            machine.accept(Fact::AudioStarted(current))?;
            machine.stop(current)?;
            machine.accept(Fact::AudioReleased(current))?;
            assert_eq!(
                machine.cancel(current, false)?,
                vec![Effect::Revoke(current)]
            );
        } else {
            assert_eq!(
                machine.cancel(current, false)?,
                vec![Effect::Revoke(current), Effect::StopAudio(current)]
            );
            assert_eq!(machine.phase(), Phase::Cancelling);
            assert_eq!(machine.start(true, true), Err(SessionError::Busy));
            machine.accept(Fact::AudioReleased(current))?;
        }
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(machine.outcome(), Some(&Outcome::Cancelled));
        assert_eq!(machine.last_text(), None);
        for old in 1..=current {
            obsolete_facts_are_inert(&mut machine, old)?;
        }
    }
    assert_eq!(machine.start(true, false)?, vec![Effect::Capture(7)]);
    machine.accept(Fact::AudioStarted(7))?;
    machine.stop(7)?;
    machine.accept(Fact::AudioReleased(7))?;
    machine.accept(Fact::Transcript {
        id: 7,
        text: "only the current result".into(),
    })?;
    assert_eq!(machine.last_text(), Some("only the current result"));
    assert_eq!(machine.outcome(), Some(&Outcome::Local));
    Ok(())
}

#[test]
fn empty_failed_and_cancelled_sessions_keep_the_previous_text_until_replaced() -> TestResult {
    let mut machine = Machine::default();
    machine.start(true, false)?;
    machine.accept(Fact::AudioStarted(1))?;
    machine.stop(1)?;
    machine.accept(Fact::AudioReleased(1))?;
    machine.accept(Fact::Transcript {
        id: 1,
        text: "retained text".into(),
    })?;

    for current in 2..=4 {
        machine.start(true, false)?;
        assert_eq!(machine.last_text(), Some("retained text"));
        machine.accept(Fact::AudioStarted(current))?;
        machine.stop(current)?;
        machine.accept(Fact::AudioReleased(current))?;
        match current {
            2 => {
                assert!(
                    machine
                        .accept(Fact::Transcript {
                            id: current,
                            text: String::new()
                        })?
                        .is_empty()
                );
                assert_eq!(machine.outcome(), Some(&Outcome::Empty));
            }
            3 => {
                assert_eq!(
                    machine.accept(Fact::Failed {
                        id: current,
                        message: "decoder unavailable".into()
                    })?,
                    vec![Effect::Revoke(current)]
                );
                assert_eq!(
                    machine.outcome(),
                    Some(&Outcome::Failed("decoder unavailable".into()))
                );
            }
            _ => {
                assert_eq!(
                    machine.cancel(current, false)?,
                    vec![Effect::Revoke(current)]
                );
                assert_eq!(machine.outcome(), Some(&Outcome::Cancelled));
            }
        }
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(machine.last_text(), Some("retained text"));
    }
    machine.start(true, false)?;
    machine.accept(Fact::AudioStarted(5))?;
    machine.stop(5)?;
    machine.accept(Fact::AudioReleased(5))?;
    machine.accept(Fact::Transcript {
        id: 5,
        text: "replacement text".into(),
    })?;
    assert_eq!(machine.last_text(), Some("replacement text"));
    Ok(())
}

#[test]
fn every_delivery_outcome_preserves_text_and_ignores_a_second_completion() -> TestResult {
    for (result, expected) in [
        (DeliveryResult::Sent, Outcome::Sent),
        (
            DeliveryResult::Held("field changed".into()),
            Outcome::Held("field changed".into()),
        ),
        (
            DeliveryResult::Failed("clipboard failed".into()),
            Outcome::Failed("clipboard failed".into()),
        ),
    ] {
        let mut machine = in_phase(Phase::Delivering)?;
        assert!(
            machine
                .accept(Fact::Delivered { id: 1, result })?
                .is_empty()
        );
        assert_eq!(machine.phase(), Phase::Idle);
        assert_eq!(machine.session_id(), None);
        assert_eq!(machine.last_text(), Some("current words"));
        assert_eq!(machine.outcome(), Some(&expected));
        assert!(
            machine
                .accept(Fact::Delivered {
                    id: 1,
                    result: DeliveryResult::Failed("duplicate".into())
                })?
                .is_empty()
        );
        assert_eq!(machine.outcome(), Some(&expected));
        assert_eq!(machine.last_text(), Some("current words"));
    }
    Ok(())
}

#[test]
fn current_audio_cleanup_failure_after_cancel_stays_visible_without_new_work() -> TestResult {
    let mut machine = in_phase(Phase::Recording)?;
    machine.cancel(1, false)?;
    assert_eq!(
        machine.accept(Fact::Failed {
            id: 1,
            message: "audio cleanup failed".into()
        })?,
        vec![Effect::Revoke(1), Effect::StopAudio(1)]
    );
    assert_eq!(machine.phase(), Phase::Cancelling);
    assert_eq!(machine.start(true, true), Err(SessionError::Busy));
    assert!(
        machine
            .accept(Fact::Transcript {
                id: 1,
                text: "revoked inference".into()
            })?
            .is_empty()
    );
    assert!(machine.accept(Fact::AudioReleased(1))?.is_empty());
    assert_eq!(machine.phase(), Phase::Idle);
    assert_eq!(
        machine.outcome(),
        Some(&Outcome::Failed("audio cleanup failed".into()))
    );
    assert_eq!(machine.last_text(), None);
    assert_eq!(machine.start(true, false)?, vec![Effect::Capture(2)]);
    obsolete_facts_are_inert(&mut machine, 1)?;
    Ok(())
}
