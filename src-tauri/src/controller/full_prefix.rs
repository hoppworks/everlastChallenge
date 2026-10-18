//! Cumulative revisions. Preview text is never a Core transcript or delivery.
use super::*;
use dictation_core::audio::MAX_SAMPLES;
#[derive(Default)]
pub(super) struct Run {
    expected: Option<Identity>,
    completed_job: u64,
    last_cut: usize,
    final_audio: Option<Vec<f32>>,
    revision: String,
}
struct Identity {
    job: u64,
    frames: usize,
    final_job: bool,
}
impl Controller {
    pub(crate) fn prefix_available(&self) -> Option<u64> {
        let active = self.active.as_ref()?;
        let run = active.full_prefix.as_ref()?;
        (self.core.phase() == Phase::Recording
            && !active.released
            && active.work_token.is_valid()
            && run.expected.is_none())
        .then_some(run.completed_job)
    }
    pub(super) fn accept_prefix(
        &mut self,
        session: u64,
        timeline: Range<usize>,
        audio: Arc<Vec<f32>>,
    ) -> Result<Vec<Action>, ControllerError> {
        if !matches!(self.core.phase(), Phase::Recording | Phase::Finalizing) {
            return Ok(vec![]);
        }
        let active = self.active.as_ref().ok_or(ControllerError::Resource)?;
        let run = active
            .full_prefix
            .as_ref()
            .ok_or(ControllerError::Resource)?;
        if !active.work_token.is_valid()
            || active.released
            || run.expected.is_some()
            || timeline.start != 0
            || timeline.end <= run.last_cut
            || timeline.end > MAX_SAMPLES
            || timeline.end != audio.len()
        {
            return Err(ControllerError::Resource);
        }
        self.prefix_job(session, audio, false)
    }
    fn prefix_job(
        &mut self,
        session: u64,
        audio: Arc<Vec<f32>>,
        final_job: bool,
    ) -> Result<Vec<Action>, ControllerError> {
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let run = active
            .full_prefix
            .as_mut()
            .ok_or(ControllerError::Resource)?;
        let job = self
            .next_job
            .checked_add(1)
            .ok_or(ControllerError::Resource)?;
        self.next_job = job;
        if !final_job {
            run.last_cut = audio.len();
        }
        run.expected = Some(Identity {
            job,
            frames: audio.len(),
            final_job,
        });
        Ok(vec![Action::Transcribe(Job {
            session,
            generation: active.settings.generation,
            job_id: job,
            sample_range: 0..audio.len(),
            timeline_range: 0..audio.len(),
            audio,
            language: active.settings.language,
            token: active.work_token.clone(),
        })])
    }
    pub(super) fn prefix_matches(
        &self,
        generation: u64,
        job: u64,
        storage: &Range<usize>,
        timeline: &Range<usize>,
    ) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.work_token.is_valid()
                && generation == active.settings.generation
                && active
                    .full_prefix
                    .as_ref()
                    .and_then(|run| run.expected.as_ref())
                    .is_some_and(|expected| {
                        expected.job == job
                            && storage == &(0..expected.frames)
                            && timeline == storage
                            && (!expected.final_job || active.released)
                    })
        }) && matches!(self.core.phase(), Phase::Recording | Phase::Finalizing)
    }
    pub(super) fn prefix_result(
        &mut self,
        result: Result<String, String>,
    ) -> Result<Vec<Action>, ControllerError> {
        let session = self.core.session_id().ok_or(ControllerError::Resource)?;
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let run = active
            .full_prefix
            .as_mut()
            .ok_or(ControllerError::Resource)?;
        let expected = run.expected.take().ok_or(ControllerError::Resource)?;
        let text = match result {
            Ok(text) if text.len() <= 256 * 1024 => text,
            Ok(_) => return self.fail(session, "Bounded prefix revision text exceeded".into()),
            Err(error) => return self.fail(session, error),
        };
        run.completed_job = expected.job;
        // Replace, never append/deduplicate. Keep at most one bounded revision.
        run.revision = text;
        if expected.final_job {
            let text = std::mem::take(&mut run.revision);
            let effects = self.core.accept(Fact::Transcript { id: session, text })?;
            self.actions(effects)
        } else {
            self.prefix_final_job()
        }
    }
    pub(super) fn prefix_final_job(&mut self) -> Result<Vec<Action>, ControllerError> {
        let session = self.core.session_id().ok_or(ControllerError::Resource)?;
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let run = active
            .full_prefix
            .as_mut()
            .ok_or(ControllerError::Resource)?;
        if !active.released || !active.work_token.is_valid() || run.expected.is_some() {
            return Ok(vec![]);
        }
        let audio = run.final_audio.take().ok_or(ControllerError::Resource)?;
        // Even an unchanged/empty tail requires this final full-recording call.
        self.prefix_job(session, Arc::new(audio), true)
    }
    pub(super) fn prefix_finished(
        &mut self,
        session: u64,
        result: Result<Vec<f32>, String>,
        mut actions: Vec<Action>,
    ) -> Result<Vec<Action>, ControllerError> {
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        if active.released {
            return Ok(actions);
        }
        active.released = true;
        let mut effects = Vec::new();
        match result {
            Ok(audio) if !matches!(self.core.phase(), Phase::Cancelling | Phase::Starting) => {
                let run = active
                    .full_prefix
                    .as_mut()
                    .ok_or(ControllerError::Resource)?;
                if audio.len() > MAX_SAMPLES || audio.len() < run.last_cut {
                    return Err(ControllerError::Resource);
                }
                run.final_audio = Some(audio);
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
        actions.extend(self.actions(effects)?);
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn settings() -> SessionSettings {
        SessionSettings {
            device: None,
            language: Language::German,
            generation: 7,
        }
    }
    fn start(controller: &mut Controller) -> Result<u64, Box<dyn std::error::Error>> {
        controller.start(settings(), None, true, false)?;
        let sid = controller.snapshot().session.ok_or("missing SID")?;
        controller.accept(Event::AudioStarted(sid))?;
        Ok(sid)
    }
    fn job(actions: Vec<Action>) -> Result<Job, Box<dyn std::error::Error>> {
        let mut jobs = actions.into_iter().filter_map(|a| {
            if let Action::Transcribe(job) = a {
                Some(job)
            } else {
                None
            }
        });
        let job = jobs.next().ok_or("missing job")?;
        assert!(jobs.next().is_none());
        Ok(job)
    }
    fn done(job: &Job, text: &str) -> Event {
        Event::InferenceFinished {
            session: job.session,
            generation: job.generation,
            job_id: job.job_id,
            sample_range: job.sample_range.clone(),
            timeline_range: job.timeline_range.clone(),
            result: Ok(text.into()),
        }
    }
    #[test]
    fn every_identity_guards_revisions_and_final_always_redecodes_the_full_recording() -> TestResult
    {
        let mut controller = Controller::with_policy(crate::pipeline::Policy::FullPrefix);
        let sid = start(&mut controller)?;
        let preview = job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 0..19200,
            samples: Arc::new(vec![0.1; 19200]),
        })?)?;
        for which in 0..5 {
            let mut wrong = done(&preview, "stale");
            if let Event::InferenceFinished {
                session,
                generation,
                job_id,
                sample_range,
                timeline_range,
                ..
            } = &mut wrong
            {
                match which {
                    0 => *session += 1,
                    1 => *generation += 1,
                    2 => *job_id += 1,
                    3 => *sample_range = 1..19201,
                    _ => *timeline_range = 1..19201,
                }
            }
            #[cfg(feature = "measurement")]
            assert!(!controller.inference_matches(&wrong));
            assert!(controller.accept(wrong)?.is_empty());
        }
        assert!(
            controller
                .accept(done(&preview, "old revision"))?
                .is_empty()
        );
        assert!(controller.accept(done(&preview, "duplicate"))?.is_empty());
        assert_eq!(controller.snapshot().phase, Phase::Recording);
        assert_eq!(controller.snapshot().text, None);
        controller.stop(sid)?;
        let final_job = job(controller.accept(Event::AudioFinished {
            session: sid,
            result: Ok(vec![0.1; 24000]),
        })?)?;
        assert_eq!(final_job.timeline_range, 0..24000);
        assert_eq!(final_job.sample_range, 0..24000);
        assert!(final_job.job_id > preview.job_id);
        assert!(controller.accept(done(&preview, "late"))?.is_empty());
        assert_eq!(controller.snapshot().phase, Phase::Finalizing);
        controller.accept(done(&final_job, "entire revised text"))?;
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("entire revised text")
        );
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Local));
        assert!(
            controller
                .accept(done(&final_job, "duplicate final"))?
                .is_empty()
        );
        let next = start(&mut controller)?;
        assert!(next > sid);
        assert!(
            controller
                .accept(done(&final_job, "old session"))?
                .is_empty()
        );
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("entire revised text"),
            "ordinary Controller retains previous text; stale completion cannot replace it"
        );
        assert_eq!(controller.snapshot().phase, Phase::Recording);
        Ok(())
    }
    #[test]
    fn final_call_is_mandatory_even_if_its_frames_equal_the_last_prefix() -> TestResult {
        let mut controller = Controller::with_policy(crate::pipeline::Policy::FullPrefix);
        let sid = start(&mut controller)?;
        let preview = job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 0..100,
            samples: Arc::new(vec![0.1; 100]),
        })?)?;
        controller.accept(done(&preview, "preview"))?;
        let final_job = job(controller.accept(Event::AudioFinished {
            session: sid,
            result: Ok(vec![0.1; 100]),
        })?)?;
        assert_eq!(final_job.timeline_range, preview.timeline_range);
        assert!(final_job.job_id > preview.job_id);
        controller.accept(done(&final_job, ""))?;
        assert_eq!(controller.snapshot().text, None);
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Empty));
        Ok(())
    }
    #[test]
    fn oversized_revision_fails_without_a_partial_transcript_or_next_job() -> TestResult {
        let mut controller = Controller::with_policy(crate::pipeline::Policy::FullPrefix);
        let sid = start(&mut controller)?;
        let preview = job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 0..1,
            samples: Arc::new(vec![0.1]),
        })?)?;
        let actions = controller.accept(done(&preview, &"x".repeat(256 * 1024 + 1)))?;
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, Action::CancelInference(id) if *id == sid))
        );
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, Action::Transcribe(_) | Action::Deliver { .. }))
        );
        assert_eq!(controller.snapshot().text, None);
        assert_eq!(controller.snapshot().phase, Phase::Cancelling);
        controller.accept(Event::AudioFinished {
            session: sid,
            result: Ok(vec![0.1]),
        })?;
        assert!(matches!(
            controller.snapshot().outcome,
            Some(Outcome::Failed(_))
        ));
        assert_eq!(controller.snapshot().text, None);
        Ok(())
    }
}
