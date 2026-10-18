//! Measurement-only, PCM/text-free serial-worker observations.
use super::*;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub session: u64,
    pub generation: u64,
    pub job: u64,
    pub timeline: Range<usize>,
    pub storage: Range<usize>,
    pub language: Option<Language>,
}
impl Identity {
    pub fn of(job: &Job) -> Self {
        Self {
            session: job.session,
            generation: job.generation,
            job: job.job_id,
            timeline: job.timeline_range.clone(),
            storage: job.sample_range.clone(),
            language: Some(job.language),
        }
    }
}
pub(crate) enum Edge {
    Queued,
    BeforeDecode,
    AfterDecode(&'static str),
    CancelledPending,
}
pub(crate) trait Observer: Send + Sync {
    fn observe(&self, identity: Identity, at: Instant, edge: Edge);
}
impl Shared {
    pub(super) fn observe(&self, job: &Job, edge: Edge) {
        let observer = self
            .observer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(observer) = observer {
            observer.observe(Identity::of(job), Instant::now(), edge);
        }
    }
}
