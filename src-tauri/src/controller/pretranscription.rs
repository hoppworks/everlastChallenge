//! Bounded per-session ledger. Core phases remain authoritative.
use super::*;
use dictation_core::audio::MAX_SAMPLES;

#[derive(Default)]
pub(super) struct Run {
    pub next_start: usize,
    covered: usize,
    completed_job: u64,
    expected: Option<Identity>,
    pub(super) final_rest: Option<Vec<f32>>,
    pub(super) final_frames: Option<usize>,
    text: String,
}
struct Identity {
    job: u64,
    timeline: Range<usize>,
    storage: Range<usize>,
}
impl Controller {
    pub(crate) fn enable_pretranscription(&mut self) {
        self.pretranscription = true;
    }
    pub(crate) fn seal_available(&self) -> Option<u64> {
        if self.core.phase() != Phase::Recording {
            return None;
        }
        let active = self.active.as_ref()?;
        let run = active.run.as_ref()?;
        (active.work_token.is_valid() && run.expected.is_none()).then_some(run.completed_job)
    }
    pub(super) fn accept_segment(
        &mut self,
        session: u64,
        timeline: Range<usize>,
        samples: Arc<Vec<f32>>,
    ) -> Result<Vec<Action>, ControllerError> {
        if !matches!(self.core.phase(), Phase::Recording | Phase::Finalizing) {
            return Ok(vec![]);
        }
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let Some(run) = &mut active.run else {
            return Err(ControllerError::Resource);
        };
        if active.released
            || run.expected.is_some()
            || timeline.start != run.next_start
            || timeline.end <= timeline.start
            || timeline.end > MAX_SAMPLES
            || timeline.len() > samples.len()
        {
            return Err(ControllerError::Resource);
        }
        let job = self
            .next_job
            .checked_add(1)
            .ok_or(ControllerError::Resource)?;
        self.next_job = job;
        let storage = 0..timeline.len();
        run.next_start = timeline.end;
        run.expected = Some(Identity {
            job,
            timeline: timeline.clone(),
            storage: storage.clone(),
        });
        Ok(vec![Action::Transcribe(Job {
            session,
            generation: active.settings.generation,
            job_id: job,
            sample_range: storage,
            timeline_range: timeline,
            audio: samples,
            language: active.settings.language,
            token: active.work_token.clone(),
        })])
    }
    pub(super) fn segment_matches(
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
                    .run
                    .as_ref()
                    .and_then(|run| run.expected.as_ref())
                    .is_some_and(|expected| {
                        expected.job == job
                            && &expected.storage == storage
                            && &expected.timeline == timeline
                    })
        }) && matches!(self.core.phase(), Phase::Recording | Phase::Finalizing)
    }
    pub(super) fn segment_result(
        &mut self,
        result: Result<String, String>,
    ) -> Result<Vec<Action>, ControllerError> {
        let session = self.core.session_id().ok_or(ControllerError::Resource)?;
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let run = active.run.as_mut().ok_or(ControllerError::Resource)?;
        let expected = run.expected.take().ok_or(ControllerError::Resource)?;
        if expected.timeline.start != run.covered {
            return Err(ControllerError::Resource);
        }
        match result {
            Err(message) => return self.fail(session, message),
            Ok(text) => {
                let part = text.trim();
                let extra = part.len() + usize::from(!part.is_empty() && !run.text.is_empty());
                if run
                    .text
                    .len()
                    .checked_add(extra)
                    .is_none_or(|size| size > 256 * 1024)
                    || run.text.try_reserve_exact(extra).is_err()
                {
                    return self.fail(session, "Bounded segment text exceeded".into());
                }
                if !part.is_empty() {
                    if !run.text.is_empty() {
                        run.text.push(' ');
                    }
                    run.text.push_str(part);
                }
                run.covered = expected.timeline.end;
                run.completed_job = expected.job;
            }
        }
        self.segmented_jobs()
    }
    pub(super) fn segmented_jobs(&mut self) -> Result<Vec<Action>, ControllerError> {
        let session = self.core.session_id().ok_or(ControllerError::Resource)?;
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        let run = active.run.as_mut().ok_or(ControllerError::Resource)?;
        if !active.work_token.is_valid() || run.expected.is_some() || !active.released {
            return Ok(vec![]);
        }
        let final_frames = run.final_frames.ok_or(ControllerError::Resource)?;
        if run.next_start < final_frames {
            let audio = run.final_rest.take().ok_or(ControllerError::Resource)?;
            let timeline = run.next_start..final_frames;
            let storage = 0..audio.len();
            if timeline.len() != storage.len() {
                return Err(ControllerError::Resource);
            }
            let job = self
                .next_job
                .checked_add(1)
                .ok_or(ControllerError::Resource)?;
            self.next_job = job;
            run.next_start = final_frames;
            run.expected = Some(Identity {
                job,
                timeline: timeline.clone(),
                storage: storage.clone(),
            });
            return Ok(vec![Action::Transcribe(Job {
                session,
                generation: active.settings.generation,
                job_id: job,
                sample_range: storage,
                timeline_range: timeline,
                audio: Arc::new(audio),
                language: active.settings.language,
                token: active.work_token.clone(),
            })]);
        }
        if run.covered != final_frames {
            return Err(ControllerError::Resource);
        }
        drop(run.final_rest.take());
        let text = std::mem::take(&mut run.text);
        self.complete_transcription(session, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn settings() -> SessionSettings {
        SessionSettings {
            refinement: false,
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
    fn take_job(actions: Vec<Action>) -> Result<Job, Box<dyn std::error::Error>> {
        let mut jobs = actions.into_iter().filter_map(|action| match action {
            Action::Transcribe(job) => Some(job),
            _ => None,
        });
        let job = jobs.next().ok_or("missing single job")?;
        assert!(jobs.next().is_none());
        Ok(job)
    }
    fn completion(job: &Job, text: &str) -> Event {
        Event::InferenceFinished {
            session: job.session,
            generation: job.generation,
            job_id: job.job_id,
            timeline_range: job.timeline_range.clone(),
            sample_range: job.sample_range.clone(),
            result: Ok(text.into()),
        }
    }
    #[test]
    fn all_result_identities_and_coverage_must_match_before_one_final_transcript() -> TestResult {
        let mut controller = Controller::default();
        controller.enable_pretranscription();
        let sid = start(&mut controller)?;
        let job = take_job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 0..19200,
            samples: Arc::new(vec![0.1; 24000]),
        })?)?;
        assert_eq!(job.sample_range, 0..19200);
        assert_eq!(job.timeline_range, 0..19200);
        assert_eq!(
            job.audio.len(),
            24000,
            "physical suffix is not part of decoder input"
        );
        // Vary every tag independently; none may consume the actual expected job.
        for which in 0..5 {
            let mut wrong = completion(&job, "stale");
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
            assert!(!controller.inference_matches(&wrong));
            assert!(controller.accept(wrong)?.is_empty());
            assert_eq!(controller.snapshot().phase, Phase::Recording);
            assert_eq!(controller.snapshot().text, None);
        }
        assert!(controller.inference_matches(&completion(&job, "  one  ")));
        assert!(controller.accept(completion(&job, "  one  "))?.is_empty());
        assert!(controller.accept(completion(&job, "duplicate"))?.is_empty());
        controller.stop(sid)?;
        let last = take_job(controller.accept(Event::SegmentedFinished {
            session: sid,
            base: 19200,
            result: Ok(vec![0.2; 100]),
        })?)?;
        assert!(last.job_id > job.job_id);
        assert_eq!(last.timeline_range, 19200..19300);
        assert_eq!(last.sample_range, 0..100);
        assert!(controller.accept(completion(&job, "duplicate"))?.is_empty());
        controller.accept(completion(&last, " two "))?;
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert_eq!(controller.snapshot().text.as_deref(), Some("one two"));
        assert!(
            controller
                .accept(completion(&last, "duplicate"))?
                .is_empty()
        );
        let next = start(&mut controller)?;
        let newer = take_job(controller.accept(Event::Segment {
            session: next,
            timeline_range: 0..1,
            samples: Arc::new(vec![0.3]),
        })?)?;
        assert!(
            newer.job_id > last.job_id,
            "job IDs are monotone across session boundaries"
        );
        assert_ne!(
            newer.job_id, next,
            "job IDs are not inferred from session IDs"
        );
        Ok(())
    }

    #[test]
    fn refinement_waits_for_the_complete_segment_coverage() -> TestResult {
        let mut controller = Controller::default();
        controller.enable_pretranscription();
        let mut preferences = settings();
        preferences.refinement = true;
        controller.start(preferences, None, true, false)?;
        controller.accept(Event::AudioStarted(1))?;
        let first = take_job(controller.accept(Event::Segment {
            session: 1,
            timeline_range: 0..19200,
            samples: Arc::new(vec![0.1; 19200]),
        })?)?;
        assert!(controller.accept(completion(&first, "Das"))?.is_empty());
        assert!(controller.snapshot().refinement.original.is_none());
        controller.stop(1)?;
        let last = take_job(controller.accept(Event::SegmentedFinished {
            session: 1,
            base: 19200,
            result: Ok(vec![0.1; 100]),
        })?)?;
        let actions = controller.accept(completion(&last, "ist, äh, vollständig."))?;
        assert!(
            matches!(&actions[..], [Action::Refine(job)] if job.original.as_str() == "Das ist, äh, vollständig.")
        );
        assert!(
            controller
                .accept(completion(&last, "duplicate"))?
                .is_empty()
        );
        Ok(())
    }
    #[test]
    fn empty_parts_use_fixed_join_and_total_text_limit_fails_the_session() -> TestResult {
        let mut controller = Controller::default();
        controller.enable_pretranscription();
        let sid = start(&mut controller)?;
        let first = take_job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 0..1,
            samples: Arc::new(vec![0.1]),
        })?)?;
        controller.accept(completion(&first, " \n "))?;
        let second = take_job(controller.accept(Event::Segment {
            session: sid,
            timeline_range: 1..2,
            samples: Arc::new(vec![0.2]),
        })?)?;
        controller.accept(completion(&second, &"a".repeat(256 * 1024)))?;
        controller.stop(sid)?;
        let last = take_job(controller.accept(Event::SegmentedFinished {
            session: sid,
            base: 2,
            result: Ok(vec![0.3]),
        })?)?;
        let failed = controller.accept(completion(&last, "b"))?;
        assert!(
            failed
                .iter()
                .any(|action| matches!(action, Action::CancelInference(id) if *id == sid))
        );
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert!(matches!(
            controller.snapshot().outcome,
            Some(Outcome::Failed(_))
        ));
        assert_eq!(controller.snapshot().text, None);
        Ok(())
    }
}
