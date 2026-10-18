//! Independent overlapping-prefix proof. Worker wall spans are not decode RTF.
use everlast_dictation::{
    audio::replay::clock::FeedEvidence, runtime::measurement::job_trace::JobTraceSnapshot,
    transcription::Language,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Reference,
    #[allow(
        dead_code,
        reason = "shared validator is also compiled by probes without candidate mode"
    )]
    Candidate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputProof {
    pub job_id: u64,
    pub required_stable_frames: Option<usize>,
    pub published_through_frames_at_admission: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub trace: JobTraceSnapshot,
    pub input_proofs: Vec<InputProof>,
    pub processed_frames_sum: Option<usize>,
    pub checked_worker_span_sum_nanos: Option<u128>,
    pub final_adoption_offset_nanos: Option<u128>,
    pub error: Option<String>,
}

pub fn audit(
    trace: JobTraceSnapshot,
    feed: Option<&FeedEvidence>,
    session: Option<u64>,
    generation: u64,
    frames: usize,
    language: Language,
    variant: Variant,
) -> Evidence {
    let mut input_proofs = Vec::new();
    let result = validate(
        &trace,
        feed,
        session,
        generation,
        frames,
        language,
        variant,
        &mut input_proofs,
    );
    Evidence {
        trace,
        input_proofs,
        processed_frames_sum: result.as_ref().ok().map(|proof| proof.processed_frames),
        checked_worker_span_sum_nanos: result.as_ref().ok().map(|proof| proof.worker_spans),
        final_adoption_offset_nanos: result.as_ref().ok().map(|proof| proof.final_adoption),
        error: result.err().map(str::to_owned),
    }
}

