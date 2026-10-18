//! One bounded diagnostic job identity; holds no Runtime, text or PCM owner.
use std::{fmt, ops::Range, sync::Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub ordinal: u64,
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    pub storage: Range<usize>,
    pub timeline: Range<usize>,
    pub frames: usize,
    pub language: &'static str,
    pub pcm_sha256: String,
}
impl Request {
    pub fn matches_completed(
        &self,
        session: u64,
        generation: u64,
        job_id: u64,
        storage: &Range<usize>,
        timeline: &Range<usize>,
    ) -> bool {
        self.session == session
            && self.generation == generation
            && self.job_id == job_id
            && &self.storage == storage
            && &self.timeline == timeline
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GateError(pub &'static str);
impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for GateError {}
#[derive(Default)]
struct State {
    expected: Option<Request>,
    consumed: bool,
    error: Option<&'static str>,
}
#[derive(Clone, Debug)]
pub(super) struct Evidence {
    pub expected: Option<Request>,
    pub consumed: bool,
    pub error: Option<&'static str>,
}
#[derive(Default)]
pub(super) struct RequestGate(Mutex<State>);
impl RequestGate {
    pub fn arm(&self, request: Request) -> Result<(), GateError> {
        if request.ordinal == 0
            || request.session == 0
            || request.generation == 0
            || request.job_id == 0
            || request.frames == 0
            || request.frames > 4_800_000
            || request.storage.start >= request.storage.end
            || request.storage.end > 4_800_000
            || request.storage.len() != request.frames
            || request.timeline.start >= request.timeline.end
            || request.timeline.end > 4_800_000
            || request.timeline.len() != request.frames
            || !["de", "en"].contains(&request.language)
            || request.pcm_sha256.len() != 64
            || !request.pcm_sha256.bytes().all(|v| v.is_ascii_hexdigit())
        {
            return Err(GateError("invalid diagnostic request"));
        }
        let mut state = self
            .0
            .lock()
            .map_err(|_| GateError("request lock poisoned"))?;
        if state.expected.is_some() || state.error.is_some() {
            return Err(GateError("request still owned or invalid"));
        }
        state.expected = Some(request);
        state.consumed = false;
        Ok(())
    }
    pub fn consume(
        &self,
        frames: usize,
        language: &str,
        pcm_sha256: &str,
    ) -> Result<Request, GateError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| GateError("request lock poisoned"))?;
        if state.error.is_some() {
            return Err(GateError("request protocol invalid"));
        }
        let Some(expected) = state.expected.clone() else {
            state.error = Some("decoder called without armed request");
            return Err(GateError("decoder called without armed request"));
        };
        if state.consumed
            || expected.frames != frames
            || expected.language != language
            || expected.pcm_sha256 != pcm_sha256
        {
            state.error = Some("duplicate or mismatching decoder input");
            return Err(GateError("duplicate or mismatching decoder input"));
        }
        state.consumed = true;
        Ok(expected)
    }
    pub fn acknowledge(&self, request: &Request) -> Result<(), GateError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| GateError("request lock poisoned"))?;
        if state.error.is_some() || !state.consumed || state.expected.as_ref() != Some(request) {
            return Err(GateError("receipt does not match retained request"));
        }
        state.expected = None;
        state.consumed = false;
        Ok(())
    }
    pub fn evidence(&self) -> Result<Evidence, GateError> {
        let state = self
            .0
            .lock()
            .map_err(|_| GateError("request lock poisoned"))?;
        Ok(Evidence {
            expected: state.expected.clone(),
            consumed: state.consumed,
            error: state.error,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        Request {
            ordinal: 1,
            session: 7,
            generation: 8,
            job_id: 9,
            storage: 0..2,
            timeline: 10..12,
            frames: 2,
            language: "de",
            pcm_sha256: "a".repeat(64),
        }
    }
    #[test]
    fn admitted_identity_is_retained_until_matching_completion_receipt() -> Result<(), GateError> {
        let gate = RequestGate::default();
        let expected = request();
        gate.arm(expected.clone())?;
        let actual = gate.consume(2, "de", &"a".repeat(64))?;
        assert_eq!(actual, expected);
        assert!(actual.matches_completed(7, 8, 9, &(0..2), &(10..12)));
        assert!(!actual.matches_completed(7, 8, 10, &(0..2), &(10..12)));
        assert!(gate.arm(expected.clone()).is_err());
        assert_eq!(gate.evidence()?.expected, Some(expected.clone()));
        assert!(gate.evidence()?.consumed);
        assert_eq!(gate.evidence()?.error, None);
        let mut wrong = expected;
        wrong.generation += 1;
        assert!(gate.acknowledge(&wrong).is_err());
        gate.acknowledge(&actual)?;
        assert!(gate.evidence()?.expected.is_none());
        Ok(())
    }
    #[test]
    fn unarmed_or_duplicate_decode_cannot_be_reassociated_with_a_later_job() -> Result<(), GateError>
    {
        let unarmed = RequestGate::default();
        assert!(unarmed.consume(2, "de", &"a".repeat(64)).is_err());
        assert!(unarmed.arm(request()).is_err());
        assert!(unarmed.evidence()?.expected.is_none());
        let gate = RequestGate::default();
        let expected = request();
        gate.arm(expected.clone())?;
        gate.consume(2, "de", &"a".repeat(64))?;
        assert!(gate.consume(2, "de", &"a".repeat(64)).is_err());
        assert!(gate.acknowledge(&expected).is_err());
        assert_eq!(gate.evidence()?.expected, Some(expected));
        Ok(())
    }
    #[test]
    fn mismatching_pcm_language_or_frames_preserves_expected_and_poison() -> Result<(), GateError> {
        for (frames, language, hash) in [
            (3, "de", "a".repeat(64)),
            (2, "en", "a".repeat(64)),
            (2, "de", "b".repeat(64)),
        ] {
            let gate = RequestGate::default();
            let expected = request();
            gate.arm(expected.clone())?;
            assert!(gate.consume(frames, language, &hash).is_err());
            assert_eq!(gate.evidence()?.expected, Some(expected.clone()));
            assert!(!gate.evidence()?.consumed);
            assert!(gate.evidence()?.error.is_some());
            assert!(gate.consume(2, "de", &"a".repeat(64)).is_err());
            assert!(gate.arm(expected).is_err());
        }
        Ok(())
    }
}
