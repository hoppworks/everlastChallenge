//! Per-recording limits driven by elapsed monotone time, without clocks or worker I/O.
use crate::audio::MAX_SECONDS;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitDecision {
    Continue,
    Warn,
    Stop,
}

#[derive(Debug, Default)]
pub struct RecordingLimits {
    warned: bool,
    stopped: bool,
}

impl RecordingLimits {
    /// The caller supplies elapsed monotone time since stream start. Construct a
    /// fresh value for each session. Stop is terminal, even if a later time input
    /// decreases. A frame limit may stop capture before time does. Stop takes
    /// precedence over a warning when a delayed poll already passed both boundaries.
    #[must_use]
    pub fn advance(&mut self, elapsed: Duration, frame_limit_reached: bool) -> LimitDecision {
        if self.stopped || frame_limit_reached || elapsed >= Duration::from_secs(MAX_SECONDS as u64)
        {
            self.stopped = true;
            return LimitDecision::Stop;
        }
        if !self.warned && elapsed >= Duration::from_secs((MAX_SECONDS - 30) as u64) {
            self.warned = true;
            LimitDecision::Warn
        } else {
            LimitDecision::Continue
        }
    }
}