struct ValidatedTiming {
    processed_frames: usize,
    worker_spans: u128,
    final_adoption: u128,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one explicit frozen run identity and its retained proof output"
)]
fn validate(
    trace: &JobTraceSnapshot,
    feed: Option<&FeedEvidence>,
    session: Option<u64>,
    generation: u64,
    frames: usize,
    language: Language,
    variant: Variant,
    proofs: &mut Vec<InputProof>,
) -> Result<ValidatedTiming, &'static str> {
    use everlast_dictation::runtime::measurement::job_trace::MAX_JOBS;
    if !trace.enabled
        || session.is_none_or(|id| id == 0)
        || trace.session != session
        || generation == 0
    {
        return Err("missing/current-session job trace unavailable");
    }
    if trace.error.is_some()
        || trace.maximum_jobs != MAX_JOBS
        || trace.jobs.is_empty()
        || trace.jobs.len() > MAX_JOBS
        || trace.overflow_events != 0
        || trace.first_overflow_job.is_some()
    {
        return Err("missing or failed bounded job trace");
    }
    if trace
        .retired_job
        .as_ref()
        .is_some_and(|job| job.worker_settled_offset_nanos.is_none())
    {
        return Err("unfinished retired worker job");
    }
    if frames > dictation_core::audio::MAX_SAMPLES
        || (variant == Variant::Reference && trace.jobs.len() != 1)
    {
        return Err("recording bound or reference job count mismatch");
    }
    let feed = feed.ok_or("missing actual input publications")?;
    let mut through = 0;
    let mut last_after = 0;
    if !feed.timing_valid {
        return Err("input feed timing invalid");
    }
    for publication in &feed.publications {
        if publication.start_frame != through
            || publication.end_frame <= through
            || publication.end_frame > frames
            || publication.before_publish_nanos < publication.planned_nanos
            || publication.before_publish_nanos < last_after
            || publication.after_publish_nanos < publication.before_publish_nanos
            || publication.after_publish_nanos - publication.planned_nanos
                != publication.deviation_nanos
            || publication.deviation_nanos > 10_000_000
            || publication.timing != "onBudget"
        {
            return Err("noncontiguous or inconsistent complete-publication evidence");
        }
        through = publication.end_frame;
        last_after = publication.after_publish_nanos;
    }
    if through != frames {
        return Err("input publications do not cover complete fixture");
    }
    let mut previous_job = 0;
    let mut previous_cut = 0;
    let mut previous_adopt = None;
    let mut processed = 0_usize;
    let mut worker_spans = 0_u128;
    for (index, job) in trace.jobs.iter().enumerate() {
        let final_job = index + 1 == trace.jobs.len();
        if Some(job.session) != session
            || job.generation != generation
            || job.job_id <= previous_job
            || job.language != Some(language.code())
            || job.timeline_start != 0
            || job.storage_start != 0
            || job.timeline_end != job.storage_end
            || job.input_frames != job.timeline_end
            || job.timeline_end > frames
            || job.audio_released_at_submit != Some(final_job)
            || (final_job && job.timeline_end != frames)
            || (!final_job && (variant == Variant::Reference || job.timeline_end <= previous_cut))
        {
            return Err("job identity, prefix order or final whole-recording range mismatch");
        }
        let submit = job
            .host_submit_offset_nanos
            .ok_or("missing host admission time")?;
        let queued = job
            .service_accepted_offset_nanos
            .ok_or("missing service acceptance time")?;
        let begin = job
            .worker_before_decode_offset_nanos
            .ok_or("missing worker entry time")?;
        let end = job
            .worker_after_decode_offset_nanos
            .ok_or("missing worker completion time")?;
        let settled = job
            .worker_settled_offset_nanos
            .ok_or("missing worker settlement time")?;
        let completion = job
            .completion_observed_offset_nanos
            .ok_or("missing observed completion time")?;
        let adopt = job
            .result_adopted_offset_nanos
            .ok_or("missing result adoption time")?;
        if !(submit <= queued
            && queued <= begin
            && begin <= end
            && end <= settled
            && settled <= completion
            && completion <= adopt)
            || previous_adopt.is_some_and(|previous| previous > submit)
            || job.worker_outcome != Some("decoded-ok")
            || job.adoption != Some("accepted")
        {
            return Err("unordered or unsuccessful actual job completion/adoption");
        }
        // These are enclosing worker observer spans, not decoder durations.
        // Accumulate here after validating their ordering; the runner only
        // compares its independently measured decoder sum against this bound.
        worker_spans = worker_spans
            .checked_add(end.checked_sub(begin).ok_or("worker span underflow")?)
            .ok_or("worker span sum overflow")?;
        let required = if final_job {
            Some(frames)
        } else {
            job.timeline_end.checked_add(4800)
        }
        .filter(|required| *required <= frames)
        .ok_or("prefix availability guard outside fixture")?;
        let published = feed
            .publications
            .iter()
            .take_while(|publication| publication.after_publish_nanos <= submit)
            .last()
            .map_or(0, |publication| publication.end_frame);
        proofs.push(InputProof {
            job_id: job.job_id,
            required_stable_frames: Some(required),
            published_through_frames_at_admission: Some(published),
        });
        if job.required_stable_frames != Some(required) || published < required {
            return Err("admission precedes required complete input publication");
        }
        validate_receipt(job, &trace.jobs[..index])?;
        processed = processed
            .checked_add(job.input_frames)
            .ok_or("processed frame sum overflow")?;
        previous_job = job.job_id;
        previous_cut = job.timeline_end;
        previous_adopt = Some(adopt);
    }
    Ok(ValidatedTiming {
        processed_frames: processed,
        worker_spans,
        final_adoption: previous_adopt.ok_or("missing final result adoption")?,
    })
}

