//! Absolute-time developer replay feed; independent of consumers and Stop.

use super::{
    ReplayInput,
    schedule::{PublicationTiming, ReplaySchedule, ScheduleError, observe},
};
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};

pub const MAXIMUM_SLEEP: Duration = Duration::from_millis(1);
pub const FINAL_SPIN: Duration = Duration::from_micros(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    Schedule(ScheduleError),
    PartialFrame,
    ClockRange,
}
impl std::fmt::Display for FeedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schedule(error) => error.fmt(formatter),
            Self::PartialFrame => {
                formatter.write_str("Replay input contains a partial interleaved frame")
            }
            Self::ClockRange => {
                formatter.write_str("Replay end time exceeds the monotonic clock range")
            }
        }
    }
}
impl std::error::Error for FeedError {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    pub start_frame: usize,
    pub end_frame: usize,
    pub planned_nanos: u128,
    pub before_publish_nanos: u128,
    pub after_publish_nanos: u128,
    pub deviation_nanos: u128,
    pub timing: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<ChunkWaitEvidence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEvidence {
    pub publications: Vec<Publication>,
    pub timing_valid: bool,
    pub planned_stop_offset: Duration,
    pub wait_summary: WaitSummary,
}

/// Phase-local wall-clock evidence; not proof of an OS scheduling cause.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepEvidence {
    pub before_nanos: u128,
    pub requested_nanos: u128,
    pub after_nanos: u128,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitEvidence {
    pub enter_nanos: u128,
    pub exit_nanos: u128,
    pub last_sleep: Option<SleepEvidence>,
    pub max_sleep_overshoot_nanos: u128,
    pub spin_entry_nanos: Option<u128>,
    pub max_spin_clock_gap_nanos: u128,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkWaitEvidence {
    pub reserve_enter_nanos: u128,
    pub reserve_exit_nanos: u128,
    pub initial_window_wait: Option<WaitEvidence>,
    pub deadline_wait: WaitEvidence,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitSummary {
    pub observed_publications: usize,
    pub detailed_publications: usize,
    pub max_reserve_nanos: u128,
    pub max_sleep_overshoot_nanos: u128,
    pub max_spin_clock_gap_nanos: u128,
    pub max_wait_exit_to_publish_nanos: u128,
}

trait WaitBackend {
    fn now(&mut self) -> Instant;
    fn sleep(&mut self, duration: Duration);
    fn spin_hint(&mut self);
    fn before_reserve(&mut self) {}
}
struct SystemClock;
impl WaitBackend for SystemClock {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
    fn spin_hint(&mut self) {
        std::hint::spin_loop();
    }
}

impl WaitSummary {
    fn record(&mut self, detail: &ChunkWaitEvidence, before_publish_nanos: u128, invalid: bool) {
        self.observed_publications += 1;
        self.detailed_publications += usize::from(invalid);
        self.max_reserve_nanos = self.max_reserve_nanos.max(
            detail
                .reserve_exit_nanos
                .saturating_sub(detail.reserve_enter_nanos),
        );
        self.max_wait_exit_to_publish_nanos = self
            .max_wait_exit_to_publish_nanos
            .max(before_publish_nanos.saturating_sub(detail.deadline_wait.exit_nanos));
        for wait in detail
            .initial_window_wait
            .iter()
            .chain(std::iter::once(&detail.deadline_wait))
        {
            self.max_sleep_overshoot_nanos = self
                .max_sleep_overshoot_nanos
                .max(wait.max_sleep_overshoot_nanos);
            self.max_spin_clock_gap_nanos = self
                .max_spin_clock_gap_nanos
                .max(wait.max_spin_clock_gap_nanos);
        }
    }
}
fn offset(clock: Instant, origin: Instant) -> u128 {
    clock.saturating_duration_since(origin).as_nanos()
}

#[cfg(test)]
std::thread_local! {
    static WAIT_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) fn on_next_wait(hook: impl FnOnce() + 'static) {
    WAIT_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

/// Short sleeps avoid relying on one coalescible 20-ms OS timer. The final
/// spin is entered only with <=250 us remaining; it never waits for a consumer.
/// Wall-clock gaps include scheduling/interrupt delays and are not CPU-time proof.
fn wait_until(target: Instant, origin: Instant, clock: &mut impl WaitBackend) -> WaitEvidence {
    #[cfg(test)]
    WAIT_HOOK.with(|slot| {
        let hook = slot.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
    });
    let mut evidence = WaitEvidence {
        enter_nanos: offset(clock.now(), origin),
        ..WaitEvidence::default()
    };
    let mut previous_spin_tick: Option<Instant> = None;
    loop {
        // Preserve the original fresh monotone-time decision on every iteration;
        // diagnostic bookkeeping must not reuse a stale remaining-time sample.
        let current = clock.now();
        if let Some(previous) = previous_spin_tick.take() {
            evidence.max_spin_clock_gap_nanos = evidence
                .max_spin_clock_gap_nanos
                .max(current.saturating_duration_since(previous).as_nanos());
        }
        let remaining = target.saturating_duration_since(current);
        if remaining.is_zero() {
            evidence.exit_nanos = offset(current, origin);
            return evidence;
        }
        if remaining <= FINAL_SPIN {
            evidence
                .spin_entry_nanos
                .get_or_insert(offset(current, origin));
            previous_spin_tick = Some(current);
            clock.spin_hint();
        } else {
            let requested = (remaining - FINAL_SPIN).min(MAXIMUM_SLEEP);
            clock.sleep(requested);
            let after = clock.now();
            evidence.max_sleep_overshoot_nanos = evidence.max_sleep_overshoot_nanos.max(
                after
                    .saturating_duration_since(current)
                    .saturating_sub(requested)
                    .as_nanos(),
            );
            evidence.last_sleep = Some(SleepEvidence {
                before_nanos: offset(current, origin),
                requested_nanos: requested.as_nanos(),
                after_nanos: offset(after, origin),
            });
        }
    }
}

/// Publishes every bounded chunk on the owner's absolute input clock. Invalid
/// timing remains evidence, including after ring overflow or owner closure.
/// No sample values are logged or retained. This does not request Stop: the
/// caller must immediately timestamp/request its own Stop and check its budget.
/// All errors are detected before the first publication.
pub fn feed(source: &mut ReplayInput, samples: &[f32]) -> Result<FeedEvidence, FeedError> {
    feed_with_clock(source, samples, &mut SystemClock)
}
fn feed_with_clock(
    source: &mut ReplayInput,
    samples: &[f32],
    clock: &mut impl WaitBackend,
) -> Result<FeedEvidence, FeedError> {
    ReplaySchedule::new(source.format, 1).map_err(FeedError::Schedule)?;
    let channels = usize::from(source.format.channels);
    if !samples.len().is_multiple_of(channels) {
        return Err(FeedError::PartialFrame);
    }
    let schedule = ReplaySchedule::new(source.format, samples.len() / channels)
        .map_err(FeedError::Schedule)?;
    let planned_stop_offset = schedule
        .chunks()
        .last()
        .map_or(Duration::ZERO, |plan| plan.planned_offset);
    source
        .started
        .checked_add(planned_stop_offset)
        .ok_or(FeedError::ClockRange)?;
    let mut evidence = FeedEvidence {
        publications: Vec::with_capacity(schedule.chunks().len()),
        timing_valid: true,
        planned_stop_offset,
        wait_summary: WaitSummary::default(),
    };
    for plan in schedule.chunks() {
        let target = source.started + plan.planned_offset;
        // Even a caller-provided future clock must not hold owner teardown for
        // more than one scheduled chunk. No audio is written before target.
        let initial_window_wait = if let Some(window_start) =
            target.checked_sub(Duration::from_millis(20))
            && clock.now() < window_start
        {
            Some(wait_until(window_start, source.started, clock))
        } else {
            None
        };
        let reserve_enter_nanos = offset(clock.now(), source.started);
        clock.before_reserve();
        let publication = source.port.reserve_replay();
        let reserve_exit_nanos = offset(clock.now(), source.started);
        let deadline_wait = wait_until(target, source.started, clock);
        let before = clock.now().saturating_duration_since(source.started);
        if let Some(publication) = publication {
            publication.publish(&samples[plan.start_frame * channels..plan.end_frame * channels]);
        }
        let after = clock.now().saturating_duration_since(source.started);
        let observation = observe(plan, after);
        let valid = before >= plan.planned_offset
            && observation.classification == PublicationTiming::OnBudget;
        evidence.timing_valid &= valid;
        let detail = ChunkWaitEvidence {
            reserve_enter_nanos,
            reserve_exit_nanos,
            initial_window_wait,
            deadline_wait,
        };
        evidence
            .wait_summary
            .record(&detail, before.as_nanos(), !valid);
        evidence.publications.push(Publication {
            start_frame: plan.start_frame,
            end_frame: plan.end_frame,
            planned_nanos: plan.planned_offset.as_nanos(),
            before_publish_nanos: before.as_nanos(),
            after_publish_nanos: after.as_nanos(),
            deviation_nanos: observation.deviation.as_nanos(),
            diagnostics: (!valid).then_some(detail),
            timing: match observation.classification {
                PublicationTiming::Early => "early",
                PublicationTiming::OnBudget => "onBudget",
                PublicationTiming::Late => "late",
            },
        });
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::super::ReplayInput;
    use super::*;
    use crate::audio::{CaptureError, CaptureEvent, CaptureHandle, capture_buffer};
    use dictation_core::audio::AudioFormat;
    use std::time::{Duration, Instant};

    fn source(
        format: AudioFormat,
    ) -> Result<(ReplayInput, crate::audio::CaptureReader), CaptureError> {
        let (port, reader) = capture_buffer(format)?;
        Ok((
            ReplayInput {
                port,
                started: Instant::now(),
                format,
            },
            reader,
        ))
    }

    #[test]
    fn stereo_feed_counts_frames_and_preserves_partial_tail_and_interleaving()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut source, mut reader) = source(AudioFormat {
            sample_rate: 44100,
            channels: 2,
        })?;
        let samples: Vec<_> = (0..1766).map(|index| index as f32 / 2000.0).collect();
        let evidence = feed(&mut source, &samples)?;
        assert_eq!(evidence.publications.len(), 2);
        assert_eq!(evidence.publications[0].end_frame, 882);
        assert_eq!(evidence.publications[1].start_frame, 882);
        assert_eq!(evidence.publications[1].end_frame, 883);
        assert_eq!(
            evidence.planned_stop_offset,
            Duration::from_nanos(20_022_676)
        );
        assert!(
            evidence
                .publications
                .iter()
                .all(|row| row.before_publish_nanos >= row.planned_nanos)
        );
        assert_eq!(reader.read()?, samples);
        let serialized = serde_json::to_value(evidence.clone())?;
        assert_eq!(serialized["publications"][1]["plannedNanos"], 20_022_676u64);
        Ok(())
    }

    #[test]
    fn invalid_input_is_rejected_before_any_publication() -> Result<(), Box<dyn std::error::Error>>
    {
        let valid = AudioFormat {
            sample_rate: 8000,
            channels: 2,
        };
        let (mut source, mut reader) = source(valid)?;
        for samples in [vec![], vec![0.0; 3], vec![0.0; 8000 * 300 * 2 + 2]] {
            assert!(feed(&mut source, &samples).is_err());
            assert!(reader.read()?.is_empty());
        }
        for format in [
            AudioFormat {
                sample_rate: 0,
                channels: 1,
            },
            AudioFormat {
                sample_rate: 16000,
                channels: 0,
            },
            AudioFormat {
                sample_rate: 96000,
                channels: 9,
            },
        ] {
            source.format = format;
            assert!(feed(&mut source, &[0.0; 2]).is_err());
            assert!(reader.read()?.is_empty());
        }
        Ok(())
    }

    #[test]
    fn blocked_consumer_keeps_all_late_publications_and_reports_real_ring_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut source, mut reader) = source(AudioFormat {
            sample_rate: 16000,
            channels: 1,
        })?;
        source.port.publish(&vec![0.0; 16000]);
        source.started = Instant::now() - Duration::from_secs(1);
        let evidence = feed(&mut source, &[0.125; 641])?;
        assert_eq!(evidence.publications.len(), 3);
        assert_eq!(
            evidence
                .publications
                .last()
                .ok_or("missing tail")?
                .end_frame,
            641
        );
        assert!(!evidence.timing_valid);
        assert!(evidence.publications.iter().all(|row| row.timing == "late"));
        assert!(matches!(reader.read(), Err(CaptureError::Overflow)));
        Ok(())
    }

    #[test]
    fn maximum_duration_retains_exactly_fifteen_thousand_records_even_after_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut source, mut reader) = source(AudioFormat {
            sample_rate: 8000,
            channels: 1,
        })?;
        // Already elapsed absolute deadlines let us exercise the complete
        // bounded publication path without a 300-second wall-clock test.
        source.started = Instant::now() - Duration::from_secs(301);
        let evidence = feed(&mut source, &vec![0.125; 2_400_000])?;
        assert_eq!(evidence.publications.len(), 15_000);
        assert_eq!(evidence.planned_stop_offset, Duration::from_secs(300));
        assert_eq!(evidence.publications[14_999].end_frame, 2_400_000);
        assert_eq!(evidence.wait_summary.observed_publications, 15_000);
        assert_eq!(evidence.wait_summary.detailed_publications, 15_000);
        assert!(
            evidence
                .publications
                .iter()
                .all(|row| row.diagnostics.is_some())
        );
        assert!(!evidence.timing_valid);
        assert!(matches!(reader.read(), Err(CaptureError::Overflow)));
        Ok(())
    }

    #[test]
    fn actual_capture_owner_is_stopped_only_by_caller_and_retains_all_samples()
    -> Result<(), Box<dyn std::error::Error>> {
        let (ready, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_replay(
            97,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            ready,
        )?;
        let mut source = receive.recv_timeout(Duration::from_secs(2))?;
        let evidence = feed(&mut source, &[0.125; 321])?;
        let remained_active = !handle.is_finished();
        handle.request_stop();
        drop(source);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut finished = None;
        while Instant::now() < deadline {
            while let Some(event) = handle.next_event() {
                if let CaptureEvent::Finished {
                    session, result, ..
                } = event
                {
                    finished = Some((session, result));
                }
            }
            if handle.is_finished() && finished.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let joined = if handle.is_finished() {
            handle.join().is_ok()
        } else {
            drop(handle);
            false
        };
        assert!(joined, "bounded owner cleanup");
        assert!(remained_active, "feed must not stop the owner");
        let (session, audio) = finished.ok_or("missing finished event")?;
        assert_eq!(session, 97);
        assert_eq!(audio?, vec![0.125; 321]);
        assert_eq!(evidence.publications.len(), 2);
        Ok(())
    }
    #[derive(Clone, Copy)]
    enum Gap {
        Sleep,
        Spin,
        Reserve,
        None,
    }
    struct ControlledClock {
        base: Instant,
        offset: Duration,
        gap: Gap,
        injected: bool,
    }
    impl WaitBackend for ControlledClock {
        fn now(&mut self) -> Instant {
            self.base + self.offset
        }
        fn sleep(&mut self, duration: Duration) {
            self.offset += duration;
            if matches!(self.gap, Gap::Sleep)
                && !self.injected
                && self.offset >= Duration::from_micros(19_750)
            {
                self.offset += Duration::from_millis(12);
                self.injected = true;
            }
        }
        fn spin_hint(&mut self) {
            if matches!(self.gap, Gap::Spin) && !self.injected {
                self.offset += Duration::from_millis(12);
                self.injected = true;
            } else {
                self.offset += Duration::from_micros(25);
            }
        }
        fn before_reserve(&mut self) {
            if matches!(self.gap, Gap::Reserve) && !self.injected {
                self.offset += Duration::from_millis(12);
                self.injected = true;
            }
        }
    }
    fn controlled_feed(gap: Gap) -> Result<FeedEvidence, Box<dyn std::error::Error>> {
        let (mut source, mut reader) = source(AudioFormat {
            sample_rate: 16000,
            channels: 1,
        })?;
        let mut clock = ControlledClock {
            base: source.started,
            offset: if matches!(gap, Gap::Reserve) {
                Duration::from_micros(19_750)
            } else {
                Duration::ZERO
            },
            gap,
            injected: false,
        };
        let evidence = feed_with_clock(&mut source, &[0.125; 320], &mut clock)?;
        assert_eq!(reader.read()?, vec![0.125; 320]);
        Ok(evidence)
    }
    #[test]
    fn diagnostic_sleep_gap_retains_requested_and_actual_sleep_without_blame()
    -> Result<(), Box<dyn std::error::Error>> {
        let evidence = controlled_feed(Gap::Sleep)?;
        assert!(!evidence.timing_valid);
        let detail = evidence.publications[0]
            .diagnostics
            .as_ref()
            .ok_or("missing late sleep evidence")?;
        assert_eq!(detail.deadline_wait.max_sleep_overshoot_nanos, 12_000_000);
        let sleep = detail
            .deadline_wait
            .last_sleep
            .as_ref()
            .ok_or("missing last sleep")?;
        assert_eq!(
            sleep.after_nanos - sleep.before_nanos - sleep.requested_nanos,
            12_000_000
        );
        assert_eq!(detail.deadline_wait.spin_entry_nanos, None);
        assert_eq!(evidence.wait_summary.max_sleep_overshoot_nanos, 12_000_000);
        assert_eq!(evidence.wait_summary.observed_publications, 1);
        let json = serde_json::to_value(&evidence)?;
        assert_eq!(
            json["publications"][0]["diagnostics"]["deadlineWait"]["maxSleepOvershootNanos"],
            12_000_000u64
        );
        assert_eq!(json["waitSummary"]["detailedPublications"], 1);
        Ok(())
    }
    #[test]
    fn diagnostic_spin_gap_is_distinct_from_sleep_overshoot()
    -> Result<(), Box<dyn std::error::Error>> {
        let evidence = controlled_feed(Gap::Spin)?;
        assert!(!evidence.timing_valid);
        let detail = evidence.publications[0]
            .diagnostics
            .as_ref()
            .ok_or("missing late spin evidence")?;
        assert_eq!(detail.deadline_wait.max_spin_clock_gap_nanos, 12_000_000);
        assert_eq!(detail.deadline_wait.max_sleep_overshoot_nanos, 0);
        assert_eq!(detail.deadline_wait.spin_entry_nanos, Some(19_750_000));
        assert_eq!(evidence.wait_summary.max_spin_clock_gap_nanos, 12_000_000);
        Ok(())
    }
    #[test]
    fn diagnostic_reserve_gap_precedes_wait_and_is_not_reported_as_sleep()
    -> Result<(), Box<dyn std::error::Error>> {
        let evidence = controlled_feed(Gap::Reserve)?;
        assert!(!evidence.timing_valid);
        let detail = evidence.publications[0]
            .diagnostics
            .as_ref()
            .ok_or("missing reserve evidence")?;
        assert_eq!(
            detail.reserve_exit_nanos - detail.reserve_enter_nanos,
            12_000_000
        );
        assert_eq!(detail.deadline_wait.enter_nanos, 31_750_000);
        assert!(detail.deadline_wait.last_sleep.is_none());
        assert_eq!(detail.deadline_wait.max_spin_clock_gap_nanos, 0);
        assert_eq!(evidence.wait_summary.max_reserve_nanos, 12_000_000);
        Ok(())
    }
    #[test]
    fn diagnostic_on_budget_feed_keeps_only_bounded_summary_and_same_deadline()
    -> Result<(), Box<dyn std::error::Error>> {
        let evidence = controlled_feed(Gap::None)?;
        assert!(evidence.timing_valid);
        assert_eq!(evidence.publications[0].before_publish_nanos, 20_000_000);
        assert!(evidence.publications[0].diagnostics.is_none());
        assert_eq!(evidence.wait_summary.observed_publications, 1);
        assert_eq!(evidence.wait_summary.detailed_publications, 0);
        let json = serde_json::to_value(&evidence)?;
        assert!(json["publications"][0].get("diagnostics").is_none());
        assert_eq!(json["waitSummary"]["observedPublications"], 1);
        assert_eq!(MAXIMUM_SLEEP, Duration::from_millis(1));
        assert_eq!(FINAL_SPIN, Duration::from_micros(250));
        assert_eq!(
            super::super::schedule::JITTER_TOLERANCE,
            Duration::from_millis(10)
        );
        Ok(())
    }
}
