//! Host session orchestration: immutable session settings and owned audio moves.
use crate::{
    inference_service::{Job, WorkToken},
    transcription::Language,
};
use dictation_core::session::{
    DeliveryResult, Effect, Fact, Machine, Outcome, Phase, SessionError,
};
use macos_integration::{
    delivery::{CancelOutcome, DeliveryToken},
    target::ProcessIdentity,
};
use std::{num::NonZeroU64, ops::Range, sync::Arc};

#[derive(Clone)]
pub struct SessionSettings {
    pub device: Option<String>,
    pub language: Language,
    pub generation: u64,
}

pub enum Action {
    CaptureTarget {
        session: NonZeroU64,
        process: Option<ProcessIdentity>,
        token: DeliveryToken,
    },
    StartAudio {
        session: u64,
        device: Option<String>,
    },
    StopAudio(u64),
    Transcribe(Job),
    CancelInference(u64),
    Deliver {
        session: u64,
        text: String,
        token: DeliveryToken,
    },
    AbortDelivery(u64),
}

pub enum Event {
    AudioStarted(u64),
    AudioFinished {
        session: u64,
        result: Result<Vec<f32>, String>,
    },
    InferenceFinished {
        session: u64,
        generation: u64,
        job_id: u64,
        sample_range: Range<usize>,
        result: Result<String, String>,
    },
    DeliveryFinished {
        session: u64,
        result: DeliveryResult,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ControllerError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("Session resource state is inconsistent")]
    Resource,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionSnapshot {
    pub phase: Phase,
    pub session: Option<u64>,
    pub last_held_session: Option<u64>,
    pub text: Option<String>,
    pub outcome: Option<Outcome>,
}

struct Active {
    settings: SessionSettings,
    process: Option<ProcessIdentity>,
    delivery: bool,
    delivery_token: DeliveryToken,
    work_token: WorkToken,
    released: bool,
    audio: Option<Vec<f32>>,
    expected_range: Option<Range<usize>>,
}

#[derive(Default)]
pub struct Controller {
    core: Machine,
    active: Option<Active>,
}

impl Controller {
    pub(crate) fn audio_is_unreleased(&self, session: u64) -> bool {
        self.core.session_id() == Some(session)
            && self.active.as_ref().is_some_and(|active| !active.released)
    }
    pub(crate) fn cancellation_handles(&self) -> Option<(u64, DeliveryToken, WorkToken)> {
        let active = self.active.as_ref()?;
        Some((
            self.core.session_id()?,
            active.delivery_token.clone(),
            active.work_token.clone(),
        ))
    }

