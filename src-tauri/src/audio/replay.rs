//! Developer-only injection into the real capture/DSP owner. No pacing here.
use super::CapturePort;
use super::{CaptureError, CaptureEvent, ProgressSlot, RecordingOwner, capture_buffer};
use crossbeam_channel::Sender;
use dictation_core::audio::AudioFormat;
use dictation_core::audio::AudioProcessor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub mod clock;
pub mod schedule;

/// The owner has started at `started`. The caller owns pacing/publication and
/// must eventually stop the handle. Keeping the port does not keep admission
/// open after Stop; post-close publications are ignored by the real boundary.
pub struct ReplayInput {
    pub port: CapturePort,
    pub started: Instant,
    pub format: AudioFormat,
}

pub(super) fn record(
    session: u64,
    format: AudioFormat,
    ready: Sender<ReplayInput>,
    events: &Sender<CaptureEvent>,
    stop: &AtomicBool,
    maximum: &mut bool,
    progress: &ProgressSlot,
) -> Result<Vec<f32>, CaptureError> {
    if stop.load(Ordering::Acquire) {
        return Ok(Vec::new());
    }
    let processor = AudioProcessor::new(format)?;
    let (port, reader) = capture_buffer(format)?;
    let started = Instant::now();
    RecordingOwner {
        session,
        events,
        stop,
        maximum,
        progress,
    }
    .run(processor, reader, (), started, move || {
        ready
            .try_send(ReplayInput {
                port,
                started,
                format,
            })
            .map_err(|error| match error {
                crossbeam_channel::TrySendError::Full(_) => CaptureError::EventOverflow,
                crossbeam_channel::TrySendError::Disconnected(_) => CaptureError::ControllerGone,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{CaptureEvent, CaptureHandle};
    use std::{thread, time::Duration};

    fn terminal(handle: &CaptureHandle) -> Result<CaptureEvent, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event @ CaptureEvent::Finished { .. }) = handle.next_event() {
                return Ok(event);
            }
            if Instant::now() >= deadline {
                return Err("capture owner did not finish".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn replay_started_clock_and_final_stop_drain_use_the_real_capture_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 2,
        };
        let (send, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_replay(81, format, send)?;
        let mut input = receive.recv_timeout(Duration::from_secs(2))?;
        match handle.next_event() {
            Some(CaptureEvent::Started {
                session,
                format: actual,
                started,
            }) => {
                assert_eq!(session, 81);
                assert_eq!(actual, format);
                assert_eq!(started, input.started);
            }
            _ => return Err("missing Started before ready port".into()),
        }
        input.port.publish(&[0.25f32, 0.75, -0.5, 0.0, 0.5, 0.5]);
        handle.request_stop();
        match terminal(&handle)? {
            CaptureEvent::Finished {
                session: 81,
                result: Ok(audio),
                maximum: false,
            } => assert_eq!(audio, [0.5, -0.25, 0.5]),
            _ => return Err("stop drain lost actual input or failed".into()),
        }
        input.port.publish(&[1.0f32, 1.0]);
        assert!(handle.next_event().is_none());
        handle.join()?;
        Ok(())
    }

    #[test]
    fn stop_flushes_the_actual_partial_resampler_block_and_downmixes_stereo()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 48000,
            channels: 2,
        };
        let (send, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_replay(82, format, send)?;
        let mut input = receive.recv_timeout(Duration::from_secs(2))?;
        let samples: Vec<f32> = (0..4817).flat_map(|_| [0.25, 0.75]).collect();
        input.port.publish(&samples);
        handle.request_stop();
        match terminal(&handle)? {
            CaptureEvent::Finished {
                result: Ok(audio),
                maximum: false,
                ..
            } => {
                assert_eq!(audio.len(), 1606);
                assert!(audio[400..1000].iter().all(|v| (*v - 0.5).abs() < 0.001));
            }
            _ => return Err("partial block was not flushed".into()),
        }
        handle.join()?;
        Ok(())
    }

    #[test]
    fn publication_errors_reach_finished_without_partial_success()
    -> Result<(), Box<dyn std::error::Error>> {
        for kind in 0..3 {
            let format = AudioFormat {
                sample_rate: 16000,
                channels: 2,
            };
            let (send, receive) = crossbeam_channel::bounded(1);
            let handle = CaptureHandle::start_replay(90 + kind, format, send)?;
            let mut input = receive.recv_timeout(Duration::from_secs(2))?;
            match kind {
                0 => input.port.publish(&[f32::NAN, 0.0]),
                1 => input.port.publish(&vec![0.1f32; 32002]),
                _ => input.port.publish(&[0.1f32]),
            }
            handle.request_stop();
            let CaptureEvent::Finished {
                result: Err(error), ..
            } = terminal(&handle)?
            else {
                return Err("failed publication became successful audio".into());
            };
            assert!(matches!(
                (kind, error),
                (0, CaptureError::InvalidSamples)
                    | (1, CaptureError::Overflow)
                    | (2, CaptureError::IncompleteFrames)
            ));
            assert!(
                input.port.gate.publications.load(Ordering::Acquire) & super::super::CLOSED != 0
            );
            handle.join()?;
        }
        Ok(())
    }

    #[test]
    fn immediate_stop_before_ready_consumption_still_finishes_and_closes_the_port()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 1,
        };
        let (send, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_replay(94, format, send)?;
        handle.request_stop();
        match terminal(&handle)? {
            CaptureEvent::Finished {
                result: Ok(audio),
                maximum: false,
                ..
            } => assert!(audio.is_empty()),
            _ => return Err("stop before readiness did not finish normally".into()),
        }
        if let Ok(mut input) = receive.try_recv() {
            assert_eq!(
                input.port.gate.publications.load(Ordering::Acquire),
                super::super::CLOSED
            );
            input.port.publish(&[0.1f32]);
        }
        handle.join()?;
        Ok(())
    }

    #[test]
    fn stop_won_before_source_initialization_never_publishes_started_or_ready()
    -> Result<(), Box<dyn std::error::Error>> {
        // Pause only before source initialization, through the shared Handle
        // spawn boundary. The actual replay source/owner implementation runs
        // after Stop; no fake DSP or duplicated stop behavior is involved.
        let (at_start, starting) = crossbeam_channel::bounded(1);
        let (release, resume) = crossbeam_channel::bounded(1);
        let (ready, incoming) = crossbeam_channel::bounded(1);
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 1,
        };
        let handle = CaptureHandle::spawn(97, move |events, stop, maximum, progress| {
            at_start
                .try_send(())
                .map_err(|_| CaptureError::ControllerGone)?;
            resume
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| CaptureError::Worker)?;
            record(97, format, ready, events, stop, maximum, progress)
        })?;
        starting.recv_timeout(Duration::from_secs(2))?;
        handle.request_stop();
        release.try_send(())?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = handle.next_event() {
                match event {
                    CaptureEvent::Finished {
                        result: Ok(audio),
                        maximum: false,
                        ..
                    } => assert!(audio.is_empty()),
                    _ => return Err("stop before initialization emitted Started or failed".into()),
                }
                break;
            }
            if Instant::now() > deadline {
                return Err("missing stopped-before-start Finished".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(incoming.try_recv().is_err());
        assert!(handle.next_event().is_none());
        handle.join()?;
        Ok(())
    }

    #[test]
    fn disconnected_or_full_readiness_consumer_never_blocks_owner_cleanup()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 1,
        };
        for full in [false, true] {
            let (send, receive) = crossbeam_channel::bounded(1);
            if full {
                let (port, _reader) = capture_buffer(format)?;
                send.try_send(ReplayInput {
                    port,
                    format,
                    started: Instant::now(),
                })
                .map_err(|_| "prefill failed")?;
            } else {
                drop(receive);
            }
            let handle = CaptureHandle::start_replay(95, format, send)?;
            let CaptureEvent::Finished {
                result: Err(error), ..
            } = terminal(&handle)?
            else {
                return Err("readiness failure did not produce terminal error".into());
            };
            assert!(matches!(
                (full, error),
                (false, CaptureError::ControllerGone) | (true, CaptureError::EventOverflow)
            ));
            handle.join()?;
        }
        Ok(())
    }

    #[test]
    fn owner_progress_uses_real_start_time_and_accepted_signal()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 1,
        };
        let (send, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_replay(96, format, send)?;
        let mut input = receive.recv_timeout(Duration::from_secs(2))?;
        input.port.publish(&[0.5f32; 160]);
        let deadline = Instant::now() + Duration::from_secs(2);
        let measured = loop {
            if let Some(progress) = handle.take_progress() {
                break progress;
            }
            if Instant::now() > deadline {
                handle.request_stop();
                return Err("missing owner progress".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!((measured.session, measured.level_permille), (96, 500));
        assert!(measured.elapsed_millis >= 50);
        assert!(measured.elapsed_millis <= input.started.elapsed().as_millis() as u64);
        handle.request_stop();
        assert!(matches!(
            terminal(&handle)?,
            CaptureEvent::Finished { result: Ok(_), .. }
        ));
        handle.join()?;
        Ok(())
    }

    #[test]
    fn owner_unwind_closes_a_handed_out_port_and_join_reports_the_panic()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16000,
            channels: 1,
        };
        let (ready, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::spawn(98, move |events, stop, maximum, progress| {
            let (port, reader) = capture_buffer(format)?;
            let processor = AudioProcessor::new(format)?;
            let started = Instant::now();
            RecordingOwner {
                session: 98,
                events,
                stop,
                maximum,
                progress,
            }
            .run(processor, reader, (), started, move || {
                ready
                    .try_send(ReplayInput {
                        port,
                        started,
                        format,
                    })
                    .map_err(|_| CaptureError::ControllerGone)?;
                std::panic::resume_unwind(Box::new("controlled owner activation panic"));
            })
        })?;
        let mut input = receive.recv_timeout(Duration::from_secs(2))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !handle.is_finished() {
            if Instant::now() > deadline {
                return Err("panicked owner did not terminate".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            input.port.gate.publications.load(Ordering::Acquire),
            super::super::CLOSED,
            "a handed-out replay port must close even when activation unwinds"
        );
        input.port.publish(&[0.25f32; 160]);
        assert_eq!(
            input.port.gate.publications.load(Ordering::Acquire),
            super::super::CLOSED
        );
        assert!(matches!(handle.join(), Err(CaptureError::Worker)));
        Ok(())
    }
}
