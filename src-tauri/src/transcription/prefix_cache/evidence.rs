//! Text/PCM-free evidence from one actual scoped decoder invocation.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReuseOutcome {
    Decoded,
    Hit,
    NotAdmitted,
    Failed,
    InvalidContext,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedReuse {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    pub outcome: ReuseOutcome,
    pub origin_job_id: Option<u64>,
    pub prepared_frames: Option<usize>,
    /// Exact little-endian f32 bytes after current normal preparation.
    pub prepared_pcm_sha256: Option<[u8; 32]>,
    /// Entered the normal fresh-state decoder closure, not necessarily full().
    pub decoder_invoked: bool,
    /// Observed immediately before the actual native full() call.
    pub native_full_attempted: bool,
}

#[derive(Default)]
pub(super) struct NativeObservation {
    pub(super) full_attempted: bool,
}

impl super::super::ObservationSink for NativeObservation {
    fn full_started(&mut self) {
        self.full_attempted = true;
    }
}
