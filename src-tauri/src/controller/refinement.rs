use super::*;
use dictation_core::refinement::{OriginalTranscript, ProcessingDecision, SkipReason, decide};

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RefinementSnapshot {
    pub session: Option<u64>,
    pub original: Option<String>,
    pub status: RefinementStatus,
}

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize)]
pub enum RefinementStatus {
    #[default]
    Disabled,
    Processing,
    Skipped(SkipReason),
    CheckedUnchanged,
    Edited,
    Failed(String),
    HeldOriginal(String),
    Cancelled,
}
impl Controller {
    pub(super) fn complete_transcription(
        &mut self,
        session: u64,
        text: String,
    ) -> Result<Vec<Action>, ControllerError> {
        let active = self.active.as_mut().ok_or(ControllerError::Resource)?;
        if active.refining {
            return Ok(vec![]);
        }
        if text.is_empty() {
            let effects = self.core.accept(Fact::Transcript { id: session, text })?;
            return self.actions(effects);
        }
        self.refinement = RefinementSnapshot {
            session: Some(session),
            original: None,
            status: RefinementStatus::Disabled,
        };
        if active.settings.refinement {
            self.core.accept(Fact::RetainTranscript {
                id: session,
                text: text.clone(),
            })?;
            self.refinement.original = Some(text.clone());
            let original = match OriginalTranscript::new(text) {
                Ok(original) => original,
                Err(error) => return self.refinement_failed(session, error.to_string()),
            };
            let language = match active.settings.language {
                Language::German => text_refinement_protocol::Language::German,
                Language::English => text_refinement_protocol::Language::English,
            };
            match decide(&original, language.into()) {
                ProcessingDecision::Refine(_) => {
                    active.refining = true;
                    self.refinement.status = RefinementStatus::Processing;
                    return Ok(vec![Action::Refine(
                        crate::refinement_service::owner::Job {
                            session: NonZeroU64::new(session).ok_or(ControllerError::Resource)?,
                            original,
                            language,
                            cancel: active.refinement_cancel.clone(),
                        },
                    )]);
                }
                ProcessingDecision::Skip(reason) => {
                    self.refinement.status = RefinementStatus::Skipped(reason);
                    let effects = self.core.accept(Fact::Transcript {
                        id: session,
                        text: original.as_str().to_owned(),
                    })?;
                    return self.actions(effects);
                }
            }
        }
        let effects = self.core.accept(Fact::Transcript { id: session, text })?;
        self.actions(effects)
    }
    pub(super) fn refinement_failed(
        &mut self,
        session: u64,
        message: String,
    ) -> Result<Vec<Action>, ControllerError> {
        self.refinement.status = RefinementStatus::Failed(message.clone());
        let effects = self.core.accept(Fact::Failed {
            id: session,
            message,
        })?;
        self.actions(effects)
    }
    pub(super) fn complete_refinement(
        &mut self,
        session: u64,
        original: OriginalTranscript,
        result: Result<
            dictation_core::refinement::AcceptedTranscript,
            crate::refinement_service::Error,
        >,
    ) -> Result<Vec<Action>, ControllerError> {
        let active = self.active.as_ref().ok_or(ControllerError::Resource)?;
        if !active.refining || self.core.phase() != Phase::Finalizing {
            return Ok(vec![]);
        }
        if active.refinement_cancel.is_cancelled() {
            return self.cancel(session);
        }
        if self.refinement.session != Some(session)
            || self.refinement.original.as_deref() != Some(original.as_str())
        {
            return self.refinement_failed(
                session,
                "Text refinement source does not match the active transcript".into(),
            );
        }
        match result {
            Ok(accepted) => {
                let language = match active.settings.language {
                    Language::German => dictation_core::refinement::Language::German,
                    Language::English => dictation_core::refinement::Language::English,
                };
                if accepted.original() != &original || accepted.language() != language {
                    return self.refinement_failed(
                        session,
                        "Validated text belongs to a different original or language".into(),
                    );
                }
                self.active
                    .as_mut()
                    .ok_or(ControllerError::Resource)?
                    .refining = false;
                self.refinement.status = if accepted.edit_count() == 0 {
                    RefinementStatus::CheckedUnchanged
                } else {
                    RefinementStatus::Edited
                };
                let effects = self.core.accept(Fact::Transcript {
                    id: session,
                    text: accepted.as_str().to_owned(),
                })?;
                self.actions(effects)
            }
            Err(error) => {
                let rejected = matches!(
                    error.reason,
                    crate::refinement_service::Failure::InvalidCandidate
                        | crate::refinement_service::Failure::Rejected(
                            text_refinement_protocol::FailureKind::InvalidCandidate
                        )
                );
                let message = error.to_string();
                let actions = self.refinement_failed(session, message.clone())?;
                if rejected {
                    self.refinement.status = RefinementStatus::HeldOriginal(message);
                }
                Ok(actions)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn pending()
    -> Result<(Controller, crate::refinement_service::owner::Job), Box<dyn std::error::Error>> {
        pending_text("Das ist, äh, ein Test.")
    }
    fn pending_text(
        text: &str,
    ) -> Result<(Controller, crate::refinement_service::owner::Job), Box<dyn std::error::Error>>
    {
        let mut controller = Controller::default();
        let mut settings = crate::controller::tests::settings();
        settings.refinement = true;
        controller.start(settings, None, true, true)?;
        let job = crate::controller::tests::finish_audio(&mut controller, 1)?;
        let mut actions = controller.accept(Event::InferenceFinished {
            session: job.session,
            generation: job.generation,
            job_id: job.job_id,
            sample_range: job.sample_range,
            timeline_range: job.timeline_range,
            result: Ok(text.into()),
        })?;
        match actions.pop() {
            Some(Action::Refine(job)) if actions.is_empty() => Ok((controller, job)),
            _ => Err("missing refinement job".into()),
        }
    }

    #[test]
    fn accepted_refinement_delivers_once_and_keeps_the_original() -> TestResult {
        let (mut controller, job) = pending()?;
        let edits = [dictation_core::refinement::ProposedEdit {
            kind: dictation_core::refinement::EditKind::Filler,
            before: ", äh,".into(),
            after: "".into(),
            left_context: None,
            right_context: None,
        }];
        let accepted =
            dictation_core::refinement::validate(&job.original, &edits, job.language.into())?;
        let result = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original.clone(),
            result: Ok(accepted.clone()),
        })?;
        assert!(
            matches!(&result[..], [Action::Deliver { text, .. }] if text == "Das ist ein Test.")
        );
        assert_eq!(
            controller.snapshot().refinement.original.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Edited
        ));
        assert!(
            controller
                .accept(Event::RefinementFinished {
                    session: 1,
                    original: job.original,
                    result: Ok(accepted)
                })?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn cancellation_keeps_complete_original_and_rejects_late_results() -> TestResult {
        let (mut controller, job) = pending()?;
        controller.cancel(1)?;
        assert!(job.cancel.is_cancelled());
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.phase, Phase::Idle);
        assert_eq!(snapshot.text.as_deref(), Some("Das ist, äh, ein Test."));
        assert!(matches!(
            snapshot.refinement.status,
            RefinementStatus::Cancelled
        ));
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        controller.start(crate::controller::tests::settings(), None, true, false)?;
        assert!(
            controller
                .accept(Event::RefinementFinished {
                    session: 1,
                    original: job.original,
                    result: Ok(accepted)
                })?
                .is_empty()
        );
        assert_eq!(controller.snapshot().session, Some(2));
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        Ok(())
    }

    #[test]
    fn clearing_during_refinement_revokes_work_and_cannot_restore_the_text() -> TestResult {
        let (mut controller, job) = pending()?;
        controller.clear_text()?;
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert!(controller.snapshot().text.is_none());
        assert!(controller.snapshot().refinement.original.is_none());
        assert!(job.cancel.is_cancelled());
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        assert!(
            controller
                .accept(Event::RefinementFinished {
                    session: 1,
                    original: job.original,
                    result: Ok(accepted)
                })?
                .is_empty()
        );
        assert!(controller.snapshot().text.is_none());
        Ok(())
    }

    #[test]
    fn clear_after_refinement_revokes_delivery_before_its_commit() -> TestResult {
        let (mut controller, job) = pending()?;
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        let actions = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Ok(accepted),
        })?;
        let token = actions
            .into_iter()
            .find_map(|action| match action {
                Action::Deliver { token, .. } => Some(token),
                _ => None,
            })
            .ok_or("missing delivery")?;
        controller.clear_text()?;
        assert!(!token.is_pending());
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert!(controller.snapshot().text.is_none());
        assert!(controller.snapshot().refinement.original.is_none());
        Ok(())
    }

    #[test]
    fn clearing_history_after_delivery_commit_does_not_retract_the_sent_event() -> TestResult {
        let (mut controller, job) = pending()?;
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        let actions = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Ok(accepted),
        })?;
        let token = actions
            .into_iter()
            .find_map(|action| match action {
                Action::Deliver { token, .. } => Some(token),
                _ => None,
            })
            .ok_or("missing delivery")?;
        token.try_commit()?;
        assert!(controller.clear_text()?.is_empty());
        controller.accept(Event::DeliveryFinished {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Sent));
        assert!(controller.snapshot().text.is_none());
        assert!(controller.snapshot().refinement.original.is_none());
        Ok(())
    }

    #[test]
    fn accepted_text_for_another_original_cannot_authorize_delivery() -> TestResult {
        let (mut controller, job) = pending()?;
        let foreign = OriginalTranscript::new("Andere Worte.".into())?;
        let accepted = dictation_core::refinement::validate(&foreign, &[], job.language.into())?;
        let actions = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Ok(accepted),
        })?;
        assert!(!actions.iter().any(|a| matches!(a, Action::Deliver { .. })));
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Failed(_)
        ));
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        Ok(())
    }

    #[test]
    fn validation_in_another_language_cannot_delete_a_meaningful_word() -> TestResult {
        let (mut controller, job) = pending_text("Wir kommen, äh, um zehn.")?;
        let accepted = dictation_core::refinement::validate(
            &job.original,
            &[dictation_core::refinement::ProposedEdit {
                kind: dictation_core::refinement::EditKind::Filler,
                before: "um".into(),
                after: "".into(),
                left_context: None,
                right_context: None,
            }],
            dictation_core::refinement::Language::English,
        )?;
        let actions = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Ok(accepted),
        })?;
        assert!(!actions.iter().any(|a| matches!(a, Action::Deliver { .. })));
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("Wir kommen, äh, um zehn.")
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Failed(_)
        ));
        Ok(())
    }

    #[test]
    fn rejected_refinement_holds_the_original_and_records_the_reason() -> TestResult {
        let (mut controller, job) = pending()?;
        let actions = controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Err(crate::refinement_service::Failure::InvalidCandidate.into()),
        })?;
        assert!(!actions.iter().any(|a| matches!(a, Action::Deliver { .. })));
        assert!(job.cancel.is_cancelled());
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::HeldOriginal(_)
        ));
        Ok(())
    }

    #[test]
    fn duplicate_asr_and_foreign_results_do_not_replace_the_pending_original() -> TestResult {
        let (mut controller, job) = pending()?;
        for result in [Ok("replacement".into()), Err("late ASR failure".into())] {
            assert!(
                controller
                    .accept(Event::InferenceFinished {
                        session: 1,
                        generation: 7,
                        job_id: 1,
                        sample_range: 0..800,
                        timeline_range: 0..800,
                        result,
                    })?
                    .is_empty()
            );
        }
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        assert!(
            controller
                .accept(Event::RefinementFinished {
                    session: 999,
                    original: job.original.clone(),
                    result: Ok(accepted)
                })?
                .is_empty()
        );
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Processing
        ));
        job.cancel.cancel();
        controller.fail(1, "late worker failure".into())?;
        assert_eq!(controller.snapshot().outcome, Some(Outcome::Cancelled));
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Cancelled
        ));
        Ok(())
    }

    #[test]
    fn oversized_original_is_retained_without_sending_a_truncated_job() -> TestResult {
        let mut controller = Controller::default();
        let mut settings = crate::controller::tests::settings();
        settings.refinement = true;
        controller.start(settings, None, true, true)?;
        let job = crate::controller::tests::finish_audio(&mut controller, 1)?;
        let original = "x".repeat(dictation_core::refinement::MAX_ORIGINAL_BYTES + 1);
        let actions = controller.accept(Event::InferenceFinished {
            session: 1,
            generation: job.generation,
            job_id: job.job_id,
            sample_range: job.sample_range,
            timeline_range: job.timeline_range,
            result: Ok(original.clone()),
        })?;
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, Action::Refine(_) | Action::Deliver { .. }))
        );
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some(original.as_str())
        );
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Failed(_)
        ));
        assert_eq!(controller.snapshot().phase, Phase::Idle);
        Ok(())
    }

    #[test]
    fn skipped_and_checked_unchanged_results_remain_distinguishable() -> TestResult {
        let (mut controller, job) = pending()?;
        let accepted =
            dictation_core::refinement::validate(&job.original, &[], job.language.into())?;
        controller.accept(Event::RefinementFinished {
            session: 1,
            original: job.original,
            result: Ok(accepted),
        })?;
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::CheckedUnchanged
        ));
        controller.accept(Event::DeliveryFinished {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        let mut settings = crate::controller::tests::settings();
        settings.refinement = true;
        controller.start(settings, None, true, false)?;
        let job = crate::controller::tests::finish_audio(&mut controller, 2)?;
        let actions = controller.accept(Event::InferenceFinished {
            session: 2,
            generation: job.generation,
            job_id: job.job_id,
            sample_range: job.sample_range,
            timeline_range: job.timeline_range,
            result: Ok("https://example.org/a".into()),
        })?;
        assert!(!actions.iter().any(|a| matches!(a, Action::Refine(_))));
        assert!(matches!(
            controller.snapshot().refinement.status,
            RefinementStatus::Skipped(SkipReason::ExactValue)
        ));
        assert_eq!(
            controller.snapshot().text.as_deref(),
            Some("https://example.org/a")
        );
        Ok(())
    }

    #[test]
    fn full_transcript_waits_for_refinement_without_delivery() -> TestResult {
        let mut controller = Controller::default();
        let mut settings = crate::controller::tests::settings();
        settings.refinement = true;
        controller.start(settings, None, true, true)?;
        let job = crate::controller::tests::finish_audio(&mut controller, 1)?;
        let actions = controller.accept(Event::InferenceFinished {
            session: job.session,
            generation: job.generation,
            job_id: job.job_id,
            sample_range: job.sample_range,
            timeline_range: job.timeline_range,
            result: Ok("Das ist, äh, ein Test.".into()),
        })?;
        assert!(matches!(&actions[..], [Action::Refine(_)]));
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.phase, Phase::Finalizing);
        assert_eq!(snapshot.text.as_deref(), Some("Das ist, äh, ein Test."));
        assert!(matches!(
            snapshot.refinement.status,
            RefinementStatus::Processing
        ));
        assert_eq!(
            snapshot.refinement.original.as_deref(),
            Some("Das ist, äh, ein Test.")
        );
        Ok(())
    }
}
