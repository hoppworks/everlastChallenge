//! Absolute Mach waits behind a safe, duration-based interface.
use std::time::Duration;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WaitError {
    #[error("mach_timebase_info failed with code {0}")]
    Timebase(i32),
    #[error("invalid Mach timebase {numer}/{denom}")]
    InvalidTimebase { numer: u32, denom: u32 },
    #[error("Mach wait duration or deadline overflow")]
    Overflow,
    #[error("mach_wait_until failed with code {0}")]
    Wait(i32),
}

/// Uses public Mach APIs without changing thread policy or applying a fallback.
/// Construction queries the timebase once. A successful wait is never reported
/// before its absolute Mach deadline; scheduler lateness remains possible.
/// This clock does not advance during system sleep (unlike mach_continuous_time).
pub struct MachWaiter(Waiter<Native>);
impl MachWaiter {
    pub fn new() -> Result<Self, WaitError> {
        Waiter::new(Native).map(Self)
    }

    pub fn sleep(&self, duration: Duration) -> Result<(), WaitError> {
        self.0.sleep(duration)
    }
}

trait Backend {
    fn timebase(&self) -> Result<(u32, u32), i32>;
    fn now(&self) -> u64;
    fn wait_until(&self, deadline: u64) -> i32;
}
struct Native;
// Public SDK mach/mach_time.h: two uint32_t fields, kern_return_t and
// uint64_t absolute ticks. Keep the small ABI binding local; no new dependency.
#[repr(C)]
struct TimebaseInfo {
    numer: u32,
    denom: u32,
}
unsafe extern "C" {
    fn mach_timebase_info(info: *mut TimebaseInfo) -> libc::c_int;
    fn mach_absolute_time() -> u64;
    fn mach_wait_until(deadline: u64) -> libc::c_int;
}
impl Backend for Native {
    fn timebase(&self) -> Result<(u32, u32), i32> {
        let mut info = TimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: info is initialized, writable and lives through this synchronous
        // public API call. The OS retains no pointer to it.
        let code = unsafe { mach_timebase_info(&mut info) };
        if code == 0 {
            Ok((info.numer, info.denom))
        } else {
            Err(code)
        }
    }
    fn now(&self) -> u64 {
        // SAFETY: public clock query has no pointer or ownership requirements.
        unsafe { mach_absolute_time() }
    }
    fn wait_until(&self, deadline: u64) -> i32 {
        // SAFETY: public API accepts an absolute clock tick, not a pointer. The
        // caller remains on this thread; no borrowed resources cross the call.
        unsafe { mach_wait_until(deadline) }
    }
}
struct Waiter<B> {
    backend: B,
    numer: u32,
    denom: u32,
}
impl<B: Backend> Waiter<B> {
    fn new(backend: B) -> Result<Self, WaitError> {
        let (numer, denom) = backend.timebase().map_err(WaitError::Timebase)?;
        if numer == 0 || denom == 0 {
            return Err(WaitError::InvalidTimebase { numer, denom });
        }
        Ok(Self {
            backend,
            numer,
            denom,
        })
    }
    fn sleep(&self, duration: Duration) -> Result<(), WaitError> {
        if duration.is_zero() {
            return Ok(());
        }
        let scaled = duration
            .as_nanos()
            .checked_mul(u128::from(self.denom))
            .ok_or(WaitError::Overflow)?;
        let numer = u128::from(self.numer);
        let ticks = (scaled / numer)
            .checked_add(u128::from(scaled % numer != 0))
            .ok_or(WaitError::Overflow)?;
        let ticks = u64::try_from(ticks).map_err(|_| WaitError::Overflow)?;
        let deadline = self
            .backend
            .now()
            .checked_add(ticks)
            .ok_or(WaitError::Overflow)?;
        loop {
            let code = self.backend.wait_until(deadline);
            if code != 0 {
                return Err(WaitError::Wait(code));
            }
            if self.backend.now() >= deadline {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
    };
    struct Fake {
        base: Result<(u32, u32), i32>,
        queries: Cell<usize>,
        now: Cell<u64>,
        waits: RefCell<Vec<u64>>,
        returns: RefCell<VecDeque<(i32, u64)>>,
    }
    impl Backend for Fake {
        fn timebase(&self) -> Result<(u32, u32), i32> {
            self.queries.set(self.queries.get() + 1);
            self.base
        }
        fn now(&self) -> u64 {
            self.now.get()
        }
        fn wait_until(&self, deadline: u64) -> i32 {
            self.waits.borrow_mut().push(deadline);
            let (code, now) = self
                .returns
                .borrow_mut()
                .pop_front()
                .unwrap_or((0, deadline));
            self.now.set(now);
            code
        }
    }
    fn fake(numer: u32, denom: u32, now: u64) -> Fake {
        Fake {
            base: Ok((numer, denom)),
            queries: Cell::new(0),
            now: Cell::new(now),
            waits: RefCell::new(Vec::new()),
            returns: RefCell::new(VecDeque::new()),
        }
    }
    #[test]
    fn durations_round_up_to_absolute_ticks_and_reuse_one_timebase() -> Result<(), WaitError> {
        // At 125/3 nanoseconds per tick: 1ns requires 1 tick, 1000ns 24 ticks.
        let waiter = Waiter::new(fake(125, 3, 100))?;
        waiter.sleep(Duration::from_nanos(1))?;
        waiter.sleep(Duration::from_micros(1))?;
        assert_eq!(*waiter.backend.waits.borrow(), [101, 125]);
        assert_eq!(waiter.backend.queries.get(), 1);
        Ok(())
    }
    #[test]
    fn invalid_timebases_and_query_errors_are_rejected_at_construction() {
        for (numer, denom) in [(0, 1), (1, 0), (0, 0)] {
            assert!(matches!(Waiter::new(fake(numer, denom, 0)),
                Err(WaitError::InvalidTimebase { numer: n, denom: d }) if n == numer && d == denom));
        }
        let mut backend = fake(1, 1, 0);
        backend.base = Err(17);
        assert!(matches!(Waiter::new(backend), Err(WaitError::Timebase(17))));
    }

    #[test]
    fn early_success_waits_again_for_the_same_absolute_deadline() -> Result<(), WaitError> {
        let waiter = Waiter::new(fake(1, 1, 100))?;
        waiter
            .backend
            .returns
            .borrow_mut()
            .extend([(0, 105), (0, 119), (0, 120)]);
        waiter.sleep(Duration::from_nanos(20))?;
        assert!(waiter.backend.now.get() >= 120);
        assert_eq!(*waiter.backend.waits.borrow(), [120, 120, 120]);
        Ok(())
    }

    #[test]
    fn os_wait_errors_are_preserved_without_fallback_or_retry() -> Result<(), WaitError> {
        let waiter = Waiter::new(fake(1, 1, 100))?;
        waiter
            .backend
            .returns
            .borrow_mut()
            .extend([(0, 101), (14, 102)]);
        assert_eq!(
            waiter.sleep(Duration::from_nanos(20)),
            Err(WaitError::Wait(14))
        );
        assert_eq!(*waiter.backend.waits.borrow(), [120, 120]);
        Ok(())
    }
    #[test]
    fn unrepresentable_duration_or_absolute_deadline_never_reaches_wait() -> Result<(), WaitError> {
        for (backend, duration) in [
            (fake(1, u32::MAX, 0), Duration::MAX),
            (fake(1, 1, u64::MAX), Duration::from_nanos(1)),
        ] {
            let waiter = Waiter::new(backend)?;
            assert_eq!(waiter.sleep(duration), Err(WaitError::Overflow));
            assert!(waiter.backend.waits.borrow().is_empty());
        }
        let waiter = Waiter::new(fake(u32::MAX, u32::MAX, 0))?;
        waiter.sleep(Duration::from_nanos(u64::MAX))?;
        assert_eq!(*waiter.backend.waits.borrow(), [u64::MAX]);
        Ok(())
    }
    #[test]
    fn zero_duration_does_not_call_the_wait_backend() -> Result<(), WaitError> {
        let waiter = Waiter::new(fake(1, 1, 100))?;
        waiter.backend.returns.borrow_mut().push_back((14, 100));
        waiter.sleep(Duration::ZERO)?;
        assert!(waiter.backend.waits.borrow().is_empty());
        Ok(())
    }
}