    pub fn fail(&mut self, session: u64, message: String) -> Result<Vec<Action>, ControllerError> {
        if self.core.session_id() != Some(session) {
            return Ok(vec![]);
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| !active.work_token.is_valid())
        {
            return self.reconcile_cancel();
        }
        let effects = self.core.accept(Fact::Failed {
            id: session,
            message,
        })?;
        self.actions(effects)
    }
    pub fn clear_text(&mut self) {
        self.core.clear_text();
    }
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            phase: self.core.phase(),
            session: self.core.session_id(),
            last_held_session: self.core.last_held_session(),
            text: self.core.last_text().map(str::to_owned),
            outcome: self.core.outcome().cloned(),
        }
    }

    pub fn start(
        &mut self,
        settings: SessionSettings,
        process: Option<ProcessIdentity>,
        ready: bool,
        delivery: bool,
    ) -> Result<Vec<Action>, ControllerError> {
        let effects = self
            .core
            .start(ready && settings.generation != 0, delivery)?;
        let session = NonZeroU64::new(self.core.session_id().ok_or(ControllerError::Resource)?)
            .ok_or(ControllerError::Resource)?;
        self.active = Some(Active {
            settings,
            process,
            delivery,
            delivery_token: DeliveryToken::new(session),
            work_token: WorkToken::default(),
            released: false,
            audio: None,
            expected_range: None,
        });
        self.actions(effects)
    }

    pub fn stop(&mut self, session: u64) -> Result<Vec<Action>, ControllerError> {
        let effects = self.core.stop(session)?;
        self.actions(effects)
    }

    pub fn cancel(&mut self, session: u64) -> Result<Vec<Action>, ControllerError> {
        if self.core.session_id() != Some(session) {
            return Ok(vec![]);
        }
        let active = self.active.as_ref().ok_or(ControllerError::Resource)?;
        let committed = active.delivery_token.cancel() == CancelOutcome::AlreadyCommitted;
        let effects = self.core.cancel(session, committed)?;
        self.actions(effects)
    }

    /// Fold an immediate client-side revocation into the serial state machine.
    pub(crate) fn reconcile_cancel(&mut self) -> Result<Vec<Action>, ControllerError> {
        if let Some((session, _, work)) = self.cancellation_handles()
            && !work.is_valid()
            && self.core.phase() != Phase::Cancelling
        {
            return match self.cancel(session) {
                // The native send won the atomic commit; its result still settles delivery.
                Err(ControllerError::Session(SessionError::DeliveryCommitted)) => Ok(vec![]),
                result => result,
            };
        }
        Ok(vec![])
    }

    pub fn accept(&mut self, event: Event) -> Result<Vec<Action>, ControllerError> {
        let session = match &event {
            Event::AudioStarted(id) => *id,
            Event::AudioFinished { session, .. }
            | Event::InferenceFinished { session, .. }
            | Event::DeliveryFinished { session, .. } => *session,
        };
        if self.core.session_id() != Some(session) {
            return Ok(vec![]);
        }
        let mut actions = self.reconcile_cancel()?;
        if self.core.session_id() != Some(session) {
            return Ok(actions);
        }
        let mut effects = Vec::new();
        match event {
            Event::AudioStarted(id) => effects.extend(self.core.accept(Fact::AudioStarted(id))?),
            Event::AudioFinished { result, .. } => {
                let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
                if active.released {
                    return Ok(actions);
                }
                active.released = true;
                match result {
                    Ok(audio)
                        if self.core.phase() != Phase::Cancelling
                            && self.core.phase() != Phase::Starting =>
                    {
                        active.audio = Some(audio);
                        effects.extend(self.core.stop(session)?);
                    }
                    Ok(_) if self.core.phase() == Phase::Starting => {
                        effects.extend(self.core.accept(Fact::Failed {
                            id: session,
                            message: "Audio ended before stream start was confirmed".into(),
                        })?)
                    }
                    Err(message) if self.core.phase() != Phase::Cancelling => {
                        effects.extend(self.core.accept(Fact::Failed {
                            id: session,
                            message,
                        })?)
                    }
                    _ => {}
                }
                effects.extend(self.core.accept(Fact::AudioReleased(session))?);
            }
            Event::InferenceFinished {
                generation,
                job_id,
                sample_range,
                result,
                ..
            } => {
                let active = self.active.as_ref().ok_or(ControllerError::Resource)?;
                if self.core.phase() != Phase::Finalizing
                    || !active.released
                    || generation != active.settings.generation
                    || job_id != session
                    || active.expected_range.as_ref() != Some(&sample_range)
                {
                    return Ok(actions);
                }
                effects.extend(self.core.accept(match result {
                    Ok(text) => Fact::Transcript { id: session, text },
                    Err(message) => Fact::Failed {
                        id: session,
                        message,
                    },
                })?);
            }
            Event::DeliveryFinished { result, .. } => {
                effects.extend(self.core.accept(Fact::Delivered {
                    id: session,
                    result,
                })?)
            }
        }
        actions.extend(self.actions(effects)?);
        Ok(actions)
    }

    fn actions(&mut self, effects: Vec<Effect>) -> Result<Vec<Action>, ControllerError> {
        let mut actions = Vec::new();
        for effect in effects {
            let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
            match effect {
                Effect::Capture(session) => {
                    if active.delivery {
                        actions.push(Action::CaptureTarget {
                            session: active.delivery_token.session(),
                            process: active.process,
                            token: active.delivery_token.clone(),
                        });
                    }
                    actions.push(Action::StartAudio {
                        session,
                        device: active.settings.device.clone(),
                    });
                }
                Effect::StopAudio(session) => actions.push(Action::StopAudio(session)),
                Effect::Transcribe(session) => {
                    let audio = active.audio.take().ok_or(ControllerError::Resource)?;
                    let sample_range = 0..audio.len();
                    active.expected_range = Some(sample_range.clone());
                    actions.push(Action::Transcribe(Job {
                        session,
                        generation: active.settings.generation,
                        job_id: session,
                        sample_range,
                        audio: Arc::new(audio),
                        language: active.settings.language,
                        token: active.work_token.clone(),
                    }));
                }
                Effect::Deliver(session) => actions.push(Action::Deliver {
                    session,
                    text: self
                        .core
                        .last_text()
                        .ok_or(ControllerError::Resource)?
                        .to_owned(),
                    token: active.delivery_token.clone(),
                }),
                Effect::Revoke(session) => {
                    active.work_token.cancel();
                    active.delivery_token.cancel();
                    active.audio = None;
                    actions.push(Action::CancelInference(session));
                    actions.push(Action::AbortDelivery(session));
                }
            }
        }
        if self.core.phase() == Phase::Idle {
            self.active = None;
        }
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SessionSettings {
        SessionSettings {
            device: Some("selected device".into()),
            language: Language::German,
            generation: 7,
        }
    }

    fn finish_audio(
        controller: &mut Controller,
        id: u64,
    ) -> Result<Job, Box<dyn std::error::Error>> {
        controller.accept(Event::AudioStarted(id))?;
        controller.stop(id)?;
        let actions = controller.accept(Event::AudioFinished {
            session: id,
            result: Ok(vec![0.2; 800]),
        })?;
        actions
            .into_iter()
            .find_map(|action| match action {
                Action::Transcribe(job) => Some(job),
                _ => None,
            })
            .ok_or_else(|| "no inference job".into())
    }

    #[test]
    fn cancelled_old_inference_cannot_change_a_new_recording()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        controller.start(settings(), None, true, false)?;
        let job = finish_audio(&mut controller, 1)?;
        let cancelled = controller.cancel(1)?;
        assert!(matches!(
            &cancelled[..],
            [Action::CancelInference(1), Action::AbortDelivery(1)]
        ));
        assert!(!job.token.is_valid());
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        controller.start(
            SessionSettings {
                generation: 8,
                ..settings()
            },
            None,
            true,
            false,
        )?;
        assert!(
            controller
                .accept(Event::InferenceFinished {
                    session: 1,
                    generation: 7,
                    job_id: 1,
                    sample_range: 0..800,
                    result: Ok("old text".into())
                })?
                .is_empty()
        );
        assert_eq!(controller.snapshot().session, Some(2));
        assert_eq!(controller.snapshot().phase, Phase::Starting);
        assert_eq!(controller.snapshot().text, None);
        Ok(())
    }

    #[test]
    fn delivery_keeps_its_start_token_and_committed_send_cannot_be_cancelled()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        let start = controller.start(settings(), None, true, true)?;
        let token = match &start[..] {
            [
                Action::CaptureTarget { token, .. },
                Action::StartAudio { .. },
            ] => token.clone(),
            _ => return Err("missing start target capture".into()),
        };
        finish_audio(&mut controller, 1)?;
        let actions = controller.accept(Event::InferenceFinished {
            session: 1,
            generation: 7,
            job_id: 1,
            sample_range: 0..800,
            result: Ok("new text".into()),
        })?;
        let delivered_token = match &actions[..] {
            [Action::Deliver { token, text, .. }] if text == "new text" => token.clone(),
            _ => return Err("missing delivery".into()),
        };
        token.try_commit()?;
        assert!(!delivered_token.is_pending());
        assert!(matches!(
            controller.cancel(1),
            Err(ControllerError::Session(SessionError::DeliveryCommitted))
        ));
        assert_eq!(controller.snapshot().phase, Phase::Delivering);
        controller.accept(Event::DeliveryFinished {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Sent));
        Ok(())
    }

    #[test]
    fn stop_during_start_and_failed_audio_release_do_not_strand_the_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        controller.start(settings(), None, true, false)?;
        assert!(controller.stop(1)?.is_empty());
        assert!(matches!(
            &controller.accept(Event::AudioStarted(1))?[..],
            [Action::StopAudio(1)]
        ));
        controller.accept(Event::AudioFinished {
            session: 1,
            result: Err("stream failed".into()),
        })?;
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert_eq!(
            controller.snapshot().outcome,
            Some(Outcome::Failed("stream failed".into()))
        );
        controller.start(settings(), None, true, false)?;
        controller.cancel(2)?;
        assert_eq!(controller.snapshot().phase, Phase::Cancelling);
        controller.accept(Event::AudioFinished {
            session: 2,
            result: Ok(vec![0.3; 80]),
        })?;
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Cancelled));
        Ok(())
    }

    #[test]
    fn stop_then_release_transfers_audio_once_with_frozen_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        let actions = controller.start(settings(), None, true, false)?;
        assert!(
            matches!(&actions[..], [Action::StartAudio { session: 1, device: Some(device) }] if device == "selected device")
        );
        controller.accept(Event::AudioStarted(1))?;
        assert!(matches!(&controller.stop(1)?[..], [Action::StopAudio(1)]));
        let actions = controller.accept(Event::AudioFinished {
            session: 1,
            result: Ok(vec![0.2; 800]),
        })?;
        match &actions[..] {
            [Action::Transcribe(job)] => {
                assert_eq!((job.session, job.generation, job.job_id), (1, 7, 1));
                assert_eq!(job.sample_range, 0..800);
                assert_eq!(job.audio.len(), 800);
                assert_eq!(job.language, Language::German);
            }
            _ => return Err("no single transcription request".into()),
        }
        assert!(
            controller
                .accept(Event::AudioFinished {
                    session: 1,
                    result: Ok(vec![0.3; 800])
                })?
                .is_empty()
        );
        controller.accept(Event::InferenceFinished {
            session: 1,
            generation: 7,
            job_id: 1,
            sample_range: 0..800,
            result: Ok("fixture words".into()),
        })?;
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert_eq!(controller.snapshot().text.as_deref(), Some("fixture words"));
        Ok(())
    }
    #[test]
    fn revoked_result_cannot_replace_retained_text() -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        controller.start(settings(), None, true, false)?;
        controller.accept(Event::AudioStarted(1))?;
        controller.stop(1)?;
        controller.accept(Event::AudioFinished {
            session: 1,
            result: Ok(vec![0.2; 800]),
        })?;
        let (_, _, work) = controller.cancellation_handles().ok_or("missing token")?;
        work.cancel();
        controller.accept(Event::InferenceFinished {
            session: 1,
            generation: 7,
            job_id: 1,
            sample_range: 0..800,
            result: Ok("must never be retained".into()),
        })?;
        assert_eq!(controller.snapshot().text, None);
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Cancelled));
        Ok(())
    }
    #[test]
    fn cancelled_submission_error_preserves_cancelled_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        controller.start(settings(), None, true, false)?;
        controller.accept(Event::AudioStarted(1))?;
        controller.stop(1)?;
        controller.accept(Event::AudioFinished {
            session: 1,
            result: Ok(vec![0.2; 800]),
        })?;
        let (_, _, work) = controller.cancellation_handles().ok_or("missing token")?;
        work.cancel();
        controller.fail(1, "Invalid inference job".into())?;
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Cancelled));
        Ok(())
    }
}
