//! Measurement-only phase evidence. This does not change scheduling or decoding.
pub use super::phase::Phase;
use macos_integration::thread_observation::{ObservationError, ThreadSnapshot};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct EngineProfile {
    /// None means that phase was never completed; Some(ZERO) is a measured zero.
    pub phases: [Option<Duration>; 7],
    pub total_elapsed: Duration,
    pub thread_before: Result<ThreadSnapshot, ObservationError>,
    pub thread_after: Result<ThreadSnapshot, ObservationError>,
    /// Profile protocol failure leaves the Engine result intact, but makes the
    /// diagnostic profile invalid. No guessed timings are exported.
    pub error: Option<&'static str>,
}
impl EngineProfile {
    pub fn elapsed(&self, phase: Phase) -> Option<Duration> {
        self.phases[phase.index()]
    }
}

pub(super) struct Collector {
    started: Instant,
    active: Option<(Phase, Instant)>,
    phases: [Option<Duration>; 7],
    before: Result<ThreadSnapshot, ObservationError>,
    error: Option<&'static str>,
}
impl Collector {
    pub(super) fn new() -> Self {
        let before = macos_integration::thread_observation::current();
        Self {
            started: Instant::now(),
            active: None,
            phases: [None; 7],
            before,
            error: None,
        }
    }
    pub(super) fn start(&mut self, phase: Phase) {
        self.start_at(phase, Instant::now());
    }
    pub(super) fn end(&mut self, phase: Phase) {
        self.end_at(phase, Instant::now());
    }
    fn start_at(&mut self, phase: Phase, at: Instant) {
        if self.active.is_some() || self.phases[phase.index()].is_some() {
            self.error = Some("overlapping or repeated phase");
            return;
        }
        self.active = Some((phase, at));
    }
    fn end_at(&mut self, phase: Phase, at: Instant) {
        match self.active.take() {
            Some((expected, started)) if expected == phase => {
                match at.checked_duration_since(started) {
                    Some(elapsed) => self.phases[phase.index()] = Some(elapsed),
                    None => self.error = Some("phase clock moved backwards"),
                }
            }
            _ => self.error = Some("phase completion without matching start"),
        }
    }
    pub(super) fn finish(mut self) -> EngineProfile {
        let total_elapsed = self.started.elapsed();
        if self.active.is_some() {
            self.error = Some("unfinished phase");
        }
        let after = macos_integration::thread_observation::current();
        EngineProfile {
            phases: self.phases,
            total_elapsed,
            thread_before: self.before,
            thread_after: after,
            error: self.error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_error_phase_is_measured_while_unreached_decode_is_absent() {
        let mut profile = Collector::new();
        let start = Instant::now();
        profile.start_at(Phase::ValidationEnergy, start);
        profile.end_at(Phase::ValidationEnergy, start);
        profile.start_at(Phase::VadInference, start);
        profile.end_at(Phase::VadInference, start + Duration::from_millis(3));
        let completed = profile.finish();
        assert_eq!(
            completed.elapsed(Phase::ValidationEnergy),
            Some(Duration::ZERO)
        );
        assert_eq!(
            completed.elapsed(Phase::VadInference),
            Some(Duration::from_millis(3))
        );
        assert_eq!(completed.elapsed(Phase::StateCreate), None);
        assert_eq!(completed.elapsed(Phase::NativeFull), None);
        assert_eq!(completed.elapsed(Phase::StateDrop), None);
        assert_eq!(completed.error, None);
    }
    #[test]
    fn incomplete_or_repeated_phase_never_silently_overwrites_evidence() {
        let mut profile = Collector::new();
        profile.start(Phase::StateCreate);
        assert_eq!(profile.finish().error, Some("unfinished phase"));
        let mut profile = Collector::new();
        profile.start(Phase::TextJoin);
        profile.end(Phase::TextJoin);
        profile.start(Phase::TextJoin);
        assert_eq!(
            profile.finish().error,
            Some("overlapping or repeated phase")
        );
    }
}
