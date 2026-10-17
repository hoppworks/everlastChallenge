//! Developer-only time-constraint policy on a newly owned publisher thread.
//! No policy setter is exposed for the caller's thread; no QoS restore is attempted.
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublisherPolicy {
    UserInteractive,
    TimeConstrained,
}

pub const PERIOD: Duration = Duration::from_millis(20);
pub const COMPUTATION: Duration = Duration::from_micros(500);
pub const CONSTRAINT: Duration = Duration::from_millis(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeConstraint {
    pub period_ticks: u32,
    pub computation_ticks: u32,
    pub constraint_ticks: u32,
    pub preemptible: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadCpuTime {
    pub user: Duration,
    pub system: Duration,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub time_constraint: TimeConstraint,
    pub default_policy: bool,
    /// Raw public THREAD_BASIC_INFO.policy, independent of stored RT parameters.
    /// A time-constraint readback alone cannot establish absence of demotion.
    pub basic_policy: i32,
    pub cpu: ThreadCpuTime,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("{operation} failed with OS code {code}")]
    Os { operation: &'static str, code: i32 },
    #[error("invalid Mach timebase {numer}/{denom}")]
    InvalidTimebase { numer: u32, denom: u32 },
    #[error("time-constraint ticks exceed the public u32 ABI")]
    TickOverflow,
    #[error("timeshare policy observed despite time-constraint request")]
    TimeshareObserved,
    #[error("unverified basic scheduling policy {policy} after time-constraint request")]
    UnverifiedBasicPolicy { policy: i32 },
    #[error("{operation} returned an invalid or unsupported reply")]
    InvalidReply { operation: &'static str },
    #[error("configured time-constraint policy differs from the requested policy")]
    ConfiguredMismatch {
        requested: TimeConstraint,
        observed: TimeConstraint,
        default_policy: bool,
    },
}
#[derive(Debug)]
pub struct PublisherReport<T> {
    pub policy: PublisherPolicy,
    pub output: Option<T>,
    pub requested: Option<TimeConstraint>,
    pub before: Option<PolicySnapshot>,
    pub configured: Option<PolicySnapshot>,
    pub after: Option<PolicySnapshot>,
    pub setup_error: Option<PolicyError>,
    pub post_error: Option<PolicyError>,
    pub finish_error: Option<PolicyError>,
}
impl<T> PublisherReport<T> {
    fn empty(policy: PublisherPolicy) -> Self {
        Self {
            policy,
            output: None,
            requested: None,
            before: None,
            configured: None,
            after: None,
            setup_error: None,
            post_error: None,
            finish_error: None,
        }
    }
}

/// Borrowed same-thread observer. Fields are sequential OS readbacks, not an
/// atomic snapshot, scheduling guarantee or continuous demotion trace. The
/// caller chooses bounded samples.
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<macos_integration::realtime::PolicyObserver<'static>>();
/// ```
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<macos_integration::realtime::PolicyObserver<'static>>();
/// ```
pub struct PolicyObserver<'a> {
    backend: &'a dyn Backend,
    _same_thread: PhantomData<Rc<()>>,
}
impl PolicyObserver<'_> {
    pub fn snapshot(&self) -> Result<PolicySnapshot, PolicyError> {
        self.backend.snapshot()
    }
}

/// Applies the selected experimental policy only inside a newly created thread.
/// The callback runs only after successful setup/readback. Its output survives
/// post-callback readback failure. Join must be observed to confirm thread exit;
/// callback panics remain ordinary JoinHandle errors. Create normal DSP/audio
/// workers outside this callback so they do not inherit its scheduling policy.
/// No native policy test is
/// performed implicitly by constructing or importing this module.
pub fn spawn_publisher<F, T>(
    policy: PublisherPolicy,
    action: F,
) -> std::io::Result<JoinHandle<PublisherReport<T>>>
where
    F: FnOnce(&PolicyObserver<'_>) -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_with(policy, Native::current, action)
}
trait Backend {
    fn timebase(&self) -> Result<(u32, u32), PolicyError>;
    fn snapshot(&self) -> Result<PolicySnapshot, PolicyError>;
    fn set(&self, policy: TimeConstraint) -> Result<(), PolicyError>;
    fn enter_qos(&self) -> Result<(), PolicyError>;
    fn finish_qos(&self) -> Result<(), PolicyError>;
}
fn spawn_with<B, M, F, T>(
    policy: PublisherPolicy,
    make: M,
    action: F,
) -> std::io::Result<JoinHandle<PublisherReport<T>>>
where
    B: Backend + 'static,
    M: FnOnce() -> Result<B, PolicyError> + Send + 'static,
    F: FnOnce(&PolicyObserver<'_>) -> T + Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name("deadline-publisher".into())
        .spawn(move || {
            let mut report = PublisherReport::empty(policy);
            match make() {
                Ok(backend) => {
                    let mut qos_entered = false;
                    let setup = (|| -> Result<(), PolicyError> {
                        report.before = Some(backend.snapshot()?);
                        if policy == PublisherPolicy::TimeConstrained {
                            let (numer, denom) = backend.timebase()?;
                            let requested = policy_ticks(numer, denom)?;
                            report.requested = Some(requested);
                            backend.set(requested)?;
                            let configured = backend.snapshot()?;
                            report.configured = Some(configured);
                            if configured.default_policy || configured.time_constraint != requested
                            {
                                return Err(PolicyError::ConfiguredMismatch {
                                    requested,
                                    observed: configured.time_constraint,
                                    default_policy: configured.default_policy,
                                });
                            }
                            match configured.basic_policy {
                                2 => {}
                                1 => return Err(PolicyError::TimeshareObserved),
                                policy => {
                                    return Err(PolicyError::UnverifiedBasicPolicy { policy });
                                }
                            }
                        } else {
                            backend.enter_qos()?;
                            qos_entered = true;
                            report.configured = Some(backend.snapshot()?);
                        }
                        Ok(())
                    })();
                    match setup {
                        Ok(()) => {
                            report.output = Some(action(&PolicyObserver {
                                backend: &backend,
                                _same_thread: PhantomData,
                            }))
                        }
                        Err(error) => report.setup_error = Some(error),
                    }
                    if qos_entered {
                        report.finish_error = backend.finish_qos().err();
                    }
                    match backend.snapshot() {
                        Ok(snapshot) => report.after = Some(snapshot),
                        Err(error) => report.post_error = Some(error),
                    }
                }
                Err(error) => report.setup_error = Some(error),
            }
            report
        })
}
fn policy_ticks(numer: u32, denom: u32) -> Result<TimeConstraint, PolicyError> {
    if numer == 0 || denom == 0 {
        return Err(PolicyError::InvalidTimebase { numer, denom });
    }
    let ticks = |duration: Duration| -> Result<u32, PolicyError> {
        let scaled = duration
            .as_nanos()
            .checked_mul(u128::from(denom))
            .ok_or(PolicyError::TickOverflow)?;
        let divisor = u128::from(numer);
        let rounded = (scaled / divisor)
            .checked_add(u128::from(scaled % divisor != 0))
            .ok_or(PolicyError::TickOverflow)?;
        u32::try_from(rounded).map_err(|_| PolicyError::TickOverflow)
    };
    Ok(TimeConstraint {
        period_ticks: ticks(PERIOD)?,
        computation_ticks: ticks(COMPUTATION)?,
        constraint_ticks: ticks(CONSTRAINT)?,
        preemptible: true,
    })
}

// ABI checked against public SDK mach/thread_policy.h, thread_info.h,
// thread_act.h, mach_time.h and pthread.h. All fields are 32-bit integers.
const TC_FLAVOR: i32 = 2;
const BASIC_FLAVOR: i32 = 3;
const TC_COUNT: u32 = 4;
const BASIC_COUNT: u32 = 10;
#[repr(C)]
struct TimebaseInfo {
    numer: u32,
    denom: u32,
}
#[link(name = "System")]
unsafe extern "C" {
    fn mach_timebase_info(info: *mut TimebaseInfo) -> i32;
    fn thread_policy_set(thread: u32, flavor: i32, info: *mut i32, count: u32) -> i32;
    fn thread_policy_get(
        thread: u32,
        flavor: i32,
        info: *mut i32,
        count: *mut u32,
        default_policy: *mut i32,
    ) -> i32;
    fn thread_info(thread: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
}
struct Native {
    // Borrowed from the current pthread, valid only until this owned child exits.
    // pthread_mach_thread_np does not create an owned mach_thread_self send right.
    port: u32,
    qos: RefCell<Option<crate::qos::PublisherQos>>,
    _same_thread: PhantomData<Rc<()>>,
}
impl Native {
    fn current() -> Result<Self, PolicyError> {
        // SAFETY: pthread_self is the live executing child. The returned port is
        // borrowed, never deallocated or transferred to another thread here.
        let port = unsafe { libc::pthread_mach_thread_np(libc::pthread_self()) };
        if port == 0 || port == u32::MAX {
            return Err(PolicyError::InvalidReply {
                operation: "pthread_mach_thread_np",
            });
        }
        Ok(Self {
            port,
            qos: RefCell::new(None),
            _same_thread: PhantomData,
        })
    }
}
fn os_result(operation: &'static str, code: i32) -> Result<(), PolicyError> {
    if code == 0 {
        Ok(())
    } else {
        Err(PolicyError::Os { operation, code })
    }
}
fn cpu_time(seconds: i32, micros: i32) -> Result<Duration, PolicyError> {
    if seconds < 0 || !(0..1_000_000).contains(&micros) {
        return Err(PolicyError::InvalidReply {
            operation: "THREAD_BASIC_INFO CPU time",
        });
    }
    Duration::from_secs(seconds as u64)
        .checked_add(Duration::from_micros(micros as u64))
        .ok_or(PolicyError::InvalidReply {
            operation: "THREAD_BASIC_INFO CPU time",
        })
}
fn parse_snapshot(
    policy: [i32; 4],
    default_policy: i32,
    basic: [i32; 10],
) -> Result<PolicySnapshot, PolicyError> {
    if ![0, 1].contains(&default_policy) || ![0, 1].contains(&policy[3]) {
        return Err(PolicyError::InvalidReply {
            operation: "thread_policy_get boolean",
        });
    }
    Ok(PolicySnapshot {
        time_constraint: TimeConstraint {
            period_ticks: policy[0] as u32,
            computation_ticks: policy[1] as u32,
            constraint_ticks: policy[2] as u32,
            preemptible: policy[3] != 0,
        },
        default_policy: default_policy != 0,
        basic_policy: basic[5],
        cpu: ThreadCpuTime {
            user: cpu_time(basic[0], basic[1])?,
            system: cpu_time(basic[2], basic[3])?,
        },
    })
}
impl Backend for Native {
    fn timebase(&self) -> Result<(u32, u32), PolicyError> {
        let mut info = TimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: initialized writable repr(C) SDK storage lasts through this
        // synchronous call; no pointer is retained by the OS.
        let code = unsafe { mach_timebase_info(&mut info) };
        os_result("mach_timebase_info", code)?;
        Ok((info.numer, info.denom))
    }
    fn snapshot(&self) -> Result<PolicySnapshot, PolicyError> {
        let mut policy = [0_i32; 4];
        let mut count = TC_COUNT;
        let mut default_policy = 0;
        // SAFETY: the borrowed live thread port is queried on its own thread.
        // policy has exactly TC_COUNT integer_t slots, both out pointers are
        // initialized and valid, and the synchronous OS call retains none.
        let code = unsafe {
            thread_policy_get(
                self.port,
                TC_FLAVOR,
                policy.as_mut_ptr(),
                &mut count,
                &mut default_policy,
            )
        };
        os_result("thread_policy_get", code)?;
        if count != TC_COUNT {
            return Err(PolicyError::InvalidReply {
                operation: "thread_policy_get count",
            });
        }
        let mut basic = [0_i32; 10];
        let mut count = BASIC_COUNT;
        // SAFETY: basic provides the SDK's ten integer_t slots; count is valid
        // writable storage and the borrowed target thread remains alive here.
        let code = unsafe { thread_info(self.port, BASIC_FLAVOR, basic.as_mut_ptr(), &mut count) };
        os_result("thread_info THREAD_BASIC_INFO", code)?;
        if count != BASIC_COUNT {
            return Err(PolicyError::InvalidReply {
                operation: "thread_info count",
            });
        }
        parse_snapshot(policy, default_policy, basic)
    }
    fn set(&self, policy: TimeConstraint) -> Result<(), PolicyError> {
        let mut raw = [
            policy.period_ticks as i32,
            policy.computation_ticks as i32,
            policy.constraint_ticks as i32,
            i32::from(policy.preemptible),
        ];
        // SAFETY: only this newly created child thread's borrowed live port is
        // targeted. Four integer_t words match the public time-constraint ABI;
        // unsigned tick values preserve their bit pattern in integer_t storage.
        let code = unsafe { thread_policy_set(self.port, TC_FLAVOR, raw.as_mut_ptr(), TC_COUNT) };
        os_result("thread_policy_set", code)
    }
    fn enter_qos(&self) -> Result<(), PolicyError> {
        let scope = crate::qos::PublisherQos::enter().map_err(|error| match error {
            crate::qos::QosError::Unavailable => PolicyError::InvalidReply {
                operation: "PublisherQos enter",
            },
            crate::qos::QosError::Finish(code) => PolicyError::Os {
                operation: "PublisherQos enter",
                code,
            },
        })?;
        *self.qos.borrow_mut() = Some(scope);
        Ok(())
    }
    fn finish_qos(&self) -> Result<(), PolicyError> {
        let scope = self
            .qos
            .borrow_mut()
            .take()
            .ok_or(PolicyError::InvalidReply {
                operation: "PublisherQos scope missing",
            })?;
        scope.finish().map_err(|error| match error {
            crate::qos::QosError::Finish(code) => PolicyError::Os {
                operation: "PublisherQos finish",
                code,
            },
            crate::qos::QosError::Unavailable => PolicyError::InvalidReply {
                operation: "PublisherQos finish",
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[test]
    fn fixed_durations_convert_to_u32_ticks_with_ceiling_and_reject_invalid_timebases()
    -> Result<(), PolicyError> {
        assert_eq!(
            policy_ticks(125, 3)?,
            TimeConstraint {
                period_ticks: 480_000,
                computation_ticks: 12_000,
                constraint_ticks: 24_000,
                preemptible: true
            }
        );
        // Coarse illustrative timebase: 3ns per tick, so 500000ns rounds upward.
        assert_eq!(policy_ticks(3, 1)?.computation_ticks, 166_667);
        for (numer, denom) in [(0, 1), (1, 0), (0, 0)] {
            assert_eq!(
                policy_ticks(numer, denom),
                Err(PolicyError::InvalidTimebase { numer, denom })
            );
        }
        assert_eq!(policy_ticks(1, u32::MAX), Err(PolicyError::TickOverflow));
        Ok(())
    }
    #[derive(Default)]
    struct State {
        events: Vec<(thread::ThreadId, &'static str)>,
        requested: Option<TimeConstraint>,
        reads: usize,
        fail_read: Option<usize>,
        fail_set: bool,
        fail_finish: bool,
        basic_override: Option<i32>,
        mismatch: bool,
    }
    struct Fake(Arc<Mutex<State>>);
    impl Fake {
        fn event(&self, name: &'static str) -> std::sync::MutexGuard<'_, State> {
            let mut state = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.events.push((thread::current().id(), name));
            state
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            drop(self.event("drop"));
        }
    }
    fn error(operation: &'static str) -> PolicyError {
        PolicyError::Os {
            operation,
            code: 17,
        }
    }
    impl Backend for Fake {
        fn timebase(&self) -> Result<(u32, u32), PolicyError> {
            drop(self.event("timebase"));
            Ok((125, 3))
        }
        fn set(&self, policy: TimeConstraint) -> Result<(), PolicyError> {
            let mut state = self.event("set");
            if state.fail_set {
                return Err(error("set"));
            }
            state.requested = Some(policy);
            Ok(())
        }
        fn snapshot(&self) -> Result<PolicySnapshot, PolicyError> {
            let mut state = self.event("snapshot");
            state.reads += 1;
            if state.fail_read == Some(state.reads) {
                return Err(error("snapshot"));
            }
            let mut constraint = state.requested.unwrap_or(TimeConstraint {
                period_ticks: 0,
                computation_ticks: 0,
                constraint_ticks: 0,
                preemptible: false,
            });
            if state.mismatch && state.requested.is_some() {
                constraint.constraint_ticks += 1;
            }
            Ok(PolicySnapshot {
                time_constraint: constraint,
                default_policy: state.requested.is_none(),
                basic_policy: state
                    .basic_override
                    .unwrap_or(if state.requested.is_some() { 2 } else { 1 }),
                cpu: ThreadCpuTime {
                    user: Duration::from_micros(state.reads as u64),
                    system: Duration::ZERO,
                },
            })
        }
        fn enter_qos(&self) -> Result<(), PolicyError> {
            drop(self.event("enter qos"));
            Ok(())
        }
        fn finish_qos(&self) -> Result<(), PolicyError> {
            let state = self.event("finish qos");
            if state.fail_finish {
                Err(error("finish qos"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn rt_setup_and_observation_are_confined_to_the_owned_child_before_callback()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = thread::current().id();
        let state = Arc::new(Mutex::new(State::default()));
        let child = Arc::clone(&state);
        let handle = spawn_with(
            PublisherPolicy::TimeConstrained,
            move || Ok(Fake(child)),
            move |observer| observer.snapshot().map(|snapshot| snapshot.basic_policy),
        )?;
        let report = handle.join().map_err(|_| "child panicked")?;
        assert_eq!(report.output, Some(Ok(2)));
        assert!(report.setup_error.is_none());
        assert!(report.post_error.is_none());
        let state = state.lock().map_err(|_| "state poisoned")?;
        assert!(state.events.iter().all(|(thread, _)| *thread != parent));
        assert_eq!(
            state
                .events
                .iter()
                .map(|(_, name)| *name)
                .collect::<Vec<_>>(),
            [
                "snapshot", "timebase", "set", "snapshot", "snapshot", "snapshot", "drop"
            ]
        );
        assert_eq!(
            report.configured.map(|snapshot| snapshot.basic_policy),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn baseline_qos_finishes_before_after_readback_and_both_errors_preserve_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = Arc::new(Mutex::new(State {
            fail_finish: true,
            fail_read: Some(4),
            ..State::default()
        }));
        let child = Arc::clone(&state);
        let report = spawn_with(
            PublisherPolicy::UserInteractive,
            move || Ok(Fake(child)),
            |observer| {
                observer
                    .snapshot()
                    .map(|snapshot| snapshot.cpu.user.as_micros())
            },
        )?
        .join()
        .map_err(|_| "child panicked")?;
        assert_eq!(report.output, Some(Ok(3)));
        assert_eq!(report.finish_error, Some(error("finish qos")));
        assert_eq!(report.post_error, Some(error("snapshot")));
        assert!(report.setup_error.is_none());
        assert!(report.requested.is_none());
        let state = state.lock().map_err(|_| "state poisoned")?;
        assert_eq!(
            state
                .events
                .iter()
                .map(|(_, name)| *name)
                .collect::<Vec<_>>(),
            [
                "snapshot",
                "enter qos",
                "snapshot",
                "snapshot",
                "finish qos",
                "snapshot",
                "drop"
            ]
        );
        assert!(state.requested.is_none());
        Ok(())
    }

    #[test]
    fn stored_rt_parameters_do_not_admit_timeshare_or_unverified_basic_policy()
    -> Result<(), Box<dyn std::error::Error>> {
        for (basic, expected) in [
            (1, PolicyError::TimeshareObserved),
            (4, PolicyError::UnverifiedBasicPolicy { policy: 4 }),
            (99, PolicyError::UnverifiedBasicPolicy { policy: 99 }),
        ] {
            let state = Arc::new(Mutex::new(State {
                basic_override: Some(basic),
                ..State::default()
            }));
            let report = spawn_with(
                PublisherPolicy::TimeConstrained,
                move || Ok(Fake(state)),
                |_| 77,
            )?
            .join()
            .map_err(|_| "child panicked")?;
            assert!(report.output.is_none());
            assert_eq!(report.setup_error, Some(expected));
            assert_eq!(
                report.configured.map(|snapshot| snapshot.basic_policy),
                Some(basic)
            );
        }
        Ok(())
    }

    #[test]
    fn setup_os_errors_and_changed_readback_never_run_the_callback()
    -> Result<(), Box<dyn std::error::Error>> {
        for (state, expected) in [
            (
                State {
                    fail_read: Some(1),
                    ..State::default()
                },
                error("snapshot"),
            ),
            (
                State {
                    fail_set: true,
                    ..State::default()
                },
                error("set"),
            ),
            (
                State {
                    fail_read: Some(2),
                    ..State::default()
                },
                error("snapshot"),
            ),
        ] {
            let child = Arc::new(Mutex::new(state));
            let report = spawn_with(
                PublisherPolicy::TimeConstrained,
                move || Ok(Fake(child)),
                |_| 77,
            )?
            .join()
            .map_err(|_| "child panicked")?;
            assert!(report.output.is_none());
            assert_eq!(report.setup_error, Some(expected));
        }
        let child = Arc::new(Mutex::new(State {
            mismatch: true,
            ..State::default()
        }));
        let report = spawn_with(
            PublisherPolicy::TimeConstrained,
            move || Ok(Fake(child)),
            |_| 77,
        )?
        .join()
        .map_err(|_| "child panicked")?;
        assert!(report.output.is_none());
        assert!(matches!(
            report.setup_error,
            Some(PolicyError::ConfiguredMismatch { .. })
        ));
        assert!(report.configured.is_some());
        Ok(())
    }
    #[test]
    fn rt_post_readback_error_keeps_callback_output_and_the_joined_owner_is_dropped()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = Arc::new(Mutex::new(State {
            fail_read: Some(3),
            ..State::default()
        }));
        let child = Arc::clone(&state);
        let report = spawn_with(
            PublisherPolicy::TimeConstrained,
            move || Ok(Fake(child)),
            |_| vec![11, 22, 33],
        )?
        .join()
        .map_err(|_| "child panicked")?;
        assert_eq!(report.output, Some(vec![11, 22, 33]));
        assert_eq!(report.post_error, Some(error("snapshot")));
        assert!(report.setup_error.is_none());
        assert_eq!(
            state
                .lock()
                .map_err(|_| "state poisoned")?
                .events
                .last()
                .map(|(_, name)| *name),
            Some("drop")
        );
        Ok(())
    }
    #[test]
    fn callback_unwind_remains_a_join_error_and_releases_the_child_backend()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = Arc::new(Mutex::new(State::default()));
        let child = Arc::clone(&state);
        let handle = spawn_with(
            PublisherPolicy::UserInteractive,
            move || Ok(Fake(child)),
            |_| std::panic::resume_unwind(Box::new("controlled callback unwind")),
        )?;
        assert!(handle.join().is_err());
        assert_eq!(
            state
                .lock()
                .map_err(|_| "state poisoned")?
                .events
                .last()
                .map(|(_, name)| *name),
            Some("drop")
        );
        Ok(())
    }
    #[test]
    fn native_reply_layout_retains_unsigned_ticks_raw_basic_policy_and_bounded_cpu_times()
    -> Result<(), PolicyError> {
        let policy = [-1, 12_000, 24_000, 1];
        let basic = [2, 3, 4, 5, 99, 2, 0, 0, 0, 0];
        let snapshot = parse_snapshot(policy, 0, basic)?;
        assert_eq!(snapshot.time_constraint.period_ticks, u32::MAX);
        assert_eq!(snapshot.basic_policy, 2);
        assert_eq!(snapshot.cpu.user, Duration::new(2, 3000));
        assert_eq!(snapshot.cpu.system, Duration::new(4, 5000));
        for (seconds, micros) in [(-1, 0), (0, -1), (0, 1_000_000)] {
            assert!(cpu_time(seconds, micros).is_err());
        }
        assert!(parse_snapshot(policy, 2, basic).is_err());
        assert!(parse_snapshot([0, 0, 0, 2], 0, basic).is_err());
        Ok(())
    }
}
