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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEvidence {
    pub publications: Vec<Publication>,
    pub timing_valid: bool,
    pub planned_stop_offset: Duration,
}

/// Short sleeps avoid relying on one coalescible 20-ms OS timer. The final
/// spin is bounded by 250 us per chunk; it never waits for a consumer.
fn wait_until(target: Instant) {
    loop {
        let remaining = target.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        if remaining <= FINAL_SPIN {
            std::hint::spin_loop();
        } else {
            thread::sleep((remaining - FINAL_SPIN).min(MAXIMUM_SLEEP));
        }
    }
}

/// Publishes every bounded chunk on the owner's absolute input clock. Invalid
/// timing remains evidence, including after ring overflow or owner closure.
/// No sample values are logged or retained. This does not request Stop: the
/// caller must immediately timestamp/request its own Stop and check its budget.
/// All errors are detected before the first publication.
pub fn feed(source: &mut ReplayInput, samples: &[f32]) -> Result<FeedEvidence, FeedError> {
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
    };
    for plan in schedule.chunks() {
        wait_until(source.started + plan.planned_offset);
        let before = source.started.elapsed();
        source
            .port
            .publish(&samples[plan.start_frame * channels..plan.end_frame * channels]);
        let after = source.started.elapsed();
        let observation = observe(plan, after);
        evidence.timing_valid &= before >= plan.planned_offset
            && observation.classification == PublicationTiming::OnBudget;
        evidence.publications.push(Publication {
            start_frame: plan.start_frame,
            end_frame: plan.end_frame,
            planned_nanos: plan.planned_offset.as_nanos(),
            before_publish_nanos: before.as_nanos(),
            after_publish_nanos: after.as_nanos(),
            deviation_nanos: observation.deviation.as_nanos(),
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
}
