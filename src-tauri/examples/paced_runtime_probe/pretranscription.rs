//! Post-run proof from independent actual complete-publication timestamps.
use everlast_dictation::{
    audio::replay::clock::FeedEvidence,
    runtime::measurement::job_trace::{JobTraceSnapshot, MAX_JOBS},
};
use serde::Serialize;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputProof {
    job_id: u64,
    required_stable_frames: Option<usize>,
    published_through_frames_at_admission: Option<usize>,
    last_complete_publication_offset_nanos: Option<u128>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub trace: JobTraceSnapshot,
    pub input_proofs: Vec<InputProof>,
    pub error: Option<String>,
}
pub fn audit(
    trace: JobTraceSnapshot,
    feed: Option<&FeedEvidence>,
    session: Option<u64>,
    frames: usize,
    language: everlast_dictation::transcription::Language,
) -> Evidence {
    let mut proofs = Vec::new();
    let mut error = trace.error.clone();
    let mut fail = |why: &str| {
        if error.is_none() {
            error = Some(why.into());
        }
    };
    if !trace.enabled || trace.session != session || session.is_none() {
        fail("missing/current-session job trace unavailable");
    }
    if trace.maximum_jobs != MAX_JOBS || trace.jobs.len() > MAX_JOBS || trace.overflow_events != 0 {
        fail("bounded job trace overflow or invalid capacity");
    }
    let mut covered = 0;
    let mut previous_job = 0;
    if trace.jobs.len() <= MAX_JOBS {
        for job in &trace.jobs {
            let mut published = None;
            let mut publication_at = None;
            if let (Some(feed), Some(admission)) = (feed, job.host_submit_offset_nanos) {
                let mut through = 0;
                for publication in &feed.publications {
                    if publication.after_publish_nanos <= admission {
                        if publication.start_frame != through
                            || publication.after_publish_nanos < publication.before_publish_nanos
                        {
                            fail("noncontiguous or inconsistent input publication proof");
                            break;
                        }
                        through = publication.end_frame;
                        publication_at = Some(publication.after_publish_nanos);
                    } else {
                        break;
                    }
                }
                published = Some(through);
            } else {
                fail("missing complete-publication/admission evidence");
            }
            let required = match job.audio_released_at_submit {
                Some(false) => job.timeline_end.checked_add(4800),
                Some(true) => Some(job.timeline_end),
                None => None,
            };
            if required.is_none() || required != job.required_stable_frames {
                fail("missing or inconsistent source-role/stable-frame boundary");
            }
            if job.required_stable_frames.is_none_or(|required| {
                required > frames || published.is_none_or(|available| available < required)
            }) {
                fail("job admission lacks already-published required stable frames");
            }
            proofs.push(InputProof {
                job_id: job.job_id,
                required_stable_frames: job.required_stable_frames,
                published_through_frames_at_admission: published,
                last_complete_publication_offset_nanos: publication_at,
            });
            if Some(job.session) != session
                || job.language != Some(language.code())
                || job.generation != 1
                || job.job_id <= previous_job
                || job.timeline_start != covered
                || job.timeline_end < job.timeline_start
                || job.timeline_end > frames
                || job.storage_end < job.storage_start
                || job.timeline_end - job.timeline_start != job.input_frames
                || job.storage_end - job.storage_start != job.input_frames
            {
                fail("job identity/range coverage mismatch");
            }
            let ordered = job
                .host_submit_offset_nanos
                .zip(job.service_accepted_offset_nanos)
                .zip(job.worker_before_decode_offset_nanos)
                .zip(job.worker_after_decode_offset_nanos)
                .zip(job.result_adopted_offset_nanos)
                .is_some_and(|((((submit, queued), begin), end), adopt)| {
                    submit <= queued && queued <= begin && begin <= end && end <= adopt
                });
            if !ordered
                || job.worker_outcome != Some("decoded-ok")
                || job.adoption != Some("accepted")
            {
                fail("missing, rejected or inconsistent actual job observations");
            }
            covered = job.timeline_end;
            previous_job = job.job_id;
        }
    }
    if covered != frames {
        fail("final accepted jobs do not cover complete fixture");
    }
    Evidence {
        trace,
        input_proofs: proofs,
        error,
    }
}
