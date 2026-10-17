//! Deterministic input-frame timeline for the explicit developer replay adapter.

use dictation_core::audio::AudioFormat;
use std::time::Duration;

pub const MAX_CHUNKS: usize = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    UnsupportedFormat,
    InvalidFrameCount,
}
impl std::fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedFormat => "Unsupported replay audio format",
            Self::InvalidFrameCount => {
                "Replay frame count is outside the 1-frame to 300-second limit"
            }
        })
    }
}
impl std::error::Error for ScheduleError {}

#[derive(Debug, Clone, Copy)]
pub struct ReplaySchedule {
    sample_rate: usize,
    total_frames: usize,
}
impl ReplaySchedule {
    /// Counts frames, not interleaved samples. All channels in a frame share
    /// one publication time; channel count never multiplies audio duration.
    pub fn new(format: AudioFormat, total_frames: usize) -> Result<Self, ScheduleError> {
        if !(1..=8).contains(&format.channels)
            || ![8_000, 16_000, 24_000, 32_000, 44_100, 48_000].contains(&format.sample_rate)
        {
            return Err(ScheduleError::UnsupportedFormat);
        }
        let sample_rate = format.sample_rate as usize;
        if total_frames == 0 || total_frames > sample_rate * 300 {
            return Err(ScheduleError::InvalidFrameCount);
        }
        Ok(Self {
            sample_rate,
            total_frames,
        })
    }
    pub fn chunks(self) -> ChunkPlans {
        ChunkPlans {
            schedule: self,
            next_frame: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkPlan {
    pub start_frame: usize,
    pub end_frame: usize,
    pub planned_offset: Duration,
}

/// Constant-space iterator; even a maximum recording has at most 15,000 chunks.
#[derive(Debug, Clone)]
pub struct ChunkPlans {
    schedule: ReplaySchedule,
    next_frame: usize,
}
impl Iterator for ChunkPlans {
    type Item = ChunkPlan;
    fn next(&mut self) -> Option<Self::Item> {
        let start_frame = self.next_frame;
        if start_frame == self.schedule.total_frames {
            return None;
        }
        let end_frame =
            (start_frame + self.schedule.sample_rate / 50).min(self.schedule.total_frames);
        self.next_frame = end_frame;
        // Integer ceiling never schedules before the exact rational frame time.
        // Bounds: <=14,400,000 frames * 1e9, comfortably within u64.
        let nanos = (end_frame as u64 * 1_000_000_000).div_ceil(self.schedule.sample_rate as u64);
        Some(ChunkPlan {
            start_frame,
            end_frame,
            planned_offset: Duration::from_nanos(nanos),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining =
            (self.schedule.total_frames - self.next_frame).div_ceil(self.schedule.sample_rate / 50);
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for ChunkPlans {}
impl std::iter::FusedIterator for ChunkPlans {}

/// Fixed before measurement. It is a report budget, never permission to
/// publish future audio early or to slow the input clock for a consumer.
pub const JITTER_TOLERANCE: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationTiming {
    Early,
    OnBudget,
    Late,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicationObservation {
    pub plan: ChunkPlan,
    pub actual_publish_offset: Duration,
    pub classification: PublicationTiming,
    /// Absolute deviation; classification preserves whether it was early/late.
    pub deviation: Duration,
}
/// Does not reject or discard observations. The caller retains every record,
/// including early/late publications and failed runs, for the complete report.
pub fn observe(plan: ChunkPlan, actual_publish_offset: Duration) -> PublicationObservation {
    let (classification, deviation) = if actual_publish_offset < plan.planned_offset {
        (
            PublicationTiming::Early,
            plan.planned_offset - actual_publish_offset,
        )
    } else {
        let deviation = actual_publish_offset - plan.planned_offset;
        (
            if deviation <= JITTER_TOLERANCE {
                PublicationTiming::OnBudget
            } else {
                PublicationTiming::Late
            },
            deviation,
        )
    };
    PublicationObservation {
        plan,
        actual_publish_offset,
        classification,
        deviation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_second_plan_publishes_each_twenty_ms_at_its_exclusive_end_frame()
    -> Result<(), ScheduleError> {
        let schedule = ReplaySchedule::new(
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            80000,
        )?;
        assert_eq!(schedule.chunks().len(), 250);
        let mut chunks = schedule.chunks();
        assert_eq!(
            chunks.next(),
            Some(ChunkPlan {
                start_frame: 0,
                end_frame: 320,
                planned_offset: Duration::from_millis(20)
            })
        );
        assert_eq!(
            chunks.last(),
            Some(ChunkPlan {
                start_frame: 79680,
                end_frame: 80000,
                planned_offset: Duration::from_secs(5)
            })
        );
        Ok(())
    }
    #[test]
    fn publication_budget_never_allows_early_data_and_includes_exactly_ten_ms_late() {
        let plan = ChunkPlan {
            start_frame: 0,
            end_frame: 320,
            planned_offset: Duration::from_millis(20),
        };
        for (actual, classification, deviation) in [
            (
                Duration::ZERO,
                PublicationTiming::Early,
                Duration::from_millis(20),
            ),
            (
                Duration::from_nanos(19_999_999),
                PublicationTiming::Early,
                Duration::from_nanos(1),
            ),
            (
                Duration::from_millis(20),
                PublicationTiming::OnBudget,
                Duration::ZERO,
            ),
            (
                Duration::from_nanos(29_999_999),
                PublicationTiming::OnBudget,
                Duration::from_nanos(9_999_999),
            ),
            (
                Duration::from_millis(30),
                PublicationTiming::OnBudget,
                Duration::from_millis(10),
            ),
            (
                Duration::from_nanos(30_000_001),
                PublicationTiming::Late,
                Duration::from_nanos(10_000_001),
            ),
            (
                Duration::MAX,
                PublicationTiming::Late,
                Duration::MAX - Duration::from_millis(20),
            ),
        ] {
            let observation = observe(plan, actual);
            assert_eq!(observation.plan, plan);
            assert_eq!(observation.actual_publish_offset, actual);
            assert_eq!(observation.classification, classification);
            assert_eq!(observation.deviation, deviation);
        }
        assert_eq!(JITTER_TOLERANCE, Duration::from_millis(10));
    }

    #[test]
    fn standard_durations_are_contiguous_finite_and_bounded_for_every_supported_rate()
    -> Result<(), ScheduleError> {
        assert_eq!(MAX_CHUNKS, 15_000);
        for sample_rate in [8_000, 16_000, 24_000, 32_000, 44_100, 48_000] {
            for (seconds, expected_chunks) in [(5, 250), (10, 500), (15, 750), (300, 15_000)] {
                let total_frames = sample_rate as usize * seconds;
                let schedule = ReplaySchedule::new(
                    AudioFormat {
                        sample_rate,
                        channels: 8,
                    },
                    total_frames,
                )?;
                let mut chunks = schedule.chunks();
                assert_eq!(chunks.len(), expected_chunks);
                let mut previous_end = 0;
                let mut last_offset = Duration::ZERO;
                for index in 0..expected_chunks {
                    assert_eq!(
                        chunks.size_hint(),
                        (expected_chunks - index, Some(expected_chunks - index))
                    );
                    let chunk = chunks.next().ok_or(ScheduleError::InvalidFrameCount)?;
                    assert_eq!(chunk.start_frame, previous_end);
                    assert!(chunk.end_frame > chunk.start_frame);
                    assert!(chunk.end_frame <= total_frames);
                    assert_eq!(
                        chunk.planned_offset - last_offset,
                        Duration::from_millis(20)
                    );
                    previous_end = chunk.end_frame;
                    last_offset = chunk.planned_offset;
                }
                assert_eq!(previous_end, total_frames);
                assert_eq!(last_offset, Duration::from_secs(seconds as u64));
                assert_eq!(chunks.len(), 0);
                assert_eq!(chunks.next(), None);
                assert_eq!(chunks.next(), None);
            }
        }
        Ok(())
    }
    #[test]
    fn stereo_44100_counts_frames_and_keeps_exact_thirty_ms_and_partial_tail()
    -> Result<(), ScheduleError> {
        let format = AudioFormat {
            sample_rate: 44_100,
            channels: 2,
        };
        let chunks: Vec<_> = ReplaySchedule::new(format, 1323)?.chunks().collect();
        assert_eq!(
            chunks,
            [
                ChunkPlan {
                    start_frame: 0,
                    end_frame: 882,
                    planned_offset: Duration::from_millis(20)
                },
                ChunkPlan {
                    start_frame: 882,
                    end_frame: 1323,
                    planned_offset: Duration::from_millis(30)
                },
            ]
        );
        let mono: Vec<_> = ReplaySchedule::new(
            AudioFormat {
                sample_rate: 44_100,
                channels: 1,
            },
            1323,
        )?
        .chunks()
        .collect();
        assert_eq!(chunks, mono);
        for (frames, expected_nanos) in [(1, 22_676), (883, 20_022_676), (1322, 29_977_325)] {
            let last = ReplaySchedule::new(format, frames)?
                .chunks()
                .last()
                .ok_or(ScheduleError::InvalidFrameCount)?;
            assert_eq!(last.end_frame, frames);
            assert_eq!(last.planned_offset, Duration::from_nanos(expected_nanos));
        }
        Ok(())
    }
    #[test]
    fn invalid_formats_and_outside_frame_bounds_are_rejected_before_iteration() {
        for (sample_rate, channels) in [
            (0, 1),
            (16_000, 0),
            (16_000, 9),
            (44_101, 2),
            (96_000, 1),
            (u32::MAX, u16::MAX),
        ] {
            assert!(matches!(
                ReplaySchedule::new(
                    AudioFormat {
                        sample_rate,
                        channels
                    },
                    1
                ),
                Err(ScheduleError::UnsupportedFormat)
            ));
        }
        for sample_rate in [8_000, 16_000, 24_000, 32_000, 44_100, 48_000] {
            let format = AudioFormat {
                sample_rate,
                channels: 1,
            };
            for frames in [0, sample_rate as usize * 300 + 1, usize::MAX] {
                assert!(matches!(
                    ReplaySchedule::new(format, frames),
                    Err(ScheduleError::InvalidFrameCount)
                ));
            }
            assert!(ReplaySchedule::new(format, 1).is_ok());
        }
        // Eight-channel interleaved sample count must not be passed as frames.
        assert!(matches!(
            ReplaySchedule::new(
                AudioFormat {
                    sample_rate: 48_000,
                    channels: 8
                },
                115_200_000
            ),
            Err(ScheduleError::InvalidFrameCount)
        ));
    }
}