fn validate_receipt(
    job: &everlast_dictation::runtime::measurement::job_trace::JobTraceRecord,
    earlier: &[everlast_dictation::runtime::measurement::job_trace::JobTraceRecord],
) -> Result<(), &'static str> {
    use everlast_dictation::transcription::prefix_cache::ReuseOutcome;
    let receipt = job
        .prepared_reuse
        .as_ref()
        .ok_or("missing actual prepared-reuse receipt")?;
    if receipt.session != job.session
        || receipt.generation != job.generation
        || receipt.job_id != job.job_id
    {
        return Err("foreign prepared-reuse receipt identity");
    }
    if receipt.outcome == ReuseOutcome::NotAdmitted {
        return if receipt.origin_job_id.is_none()
            && receipt.prepared_frames.is_none()
            && receipt.prepared_pcm_sha256.is_none()
            && !receipt.decoder_invoked
            && !receipt.native_full_attempted
        {
            Ok(())
        } else {
            Err("nonadmitted receipt invents preparation or native work")
        };
    }
    if receipt
        .prepared_frames
        .is_none_or(|frames| frames < 16000 || frames > job.input_frames.max(16000))
        || receipt.prepared_pcm_sha256.is_none()
    {
        return Err("missing or out-of-bound prepared PCM evidence");
    }
    match receipt.outcome {
        ReuseOutcome::Decoded
            if receipt.origin_job_id == Some(job.job_id)
                && receipt.decoder_invoked
                && receipt.native_full_attempted =>
        {
            Ok(())
        }
        ReuseOutcome::Hit if !receipt.decoder_invoked && !receipt.native_full_attempted => {
            let origin = earlier
                .iter()
                .rev()
                .filter_map(|prior| prior.prepared_reuse.as_ref())
                // Hits preserve one entry. Any subsequent native miss replaces
                // it, and NotAdmitted clears it; history is not another cache.
                .find(|prior| prior.outcome != ReuseOutcome::Hit);
            if origin.is_some_and(|origin| {
                origin.outcome == ReuseOutcome::Decoded
                    && Some(origin.job_id) == receipt.origin_job_id
                    && origin.session == receipt.session
                    && origin.generation == receipt.generation
                    && origin.job_id < receipt.job_id
                    && origin.origin_job_id == Some(origin.job_id)
                    && origin.prepared_frames == receipt.prepared_frames
                    && origin.prepared_pcm_sha256 == receipt.prepared_pcm_sha256
                    && origin.decoder_invoked
                    && origin.native_full_attempted
            }) {
                Ok(())
            } else {
                Err("cache hit lacks an earlier matching native origin")
            }
        }
        _ => Err("failed or inconsistent prepared-reuse receipt"),
    }
}

