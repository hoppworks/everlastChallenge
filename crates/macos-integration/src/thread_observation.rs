//! Read-only current-thread diagnostics. No scheduling changes or port ownership.
use std::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadSnapshot {
    /// Public pthread QoS class as a raw value, not an inferred scheduling guarantee.
    pub qos_class: u32,
    pub relative_priority: i32,
    pub basic_policy: i32,
    pub user_cpu: Duration,
    pub system_cpu: Duration,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationError {
    Os { operation: &'static str, code: i32 },
    InvalidReply { operation: &'static str },
    UnsupportedPlatform,
}
impl fmt::Display for ObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ObservationError {}

fn parse(
    qos_class: u32,
    relative_priority: i32,
    basic: [i32; 10],
) -> Result<ThreadSnapshot, ObservationError> {
    let cpu = |seconds: i32, micros: i32| {
        if seconds < 0 || !(0..1_000_000).contains(&micros) {
            return Err(ObservationError::InvalidReply {
                operation: "THREAD_BASIC_INFO CPU time",
            });
        }
        Ok(Duration::from_secs(seconds as u64) + Duration::from_micros(micros as u64))
    };
    Ok(ThreadSnapshot {
        qos_class,
        relative_priority,
        basic_policy: basic[5],
        user_cpu: cpu(basic[0], basic[1])?,
        system_cpu: cpu(basic[2], basic[3])?,
    })
}

/// Sequential public OS queries of the executing thread. QoS and CPU are not an
/// atomic snapshot or a continuous policy trace. The caller establishes whether
/// this is the actual inference worker or merely a separate diagnostic caller.
#[cfg(target_os = "macos")]
pub fn current() -> Result<ThreadSnapshot, ObservationError> {
    unsafe extern "C" {
        fn pthread_get_qos_class_np(
            thread: libc::pthread_t,
            class: *mut u32,
            relative_priority: *mut i32,
        ) -> i32;
        fn thread_info(thread: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    }
    // SAFETY: the current pthread is live throughout synchronous read-only calls.
    let thread = unsafe { libc::pthread_self() };
    let mut class = 0;
    let mut relative_priority = 0;
    // SAFETY: initialized writable ABI-sized output storage remains valid; the
    // public getter retains no pointers and changes no scheduling settings.
    let code = unsafe { pthread_get_qos_class_np(thread, &mut class, &mut relative_priority) };
    if code != 0 {
        return Err(ObservationError::Os {
            operation: "pthread_get_qos_class_np",
            code,
        });
    }
    // SAFETY: returns a borrowed current-thread port, never an owned send right.
    let port = unsafe { libc::pthread_mach_thread_np(thread) };
    if port == 0 || port == u32::MAX {
        return Err(ObservationError::InvalidReply {
            operation: "pthread_mach_thread_np",
        });
    }
    let mut basic = [0_i32; 10];
    let mut count = 10;
    // SAFETY: THREAD_BASIC_INFO flavor3 requires ten integer_t slots. The
    // borrowed target thread and all writable storage stay live for the call.
    let code = unsafe { thread_info(port, 3, basic.as_mut_ptr(), &mut count) };
    if code != 0 {
        return Err(ObservationError::Os {
            operation: "thread_info",
            code,
        });
    }
    if count != 10 {
        return Err(ObservationError::InvalidReply {
            operation: "thread_info count",
        });
    }
    parse(class, relative_priority, basic)
}
#[cfg(not(target_os = "macos"))]
pub fn current() -> Result<ThreadSnapshot, ObservationError> {
    Err(ObservationError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_fields_are_checked_and_raw_qos_policy_are_preserved() -> Result<(), ObservationError> {
        let snapshot = parse(0, -3, [2, 3, 4, 999_999, 0, 7, 0, 0, 0, 0])?;
        assert_eq!(snapshot.qos_class, 0);
        assert_eq!(snapshot.basic_policy, 7);
        assert_eq!(snapshot.relative_priority, -3);
        assert_eq!(
            snapshot.user_cpu,
            Duration::from_secs(2) + Duration::from_micros(3)
        );
        for (index, value) in [(0, -1), (1, -1), (1, 1_000_000), (2, -1), (3, 1_000_000)] {
            let mut basic = [0; 10];
            basic[index] = value;
            assert!(parse(0, 0, basic).is_err());
        }
        Ok(())
    }
}
