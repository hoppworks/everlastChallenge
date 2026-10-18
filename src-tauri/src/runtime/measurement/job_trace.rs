//! Bounded text/PCM-free observations; never called by the RT publisher.
use super::*;
use crate::inference_service::{
    Completed, Job,
    observation::{Edge, Identity, Observer},
};
use crate::transcription::prefix_cache::PreparedReuse;
use serde::Serialize;
use std::time::Instant;

pub const MAX_JOBS: usize = 601;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobTraceRecord {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    pub language: Option<&'static str>,
    pub timeline_start: usize,
    pub timeline_end: usize,
    pub storage_start: usize,
    pub storage_end: usize,
    pub input_frames: usize,
    pub required_stable_frames: Option<usize>,
    pub audio_released_at_submit: Option<bool>,
    pub host_submit_offset_nanos: Option<u128>,
    pub service_accepted_offset_nanos: Option<u128>,
    pub worker_before_decode_offset_nanos: Option<u128>,
    pub worker_after_decode_offset_nanos: Option<u128>,
    pub worker_settled_offset_nanos: Option<u128>,
    pub result_adopted_offset_nanos: Option<u128>,
    /// Runtime actually collected Completed, even if its optional receipt is absent.
    pub completion_observed_offset_nanos: Option<u128>,
    /// Raw receipt, including foreign tags; never repaired into current identity.
    pub prepared_reuse: Option<PreparedReuse>,
    pub worker_outcome: Option<&'static str>,
    pub adoption: Option<&'static str>,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobTraceSnapshot {
    pub enabled: bool,
    pub session: Option<u64>,
    pub maximum_jobs: usize,
    pub jobs: Vec<JobTraceRecord>,
    pub retired_job: Option<JobTraceRecord>,
    pub first_overflow_job: Option<JobTraceRecord>,
    pub error: Option<String>,
    pub overflow_events: u64,
}
struct Record {
    identity: Identity,
    origin: Option<Instant>,
    required_stable_frames: Option<usize>,
    audio_released: Option<bool>,
    submitted: Option<Instant>,
    queued: Option<Instant>,
    begin: Option<Instant>,
    end: Option<Instant>,
    settled: Option<Instant>,
    adopted: Option<Instant>,
    completed: Option<Instant>,
    prepared_reuse: Option<PreparedReuse>,
    worker_outcome: Option<&'static str>,
    adoption: Option<&'static str>,
}
impl Record {
    fn new(identity: Identity, origin: Option<Instant>) -> Self {
        Self {
            identity,
            origin,
            required_stable_frames: None,
            audio_released: None,
            submitted: None,
            queued: None,
            begin: None,
            end: None,
            settled: None,
            adopted: None,
            completed: None,
            prepared_reuse: None,
            worker_outcome: None,
            adoption: None,
        }
    }
    fn snapshot(&self) -> JobTraceRecord {
        // Each retired/current record retains its own session's actual t0.
        let offset = |at: Option<Instant>| {
            at.zip(self.origin)
                .and_then(|(at, t0)| at.checked_duration_since(t0))
                .map(|d| d.as_nanos())
        };
        JobTraceRecord {
            session: self.identity.session,
            generation: self.identity.generation,
            job_id: self.identity.job,
            language: self.identity.language.map(|language| language.code()),
            timeline_start: self.identity.timeline.start,
            timeline_end: self.identity.timeline.end,
            storage_start: self.identity.storage.start,
            storage_end: self.identity.storage.end,
            input_frames: self.identity.storage.len(),
            required_stable_frames: self.required_stable_frames,
            audio_released_at_submit: self.audio_released,
            host_submit_offset_nanos: offset(self.submitted),
            service_accepted_offset_nanos: offset(self.queued),
            worker_before_decode_offset_nanos: offset(self.begin),
            worker_after_decode_offset_nanos: offset(self.end),
            worker_settled_offset_nanos: offset(self.settled),
            result_adopted_offset_nanos: offset(self.adopted),
            completion_observed_offset_nanos: offset(self.completed),
            prepared_reuse: self.prepared_reuse.clone(),
            worker_outcome: self.worker_outcome,
            adoption: self.adoption,
        }
    }
}
pub(super) struct Ledger {
    session: Option<u64>,
    records: Vec<Record>,
    retired: Option<Record>,
    first_overflow: Option<Record>,
    error: Option<&'static str>,
    overflow: u64,
    limit: usize,
}
impl Ledger {
    pub(super) fn new() -> Result<Self, RuntimeError> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(MAX_JOBS)
            .map_err(|_| RuntimeError::Service("job trace allocation failed".into()))?;
        Ok(Self {
            session: None,
            records,
            retired: None,
            first_overflow: None,
            error: None,
            overflow: 0,
            limit: MAX_JOBS,
        })
    }
    pub(super) fn reset(&mut self, session: u64) {
        // One serial worker can own one accepted unfinished job, even before
        // the worker-before-decode edge (it may be descheduled after dequeue).
        // Preserve it independently of the new session, without an audio Arc.
        let retiring = self
            .records
            .iter()
            .position(|r| r.queued.is_some() && r.settled.is_none())
            .map(|index| self.records.remove(index));
        let retirement_error = if let Some(retiring) = retiring {
            if self.retired.as_ref().is_some_and(|r| r.settled.is_none()) {
                Some("multiple unfinished retired worker jobs")
            } else {
                self.retired = Some(retiring);
                None
            }
        } else {
            None
        };
        self.session = Some(session);
        self.records.clear();
        self.error = retirement_error;
        self.overflow = 0;
        self.first_overflow = None;
    }
    fn record(
        &mut self,
        identity: Identity,
        origin: Option<Instant>,
        admission: bool,
    ) -> Option<&mut Record> {
        if self.session != Some(identity.session) {
            return self.retired.as_mut().filter(|r| r.identity == identity);
        }
        if let Some(index) = self.records.iter().position(|r| r.identity == identity) {
            if admission {
                self.error.get_or_insert("duplicate host job admission");
            }
            return self.records.get_mut(index);
        }
        if self.records.len() >= self.limit {
            if let Some(next) = self.overflow.checked_add(1) {
                self.overflow = next;
            } else {
                self.error = Some("job trace overflow counter exceeded u64");
            }
            self.error
                .get_or_insert("bounded job trace overflow; counter bounded by u64");
            self.first_overflow
                .get_or_insert_with(|| Record::new(identity, origin));
            return None;
        }
        if !admission {
            self.error
                .get_or_insert("job observation has no matching host identity");
        }
        self.records.push(Record::new(identity, origin));
        self.records.last_mut()
    }
    #[cfg(test)]
    pub(super) fn limit_for_test(&mut self, limit: usize) {
        self.limit = limit.min(MAX_JOBS);
    }
    pub(super) fn snapshot(&self) -> JobTraceSnapshot {
        JobTraceSnapshot {
            enabled: true,
            session: self.session,
            maximum_jobs: MAX_JOBS,
            error: self.error.map(str::to_owned),
            overflow_events: self.overflow,
            jobs: self.records.iter().map(Record::snapshot).collect(),
            retired_job: self.retired.as_ref().map(Record::snapshot),
            first_overflow_job: self.first_overflow.as_ref().map(Record::snapshot),
        }
    }
}
fn once(slot: &mut Option<Instant>, at: Instant) -> bool {
    if slot.is_some() {
        false
    } else {
        *slot = Some(at);
        true
    }
}
impl Trace {
    pub(in crate::runtime) fn jobs_enabled(&self) -> bool {
        self.jobs.is_some()
    }
    pub(in crate::runtime) fn submit_job(&self, job: &Job, early: bool) {
        let Some(ledger) = &self.jobs else {
            return;
        };
        let at = Instant::now();
        let origin = self.value.lock().unwrap_or_else(|p| p.into_inner()).t0;
        let mut ledger = ledger.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(record) = ledger.record(Identity::of(job), origin, true)
            && record.submitted.is_none()
        {
            record.submitted = Some(at);
            record.audio_released = Some(!early);
            record.required_stable_frames = if early {
                job.timeline_range.end.checked_add(4800)
            } else {
                Some(job.timeline_range.end)
            };
        }
    }
    pub(in crate::runtime) fn completed_job(&self, done: &Completed) {
        let Some(ledger) = &self.jobs else {
            return;
        };
        let at = Instant::now();
        let mut ledger = ledger.lock().unwrap_or_else(|p| p.into_inner());
        // Completed carries no language; use only an already admitted record,
        // matching every available service identity tag. Receipt tags are data,
        // never the authority selecting the record (including a retired job).
        let matches = |record: &&mut Record| {
            let identity = &record.identity;
            identity.session == done.session
                && identity.generation == done.generation
                && identity.job == done.job_id
                && identity.storage == done.sample_range
                && identity.timeline == done.timeline_range
        };
        let Ledger {
            records,
            retired,
            error,
            ..
        } = &mut *ledger;
        let Some(record) = records.iter_mut().chain(retired.iter_mut()).find(matches) else {
            error.get_or_insert("completed result has no exact admitted trace identity");
            return;
        };
        if !once(&mut record.completed, at) {
            error.get_or_insert("duplicate completed result observation");
            return;
        }
        record.prepared_reuse = done.prepared_reuse.clone();
        if record.prepared_reuse.as_ref().is_some_and(|receipt| {
            receipt.session != done.session
                || receipt.generation != done.generation
                || receipt.job_id != done.job_id
        }) {
            error.get_or_insert("completed result contains foreign prepared-reuse receipt");
        }
    }
    pub(in crate::runtime) fn adopt_job(&self, event: &Event, at: Instant, accepted: bool) {
        let Some(ledger) = &self.jobs else {
            return;
        };
        let Event::InferenceFinished {
            session,
            generation,
            job_id,
            sample_range,
            timeline_range,
            ..
        } = event
        else {
            return;
        };
        let origin = self.value.lock().unwrap_or_else(|p| p.into_inner()).t0;
        let mut ledger = ledger.lock().unwrap_or_else(|p| p.into_inner());
        let language = ledger
            .records
            .iter()
            .chain(ledger.retired.iter())
            .find(|r| r.identity.job == *job_id && r.identity.session == *session)
            .and_then(|r| r.identity.language);
        let key = Identity {
            session: *session,
            generation: *generation,
            job: *job_id,
            timeline: timeline_range.clone(),
            storage: sample_range.clone(),
            language,
        };
        if let Some(record) = ledger.record(key, origin, false) {
            if once(&mut record.adopted, at) {
                record.adoption = Some(if accepted { "accepted" } else { "rejected" });
            } else {
                ledger.error.get_or_insert("duplicate result adoption");
            }
        }
    }
}
impl Observer for Trace {
    fn observe(&self, identity: Identity, at: Instant, edge: Edge) {
        let Some(ledger) = &self.jobs else {
            return;
        };
        let origin = self.value.lock().unwrap_or_else(|p| p.into_inner()).t0;
        let mut ledger = ledger.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(record) = ledger.record(identity, origin, false) {
            let valid = match edge {
                Edge::Queued => once(&mut record.queued, at),
                Edge::BeforeDecode => once(&mut record.begin, at),
                Edge::AfterDecode(outcome) => {
                    if record.begin.is_some() {
                        let _first = once(&mut record.end, at);
                    }
                    let first = once(&mut record.settled, at);
                    if first {
                        record.worker_outcome = Some(outcome);
                    }
                    first
                }
                Edge::CancelledPending => {
                    let first = once(&mut record.settled, at);
                    if first {
                        record.worker_outcome = Some("cancelled-pending-no-decode");
                    }
                    first
                }
            };
            if !valid {
                ledger.error.get_or_insert("duplicate worker observation");
            }
        }
    }
}
