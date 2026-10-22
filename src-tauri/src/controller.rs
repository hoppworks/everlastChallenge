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

#[cfg(any(test, feature = "measurement"))]
mod full_prefix;
mod refinement;
pub use refinement::{RefinementSnapshot, RefinementStatus};
#[cfg(feature = "measurement")]
mod pretranscription;

#[derive(Clone)]
pub struct SessionSettings {
    pub device: Option<String>,
    pub language: Language,
    pub generation: u64,
    pub refinement: bool,
}

#[derive(Clone)]
pub(crate) struct SessionCancellation {
    pub session: u64,
    pub delivery: DeliveryToken,
    pub inference: WorkToken,
    pub refinement: crate::refinement_service::Cancellation,
}
impl SessionCancellation {
    pub fn revoke(&self) -> CancelOutcome {
        let outcome = self.delivery.cancel();
        self.inference.cancel();
        self.refinement.cancel();
        outcome
    }
    pub fn is_valid(&self) -> bool {
        self.inference.is_valid() && !self.refinement.is_cancelled()
    }
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
    Refine(crate::refinement_service::owner::Job),
    CancelInference(u64),
    Deliver {
        session: u64,
        text: String,
        token: DeliveryToken,
    },
    AbortDelivery(u64),
}

pub enum Event {
    Segment {
        session: u64,
        timeline_range: Range<usize>,
        samples: Arc<Vec<f32>>,
    },
    #[cfg(feature = "measurement")]
    SegmentedFinished {
        session: u64,
        base: usize,
        result: Result<Vec<f32>, String>,
    },
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
        timeline_range: Range<usize>,
        result: Result<String, String>,
    },
    DeliveryFinished {
        session: u64,
        result: DeliveryResult,
    },
    RefinementFinished {
        session: u64,
        original: dictation_core::refinement::OriginalTranscript,
        result: Result<
            dictation_core::refinement::AcceptedTranscript,
            crate::refinement_service::Error,
        >,
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
    pub refinement: RefinementSnapshot,
}

struct Active {
    settings: SessionSettings,
    process: Option<ProcessIdentity>,
    delivery: bool,
    delivery_token: DeliveryToken,
    work_token: WorkToken,
    refinement_cancel: crate::refinement_service::Cancellation,
    refining: bool,
    released: bool,
    audio: Option<Vec<f32>>,
    expected_range: Option<Range<usize>>,
    #[cfg(feature = "measurement")]
    run: Option<pretranscription::Run>,
    #[cfg(any(test, feature = "measurement"))]
    full_prefix: Option<full_prefix::Run>,
}

#[derive(Default)]
pub struct Controller {
    core: Machine,
    active: Option<Active>,
    refinement: RefinementSnapshot,
    #[cfg(feature = "measurement")]
    pretranscription: bool,
    #[cfg(any(test, feature = "measurement"))]
    full_prefix_revision: bool,
    #[cfg(any(test, feature = "measurement"))]
    next_job: u64,
}

impl Controller {
    pub(crate) fn with_policy(_policy: crate::pipeline::Policy) -> Self {
        Self {
            #[cfg(any(test, feature = "measurement"))]
            full_prefix_revision: _policy == crate::pipeline::Policy::FullPrefix,
            ..Self::default()
        }
    }

    pub(crate) fn audio_is_unreleased(&self, session: u64) -> bool {
        self.core.session_id() == Some(session)
            && self.active.as_ref().is_some_and(|active| !active.released)
    }
    pub(crate) fn cancellation_handles(&self) -> Option<SessionCancellation> {
        let active = self.active.as_ref()?;
        Some(SessionCancellation {
            session: self.core.session_id()?,
            delivery: active.delivery_token.clone(),
            inference: active.work_token.clone(),
            refinement: active.refinement_cancel.clone(),
        })
    }

