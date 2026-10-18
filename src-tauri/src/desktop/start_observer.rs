//! Passive host-start upper bounds only. Never authorizes an App command.
use std::time::{Duration, Instant};

pub const MAX_RECORDS: usize = 181;
pub const ACK_BUDGET: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Starting,
    Recording,
    Other,
}
#[derive(Clone, Copy)]
pub struct State {
    pub phase: Phase,
    pub sid: Option<u64>,
    pub ready: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub ordinal: u64,
    pub sid: Option<u64>,
    pub upper_nanos: Option<u64>,
    pub ack_nanos: Option<u64>,
    pub reason: Option<&'static str>,
}
struct Pending {
    ordinal: u64,
    origin: Instant,
    admitted: bool,
    sid: Option<u64>,
    ack: Option<Instant>,
    recording: Option<Instant>,
}
pub struct Observer {
    pending: Option<Pending>,
    records: Vec<Record>,
    attempts: u64,
    overflow: u64,
    counter_error: bool,
}
impl Default for Observer {
    fn default() -> Self {
        Self {
            pending: None,
            records: Vec::with_capacity(MAX_RECORDS),
            attempts: 0,
            overflow: 0,
            counter_error: false,
        }
    }
}
impl Observer {
    pub fn begin(&mut self, origin: Instant) -> Option<u64> {
        let overlapping = self.pending.is_some();
        self.invalidate("overlapping-start");
        self.attempts = match self.attempts.checked_add(1) {
            Some(v) => v,
            None => {
                self.counter_error = true;
                return None;
            }
        };
        if self.records.len() >= MAX_RECORDS {
            match self.overflow.checked_add(1) {
                Some(v) => self.overflow = v,
                None => self.counter_error = true,
            }
            return None;
        }
        self.pending = Some(Pending {
            ordinal: self.attempts,
            origin,
            admitted: false,
            sid: None,
            ack: None,
            recording: None,
        });
        if overlapping {
            self.invalidate("overlapping-start");
            return None;
        }
        Some(self.attempts)
    }
    pub fn admit(&mut self, ticket: u64, state: State, no_pending: bool) {
        if !self.current(ticket) {
            return;
        }
        if state.phase != Phase::Idle || state.sid.is_some() || !state.ready || !no_pending {
            self.invalidate("not-clean-warm-idle");
            return;
        }
        if let Some(p) = &mut self.pending {
            p.admitted = true;
        }
    }
    pub fn ack(&mut self, ticket: u64, success: bool, state: Option<State>, now: Instant) {
        if !self.current(ticket) {
            return;
        }
        self.expire(now);
        let Some(p) = &self.pending else {
            return;
        };
        if !success || !p.admitted {
            self.invalidate("admission-or-ack-failed");
            return;
        }
        let Some(s) = state else {
            self.invalidate("ack-state-missing");
            return;
        };
        if !s.ready
            || !matches!(s.phase, Phase::Starting | Phase::Recording)
            || s.sid.is_none_or(|sid| sid == 0)
        {
            self.invalidate("ack-state-ambiguous");
            return;
        }
        if p.ack.is_some() || p.sid.is_some_and(|sid| Some(sid) != s.sid) {
            self.invalidate("ack-duplicate-or-sid-mismatch");
            return;
        }
        if let Some(p) = &mut self.pending {
            p.sid = s.sid;
            p.ack = Some(now);
        }
        self.complete();
    }
    pub fn observe(&mut self, state: Option<State>, now: Instant) {
        self.expire(now);
        let Some(p) = &self.pending else {
            return;
        };
        if !p.admitted {
            return;
        }
        let Some(s) = state else {
            self.invalidate("runtime-unavailable");
            return;
        };
        if !s.ready {
            self.invalidate("model-or-generation-changed");
            return;
        }
        if s.phase == Phase::Idle && p.ack.is_none() {
            return;
        }
        if !matches!(s.phase, Phase::Starting | Phase::Recording)
            || s.sid.is_none_or(|sid| sid == 0)
        {
            self.invalidate("start-state-ended-or-ambiguous");
            return;
        }
        if p.sid.is_some_and(|sid| Some(sid) != s.sid) {
            self.invalidate("observed-sid-mismatch");
            return;
        }
        if let Some(p) = &mut self.pending {
            p.sid = s.sid;
            if s.phase == Phase::Recording && p.recording.is_none() {
                p.recording = Some(now);
            }
        }
        self.complete();
    }
    pub fn invalidate(&mut self, reason: &'static str) {
        if let Some(p) = self.pending.take() {
            self.records.push(Record {
                ordinal: p.ordinal,
                sid: p.sid,
                upper_nanos: None,
                ack_nanos: p.ack.and_then(|at| nanos(at, p.origin)),
                reason: Some(reason),
            });
        }
    }
    pub fn expire(&mut self, now: Instant) {
        if self.pending.as_ref().is_some_and(|p| {
            now.checked_duration_since(p.origin)
                .is_none_or(|d| d >= ACK_BUDGET)
        }) {
            self.invalidate("missing-ack-or-recording-before-observation-deadline");
        }
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub fn pending_ordinal(&self) -> Option<u64> {
        self.pending.as_ref().map(|p| p.ordinal)
    }
    pub fn overflow(&self) -> u64 {
        self.overflow
    }
    pub fn counter_error(&self) -> bool {
        self.counter_error
    }
    fn current(&self, ticket: u64) -> bool {
        self.pending.as_ref().is_some_and(|p| p.ordinal == ticket)
    }
    fn complete(&mut self) {
        let Some(p) = &self.pending else {
            return;
        };
        let (Some(recording), Some(ack)) = (p.recording, p.ack) else {
            return;
        };
        let (Some(upper), Some(ack_nanos)) = (nanos(recording, p.origin), nanos(ack, p.origin))
        else {
            self.invalidate("nonmonotone-or-unrepresentable-clock");
            return;
        };
        if let Some(p) = self.pending.take() {
            self.records.push(Record {
                ordinal: p.ordinal,
                sid: p.sid,
                upper_nanos: Some(upper),
                ack_nanos: Some(ack_nanos),
                reason: None,
            });
        }
    }
}
fn nanos(at: Instant, origin: Instant) -> Option<u64> {
    u64::try_from(at.checked_duration_since(origin)?.as_nanos()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(phase: Phase, sid: Option<u64>) -> State {
        State {
            phase,
            sid,
            ready: true,
        }
    }
    #[test]
    fn recording_before_ack_is_retained_without_premature_success() -> Result<(), &'static str> {
        let t = Instant::now();
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.observe(
            Some(state(Phase::Recording, Some(7))),
            t + Duration::from_millis(180),
        );
        assert!(o.records().is_empty());
        o.ack(
            id,
            true,
            Some(state(Phase::Recording, Some(7))),
            t + Duration::from_millis(200),
        );
        assert_eq!(o.records()[0].upper_nanos, Some(180_000_000));
        assert_eq!(o.records()[0].ack_nanos, Some(200_000_000));
        Ok(())
    }
    #[test]
    fn wrong_sid_cannot_relabel_an_earlier_recording_as_this_ack() -> Result<(), &'static str> {
        let t = Instant::now();
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.observe(
            Some(state(Phase::Recording, Some(7))),
            t + Duration::from_millis(50),
        );
        o.ack(
            id,
            true,
            Some(state(Phase::Recording, Some(8))),
            t + Duration::from_millis(60),
        );
        assert!(o.records()[0].upper_nanos.is_none());
        Ok(())
    }
    #[test]
    fn ack_before_recording_and_late_foreign_ack_preserve_correct_sid() -> Result<(), &'static str>
    {
        let t = Instant::now();
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.ack(
            id,
            true,
            Some(state(Phase::Starting, Some(7))),
            t + Duration::from_millis(10),
        );
        o.observe(
            Some(state(Phase::Recording, Some(8))),
            t + Duration::from_millis(20),
        );
        assert!(o.records()[0].upper_nanos.is_none());
        let next = o.begin(t + Duration::from_millis(30)).ok_or("ticket")?;
        o.admit(next, state(Phase::Idle, None), true);
        o.ack(
            id,
            true,
            Some(state(Phase::Recording, Some(7))),
            t + Duration::from_millis(40),
        );
        assert_eq!(o.pending_ordinal(), Some(next));
        o.ack(
            next,
            true,
            Some(state(Phase::Starting, Some(9))),
            t + Duration::from_millis(50),
        );
        o.observe(
            Some(state(Phase::Recording, Some(9))),
            t + Duration::from_millis(60),
        );
        assert_eq!(o.records()[1].upper_nanos, Some(30_000_000));
        Ok(())
    }
    #[test]
    fn overlapping_start_keeps_both_failures_and_cannot_touch_commands() -> Result<(), &'static str>
    {
        let t = Instant::now();
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        assert!(o.begin(t).is_none());
        assert_eq!(o.records().len(), 2);
        assert!(
            o.records()
                .iter()
                .all(|r| r.reason == Some("overlapping-start") && r.upper_nanos.is_none())
        );
        Ok(())
    }
    #[test]
    fn interference_cancel_failure_and_changed_ready_never_become_zero() -> Result<(), &'static str>
    {
        let t = Instant::now();
        for reason in [
            "ui-start",
            "stop-or-cancel",
            "settings",
            "power",
            "shutdown",
        ] {
            let mut o = Observer::default();
            let id = o.begin(t).ok_or("ticket")?;
            o.admit(id, state(Phase::Idle, None), true);
            o.invalidate(reason);
            o.ack(id, true, Some(state(Phase::Recording, Some(1))), t);
            assert!(o.records()[0].upper_nanos.is_none());
            assert_eq!(o.records()[0].reason, Some(reason));
        }
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), false);
        assert_eq!(o.records()[0].reason, Some("not-clean-warm-idle"));
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.ack(id, false, None, t);
        assert_eq!(o.records()[1].reason, Some("admission-or-ack-failed"));
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.observe(
            Some(State {
                ready: false,
                ..state(Phase::Starting, Some(2))
            }),
            t,
        );
        assert_eq!(o.records()[2].reason, Some("model-or-generation-changed"));
        Ok(())
    }
    #[test]
    fn timeout_and_record_capacity_keep_explicit_missing_and_overflow() -> Result<(), &'static str>
    {
        let t = Instant::now();
        let mut o = Observer::default();
        let id = o.begin(t).ok_or("ticket")?;
        o.admit(id, state(Phase::Idle, None), true);
        o.expire(t + ACK_BUDGET);
        assert!(o.records()[0].upper_nanos.is_none());
        for _ in 1..MAX_RECORDS {
            let id = o.begin(t).ok_or("ticket")?;
            o.admit(id, state(Phase::Idle, None), true);
            o.ack(id, false, None, t);
        }
        assert!(o.begin(t).is_none());
        assert_eq!(o.records().len(), MAX_RECORDS);
        assert_eq!(o.overflow(), 1);
        assert!(!o.counter_error());
        Ok(())
    }
}