/// Sum of measured decoder-call durations / unique recording duration.
/// Never pass worker-before/after wall spans as decoder-call measurements.
pub fn session_rtf(sum_decode_nanos: u128, recording_frames: usize) -> Option<f64> {
    if recording_frames == 0 {
        return None;
    }
    let value = sum_decode_nanos as f64 * 16000.0 / (recording_frames as f64 * 1_000_000_000.0);
    value.is_finite().then_some(value)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use everlast_dictation::{
        audio::replay::clock::{Publication, WaitSummary},
        runtime::measurement::job_trace::{JobTraceRecord, MAX_JOBS},
        transcription::prefix_cache::{PreparedReuse, ReuseOutcome},
    };
    use std::time::Duration;
    type TestResult = Result<(), &'static str>;
    fn receipt(
        trace: &mut JobTraceSnapshot,
        index: usize,
    ) -> Result<&mut PreparedReuse, &'static str> {
        trace
            .jobs
            .get_mut(index)
            .ok_or("fixture job missing")?
            .prepared_reuse
            .as_mut()
            .ok_or("fixture receipt missing")
    }

    fn job(id: u64, frames: usize, final_job: bool, at: u128) -> JobTraceRecord {
        JobTraceRecord {
            session: 7,
            generation: 9,
            job_id: id,
            language: Some("de"),
            timeline_start: 0,
            timeline_end: frames,
            storage_start: 0,
            storage_end: frames,
            input_frames: frames,
            required_stable_frames: Some(frames + if final_job { 0 } else { 4800 }),
            audio_released_at_submit: Some(final_job),
            host_submit_offset_nanos: Some(at),
            service_accepted_offset_nanos: Some(at + 1),
            worker_before_decode_offset_nanos: Some(at + 2),
            worker_after_decode_offset_nanos: Some(at + 3),
            worker_settled_offset_nanos: Some(at + 3),
            completion_observed_offset_nanos: Some(at + 4),
            result_adopted_offset_nanos: Some(at + 5),
            worker_outcome: Some("decoded-ok"),
            adoption: Some("accepted"),
            prepared_reuse: Some(PreparedReuse {
                session: 7,
                generation: 9,
                job_id: id,
                outcome: ReuseOutcome::Decoded,
                origin_job_id: Some(id),
                prepared_frames: Some(16000),
                prepared_pcm_sha256: Some([17; 32]),
                decoder_invoked: true,
                native_full_attempted: true,
            }),
        }
    }
    pub(crate) fn fixture() -> Result<(JobTraceSnapshot, FeedEvidence), &'static str> {
        let mut final_job = job(12, 16000, true, 1_000_000_010);
        let receipt = final_job
            .prepared_reuse
            .as_mut()
            .ok_or("fixture receipt missing")?;
        receipt.outcome = ReuseOutcome::Hit;
        receipt.origin_job_id = Some(11);
        receipt.decoder_invoked = false;
        receipt.native_full_attempted = false;
        let trace = JobTraceSnapshot {
            enabled: true,
            session: Some(7),
            maximum_jobs: MAX_JOBS,
            jobs: vec![job(11, 6400, false, 700_000_010), final_job],
            ..JobTraceSnapshot::default()
        };
        let publications = (0..50)
            .map(|i| {
                let at = (i as u128 + 1) * 20_000_000;
                Publication {
                    start_frame: i * 320,
                    end_frame: (i + 1) * 320,
                    planned_nanos: at,
                    before_publish_nanos: at,
                    after_publish_nanos: at + 1,
                    deviation_nanos: 1,
                    timing: "onBudget",
                    diagnostics: None,
                }
            })
            .collect();
        Ok((
            trace,
            FeedEvidence {
                publications,
                timing_valid: true,
                planned_stop_offset: Duration::from_secs(1),
                wait_summary: WaitSummary::default(),
            },
        ))
    }
    fn check(trace: JobTraceSnapshot, feed: &FeedEvidence, variant: Variant) -> Evidence {
        audit(
            trace,
            Some(feed),
            Some(7),
            9,
            16000,
            Language::German,
            variant,
        )
    }

    #[test]
    fn overlapping_prefix_and_same_prepared_final_hit_preserve_unique_session_denominator()
    -> TestResult {
        let (trace, feed) = fixture()?;
        let evidence = check(trace, &feed, Variant::Candidate);
        assert_eq!(evidence.error, None);
        assert_eq!(evidence.processed_frames_sum, Some(22400));
        assert_eq!(
            evidence.input_proofs[0].published_through_frames_at_admission,
            Some(11200)
        );
        Ok(())
    }

    #[test]
    fn reference_is_exactly_one_final_native_or_honest_nonadmitted_call() -> TestResult {
        let (mut trace, feed) = fixture()?;
        trace.jobs = vec![job(20, 16000, true, 1_000_000_010)];
        assert_eq!(check(trace.clone(), &feed, Variant::Reference).error, None);
        let receipt = trace.jobs[0]
            .prepared_reuse
            .as_mut()
            .ok_or("fixture receipt missing")?;
        receipt.outcome = ReuseOutcome::NotAdmitted;
        receipt.origin_job_id = None;
        receipt.prepared_frames = None;
        receipt.prepared_pcm_sha256 = None;
        receipt.decoder_invoked = false;
        receipt.native_full_attempted = false;
        assert_eq!(check(trace, &feed, Variant::Reference).error, None);
        let (trace, _) = fixture()?;
        assert!(check(trace, &feed, Variant::Reference).error.is_some());
        Ok(())
    }

    #[test]
    fn actual_complete_publication_not_before_publish_controls_prefix_admission() -> TestResult {
        let (trace, mut feed) = fixture()?;
        feed.publications[34].after_publish_nanos = 700_000_011;
        feed.publications[34].deviation_nanos = 11;
        let evidence = check(trace, &feed, Variant::Candidate);
        assert!(evidence.error.is_some());
        assert_eq!(
            evidence.input_proofs[0].published_through_frames_at_admission,
            Some(10880)
        );
        Ok(())
    }

    #[test]
    fn independent_guard_identity_order_and_receipt_tampering_fail_closed() -> TestResult {
        type Mutation = fn(&mut JobTraceSnapshot, &mut FeedEvidence) -> TestResult;
        let mutations: &[(&str, Mutation)] = &[
            ("stable marker zero", |t, _| {
                t.jobs[0].required_stable_frames = Some(0);
                Ok(())
            }),
            ("missing submit", |t, _| {
                t.jobs[0].host_submit_offset_nanos = None;
                Ok(())
            }),
            ("missing role", |t, _| {
                t.jobs[0].audio_released_at_submit = None;
                Ok(())
            }),
            ("final before release", |t, _| {
                t.jobs[1].audio_released_at_submit = Some(false);
                Ok(())
            }),
            ("suffix final", |t, _| {
                t.jobs[1].timeline_start = 6400;
                Ok(())
            }),
            ("duplicate job", |t, _| {
                t.jobs[1].job_id = 11;
                Ok(())
            }),
            ("foreign generation", |t, _| {
                t.jobs[0].generation = 10;
                Ok(())
            }),
            ("storage offset", |t, _| {
                t.jobs[0].storage_start = 1;
                Ok(())
            }),
            ("missing completion", |t, _| {
                t.jobs[0].completion_observed_offset_nanos = None;
                Ok(())
            }),
            ("adopt before completion", |t, _| {
                t.jobs[0].result_adopted_offset_nanos = Some(0);
                Ok(())
            }),
            ("missing receipt", |t, _| {
                t.jobs[0].prepared_reuse = None;
                Ok(())
            }),
            ("foreign receipt", |t, _| {
                receipt(t, 0)?.session = 8;
                Ok(())
            }),
            ("hit invented full", |t, _| {
                receipt(t, 1)?.native_full_attempted = true;
                Ok(())
            }),
            ("unknown origin", |t, _| {
                receipt(t, 1)?.origin_job_id = Some(10);
                Ok(())
            }),
            ("future origin", |t, _| {
                receipt(t, 1)?.origin_job_id = Some(12);
                Ok(())
            }),
            ("different hash", |t, _| {
                receipt(t, 1)?.prepared_pcm_sha256 = Some([18; 32]);
                Ok(())
            }),
            ("different prepared length", |t, _| {
                receipt(t, 1)?.prepared_frames = Some(16001);
                Ok(())
            }),
            ("native not attempted", |t, _| {
                receipt(t, 0)?.native_full_attempted = false;
                Ok(())
            }),
            ("publication hole", |_, f| {
                f.publications[0].start_frame = 1;
                Ok(())
            }),
            ("incomplete feed", |_, f| {
                f.publications.pop();
                Ok(())
            }),
            ("overflow", |t, _| {
                t.overflow_events = 1;
                Ok(())
            }),
            ("raw trace failure", |t, _| {
                t.error = Some("retained failure".into());
                Ok(())
            }),
        ];
        for (label, mutate) in mutations {
            let (mut trace, mut feed) = fixture()?;
            mutate(&mut trace, &mut feed)?;
            let evidence = check(trace, &feed, Variant::Candidate);
            assert!(evidence.error.is_some(), "accepted {label}");
            assert_eq!(
                evidence.processed_frames_sum, None,
                "qualified invalid {label}"
            );
            assert_eq!(evidence.trace.jobs.len(), 2, "discarded raw {label}");
        }
        Ok(())
    }

    #[test]
    fn session_rtf_uses_unique_recording_frames_not_repeated_processed_frames() -> TestResult {
        let repeated_processed_frames = 48000;
        assert_ne!(repeated_processed_frames, 16000);
        assert_eq!(session_rtf(600_000_000, 16000), Some(0.6));
        assert_eq!(session_rtf(600_000_000, 0), None);
        assert_eq!(session_rtf(0, 16000), Some(0.0));
        Ok(())
    }

    #[test]
    fn a_single_entry_cache_cannot_hit_an_evicted_native_origin() -> TestResult {
        let (mut trace, feed) = fixture()?;
        let mut intermediate = job(12, 9600, false, 900_000_010);
        intermediate
            .prepared_reuse
            .as_mut()
            .ok_or("fixture receipt missing")?
            .prepared_pcm_sha256 = Some([18; 32]);
        trace.jobs[1].job_id = 13;
        receipt(&mut trace, 1)?.job_id = 13;
        trace.jobs.insert(1, intermediate);
        assert!(
            check(trace, &feed, Variant::Candidate).error.is_some(),
            "evicted origin was reusable"
        );
        Ok(())
    }

    #[test]
    fn missing_trace_is_not_successful_overlap_evidence() -> TestResult {
        let evidence = audit(
            JobTraceSnapshot::default(),
            None,
            Some(7),
            9,
            16000,
            Language::German,
            Variant::Candidate,
        );
        assert!(evidence.error.is_some(), "absent actual trace was accepted");
        assert_eq!(evidence.processed_frames_sum, None);
        Ok(())
    }
}