    pub fn fail(&mut self, session: u64, message: String) -> Result<Vec<Action>, ControllerError> {
        if self.core.session_id() != Some(session) {
            return Ok(vec![]);
        }
        if self
            .cancellation_handles()
            .is_some_and(|cancel| !cancel.is_valid())
        {
            return self.reconcile_cancel();
        }
        if self.active.as_ref().is_some_and(|active| active.refining) {
            self.refinement.status = RefinementStatus::Failed(message.clone());
        }
        let effects = self.core.accept(Fact::Failed {
            id: session,
            message,
        })?;
        self.actions(effects)
    }
    pub fn clear_text(&mut self) -> Result<Vec<Action>, ControllerError> {
        let actions = if self.active.as_ref().is_some_and(|active| {
            active.refining
                || active.settings.refinement
                    && self.core.phase() == Phase::Delivering
                    && self.refinement.session == self.core.session_id()
        }) {
            match self.cancel(self.core.session_id().ok_or(ControllerError::Resource)?) {
                Ok(actions) => actions,
                // Clearing local history cannot retract a paste already sent.
                Err(ControllerError::Session(SessionError::DeliveryCommitted)) => vec![],
                Err(error) => return Err(error),
            }
        } else {
            vec![]
        };
        self.core.clear_text();
        self.refinement = RefinementSnapshot::default();
        Ok(actions)
    }
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            phase: self.core.phase(),
            session: self.core.session_id(),
            last_held_session: self.core.last_held_session(),
            text: self.core.last_text().map(str::to_owned),
            outcome: self.core.outcome().cloned(),
            refinement: self.refinement.clone(),
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
            refinement_cancel: crate::refinement_service::Cancellation::new(),
            refining: false,
            released: false,
            audio: None,
            expected_range: None,
            #[cfg(feature = "measurement")]
            run: self.pretranscription.then(pretranscription::Run::default),
            #[cfg(any(test, feature = "measurement"))]
            full_prefix: self.full_prefix_revision.then(full_prefix::Run::default),
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
        if active.refining {
            self.refinement.status = RefinementStatus::Cancelled;
        }
        let committed = active.delivery_token.cancel() == CancelOutcome::AlreadyCommitted;
        let effects = self.core.cancel(session, committed)?;
        self.actions(effects)
    }

    /// Fold an immediate client-side revocation into the serial state machine.
    pub(crate) fn reconcile_cancel(&mut self) -> Result<Vec<Action>, ControllerError> {
        if let Some(cancel) = self.cancellation_handles()
            && !cancel.is_valid()
            && self.core.phase() != Phase::Cancelling
        {
            return match self.cancel(cancel.session) {
                // The native send won the atomic commit; its result still settles delivery.
                Err(ControllerError::Session(SessionError::DeliveryCommitted)) => Ok(vec![]),
                result => result,
            };
        }
        Ok(vec![])
    }

    #[cfg(feature = "measurement")]
    pub(crate) fn inference_matches(&self, event: &Event) -> bool {
        let Event::InferenceFinished {
            session,
            generation,
            job_id,
            sample_range,
            timeline_range,
            ..
        } = event
        else {
            return false;
        };
        if self.core.session_id() != Some(*session) {
            return false;
        }
        let Some(active) = &self.active else {
            return false;
        };
        if !active.work_token.is_valid() {
            return false;
        }
        #[cfg(any(test, feature = "measurement"))]
        if active.full_prefix.is_some() {
            return self.prefix_matches(*generation, *job_id, sample_range, timeline_range);
        }
        #[cfg(feature = "measurement")]
        if active.run.is_some() {
            return self.segment_matches(*generation, *job_id, sample_range, timeline_range);
        }
        self.core.phase() == Phase::Finalizing
            && active.released
            && *generation == active.settings.generation
            && *job_id == *session
            && active.expected_range.as_ref() == Some(sample_range)
            && sample_range == timeline_range
    }

    pub fn accept(&mut self, event: Event) -> Result<Vec<Action>, ControllerError> {
        let session = match &event {
            Event::Segment { session, .. } => *session,
            #[cfg(feature = "measurement")]
            Event::SegmentedFinished { session, .. } => *session,
            Event::AudioStarted(id) => *id,
            Event::AudioFinished { session, .. }
            | Event::InferenceFinished { session, .. }
            | Event::DeliveryFinished { session, .. } => *session,
            Event::RefinementFinished { session, .. } => *session,
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
            Event::Segment {
                timeline_range: _timeline_range,
                samples: _samples,
                ..
            } => {
                #[cfg(any(test, feature = "measurement"))]
                if self.full_prefix_revision {
                    actions.extend(self.accept_prefix(session, _timeline_range, _samples)?);
                    return Ok(actions);
                }
                {
                    #[cfg(feature = "measurement")]
                    actions.extend(self.accept_segment(session, _timeline_range, _samples)?);
                    #[cfg(not(feature = "measurement"))]
                    return Err(ControllerError::Resource);
                }
                #[cfg(feature = "measurement")]
                return Ok(actions);
            }
            #[cfg(feature = "measurement")]
            Event::SegmentedFinished { base, result, .. } => {
                let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
                if active.released {
                    return Ok(actions);
                }
                active.released = true;
                match result {
                    Ok(audio)
                        if !matches!(self.core.phase(), Phase::Cancelling | Phase::Starting) =>
                    {
                        let run = active.run.as_mut().ok_or(ControllerError::Resource)?;
                        let final_frames = base
                            .checked_add(audio.len())
                            .ok_or(ControllerError::Resource)?;
                        if base != run.next_start
                            || final_frames > dictation_core::audio::MAX_SAMPLES
                        {
                            return Err(ControllerError::Resource);
                        }
                        run.final_frames = Some(final_frames);
                        run.final_rest = Some(audio);
                        effects.extend(self.core.stop(session)?);
                    }
                    Err(message) if self.core.phase() != Phase::Cancelling => {
                        effects.extend(self.core.accept(Fact::Failed {
                            id: session,
                            message,
                        })?);
                    }
                    Ok(_) if self.core.phase() == Phase::Starting => {
                        effects.extend(self.core.accept(Fact::Failed {
                            id: session,
                            message: "Audio ended before stream start was confirmed".into(),
                        })?);
                    }
                    _ => {}
                }
                effects.extend(self.core.accept(Fact::AudioReleased(session))?);
            }
            Event::AudioStarted(id) => effects.extend(self.core.accept(Fact::AudioStarted(id))?),
            Event::AudioFinished { result, .. } => {
                #[cfg(any(test, feature = "measurement"))]
                if self.full_prefix_revision {
                    return self.prefix_finished(session, result, actions);
                }
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
                timeline_range,
                result,
                ..
            } => {
                #[cfg(any(test, feature = "measurement"))]
                if self
                    .active
                    .as_ref()
                    .is_some_and(|a| a.full_prefix.is_some())
                {
                    if self.prefix_matches(generation, job_id, &sample_range, &timeline_range) {
                        actions.extend(self.prefix_result(result)?);
                    }
                    return Ok(actions);
                }
                #[cfg(feature = "measurement")]
                if self.active.as_ref().is_some_and(|a| a.run.is_some()) {
                    if self.segment_matches(generation, job_id, &sample_range, &timeline_range) {
                        actions.extend(self.segment_result(result)?);
                    }
                    return Ok(actions);
                }
                let active = self.active.as_ref().ok_or(ControllerError::Resource)?;
                if self.core.phase() != Phase::Finalizing
                    || active.refining
                    || !active.released
                    || generation != active.settings.generation
                    || job_id != session
                    || active.expected_range.as_ref() != Some(&sample_range)
                    || sample_range != timeline_range
                {
                    return Ok(actions);
                }
                return match result {
                    Ok(text) => {
                        actions.extend(self.complete_transcription(session, text)?);
                        Ok(actions)
                    }
                    Err(message) => {
                        effects.extend(self.core.accept(Fact::Failed {
                            id: session,
                            message,
                        })?);
                        actions.extend(self.actions(effects)?);
                        Ok(actions)
                    }
                };
            }
            Event::RefinementFinished {
                original, result, ..
            } => {
                actions.extend(self.complete_refinement(session, original, result)?);
                return Ok(actions);
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
                    #[cfg(any(test, feature = "measurement"))]
                    if active.full_prefix.is_some() {
                        actions.extend(self.prefix_final_job()?);
                        continue;
                    }
                    #[cfg(feature = "measurement")]
                    if active.run.is_some() {
                        actions.extend(self.segmented_jobs()?);
                        continue;
                    }
                    let audio = active.audio.take().ok_or(ControllerError::Resource)?;
                    let sample_range = 0..audio.len();
                    active.expected_range = Some(sample_range.clone());
                    actions.push(Action::Transcribe(Job {
                        session,
                        generation: active.settings.generation,
                        job_id: session,
                        timeline_range: sample_range.clone(),
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
                    active.refinement_cancel.cancel();
                    active.refining = false;
                    active.delivery_token.cancel();
                    active.audio = None;
                    #[cfg(feature = "measurement")]
                    {
                        active.run = None;
                    }
                    #[cfg(any(test, feature = "measurement"))]
                    {
                        active.full_prefix = None;
                    }
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

    pub(super) fn settings() -> SessionSettings {
        SessionSettings {
            refinement: false,
            device: Some("selected device".into()),
            language: Language::German,
            generation: 7,
        }
    }

    pub(super) fn finish_audio(
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
                    timeline_range: 0..800,
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
            timeline_range: 0..800,
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
            timeline_range: 0..800,
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
        let work = controller
            .cancellation_handles()
            .ok_or("missing token")?
            .inference;
        work.cancel();
        controller.accept(Event::InferenceFinished {
            session: 1,
            generation: 7,
            job_id: 1,
            sample_range: 0..800,
            timeline_range: 0..800,
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
        let work = controller
            .cancellation_handles()
            .ok_or("missing token")?
            .inference;
        work.cancel();
        controller.fail(1, "Invalid inference job".into())?;
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Cancelled));
        Ok(())
    }
}
