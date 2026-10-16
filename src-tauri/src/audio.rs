//! Native microphone owner and a preallocated callback-to-DSP boundary.

use cpal::{
    FromSample, Sample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use dictation_core::audio::{AudioError, AudioFormat, AudioProcessor, MAX_SECONDS};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};
use std::{
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const CLOSED: usize = 1 << (usize::BITS - 1);

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error("The microphone buffer overflowed; this recording is incomplete")]
    Overflow,
    #[error("The microphone returned incomplete frames")]
    IncompleteFrames,
    #[error("The microphone returned invalid samples")]
    InvalidSamples,
    #[error("The microphone stream failed")]
    Stream,
    #[error("The audio buffer could not be drained")]
    Drain,
    #[error("Microphone device unavailable or ambiguous")]
    DeviceUnavailable,
    #[error("The microphone format is not supported")]
    UnsupportedSampleFormat,
    #[error("Microphone operation failed: {0}")]
    Native(String),
    #[error("Audio controller disconnected")]
    ControllerGone,
    #[error("Audio event capacity exceeded")]
    EventOverflow,
    #[error("Audio worker could not be started or joined")]
    Worker,
}

struct Gate {
    publications: AtomicUsize,
    error: AtomicU8,
    maximum: AtomicBool,
}

struct Publication<'a>(&'a Gate);

impl Gate {
    fn enter(&self) -> Option<Publication<'_>> {
        let mut current = self.publications.load(Ordering::Acquire);
        loop {
            if current & CLOSED != 0 || self.error.load(Ordering::Acquire) != 0 {
                return None;
            }
            match self.publications.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Publication(self)),
                Err(actual) => current = actual,
            }
        }
    }

    fn failure(&self) -> Result<(), CaptureError> {
        match self.error.load(Ordering::Acquire) {
            0 => Ok(()),
            1 => Err(CaptureError::Overflow),
            2 => Err(CaptureError::IncompleteFrames),
            3 => Err(CaptureError::InvalidSamples),
            _ => Err(CaptureError::Stream),
        }
    }
}

impl Drop for Publication<'_> {
    fn drop(&mut self) {
        self.0.publications.fetch_sub(1, Ordering::Release);
    }
}

pub struct CapturePort {
    producer: Producer<f32>,
    channels: usize,
    gate: Arc<Gate>,
    remaining_frames: usize,
}

pub struct CaptureReader {
    consumer: Consumer<f32>,
    format: AudioFormat,
    scratch: Vec<f32>,
    gate: Arc<Gate>,
}

/// Reserve exactly one second of the effective input format, in full frames.
pub fn capture_buffer(format: AudioFormat) -> Result<(CapturePort, CaptureReader), CaptureError> {
    format.validate()?;
    let channels = usize::from(format.channels);
    let (producer, consumer) = RingBuffer::new(format.sample_rate as usize * channels);
    let gate = Arc::new(Gate {
        publications: AtomicUsize::new(0),
        error: AtomicU8::new(0),
        maximum: AtomicBool::new(false),
    });
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(2_048 * channels)
        .map_err(|_| AudioError::Allocation)?;
    Ok((
        CapturePort {
            producer,
            channels,
            gate: Arc::clone(&gate),
            remaining_frames: format.sample_rate as usize * MAX_SECONDS,
        },
        CaptureReader {
            consumer,
            format,
            scratch,
            gate,
        },
    ))
}

impl CapturePort {
    /// Called only by the native callback. No allocation, locks, logs or DSP.
    /// A complete callback is published atomically or the entire session fails.
    pub fn publish<T: Sample + Copy>(&mut self, samples: &[T])
    where
        f32: FromSample<T>,
    {
        let Some(_publication) = self.gate.enter() else {
            return;
        };
        if !samples.len().is_multiple_of(self.channels) {
            self.gate.error.store(2, Ordering::Release);
            return;
        }
        let published = samples.len().min(self.remaining_frames * self.channels);
        let Ok(mut chunk) = self.producer.write_chunk(published) else {
            self.gate.error.store(1, Ordering::Release);
            return;
        };
        let (first, second) = chunk.as_mut_slices();
        for (out, input) in first.iter_mut().chain(second.iter_mut()).zip(samples) {
            let normalized = input.to_sample::<f32>();
            if !normalized.is_finite() {
                self.gate.error.store(3, Ordering::Release);
                return;
            }
            *out = normalized;
        }
        chunk.commit_all();
        self.remaining_frames -= published / self.channels;
        if self.remaining_frames == 0 {
            self.gate.maximum.store(true, Ordering::Release);
            self.gate.publications.fetch_or(CLOSED, Ordering::AcqRel);
        }
    }
}

impl CaptureReader {
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Close publication before dropping the stream on its owner thread.
    pub fn close(&self) {
        self.gate.publications.fetch_or(CLOSED, Ordering::AcqRel);
    }

    pub fn publication_finished(&self) -> bool {
        self.gate.publications.load(Ordering::Acquire) == CLOSED
    }

    pub fn maximum_reached(&self) -> bool {
        self.gate.maximum.load(Ordering::Acquire)
    }

    pub fn read(&mut self) -> Result<&[f32], CaptureError> {
        self.gate.failure()?;
        self.scratch.clear();
        let channels = usize::from(self.format.channels);
        let count = (self.consumer.slots() / channels).min(2_048) * channels;
        if count == 0 {
            return Ok(&self.scratch);
        }
        let chunk = self
            .consumer
            .read_chunk(count)
            .map_err(|_| CaptureError::Drain)?;
        let (first, second) = chunk.as_slices();
        self.scratch.extend_from_slice(first);
        self.scratch.extend_from_slice(second);
        chunk.commit_all();
        Ok(&self.scratch)
    }
}

pub enum CaptureEvent {
    Started {
        session: u64,
        format: AudioFormat,
        started: Instant,
    },
    MaximumWarning {
        session: u64,
    },
    Finished {
        session: u64,
        result: Result<Vec<f32>, CaptureError>,
        maximum: bool,
    },
}

fn send_event(events: &Sender<CaptureEvent>, event: CaptureEvent) -> Result<(), CaptureError> {
    events.try_send(event).map_err(|error| match error {
        TrySendError::Full(_) => CaptureError::EventOverflow,
        TrySendError::Disconnected(_) => CaptureError::ControllerGone,
    })
}

/// Stream construction, DSP and destruction stay on this owner thread.
/// Requesting stop never blocks the controller or the GUI.
pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), CaptureError>>>,
    events: Receiver<CaptureEvent>,
}

impl CaptureHandle {
    pub fn start(session: u64, device_name: Option<String>) -> Result<Self, CaptureError> {
        // Exactly one Started, at most one Warning, exactly one Finished.
        // This reserved channel holds the complete lifecycle even if the
        // controller has not read any event yet; progress cannot occupy it.
        let (events, incoming) = crossbeam_channel::bounded(3);
        let stop = Arc::new(AtomicBool::new(false));
        let requested = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("microphone-dsp".into())
            .spawn(move || {
                let mut maximum = false;
                let result = record(
                    session,
                    device_name.as_deref(),
                    &events,
                    &requested,
                    &mut maximum,
                );
                send_event(
                    &events,
                    CaptureEvent::Finished {
                        session,
                        result,
                        maximum,
                    },
                )
            })
            .map_err(|_| CaptureError::Worker)?;
        Ok(Self {
            stop,
            worker: Some(worker),
            events: incoming,
        })
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn next_event(&self) -> Option<CaptureEvent> {
        // Disconnection has no additional event; join retains worker errors.
        self.events.try_recv().ok()
    }

    /// If finished without a terminal event, join exposes the worker error.
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Join outside the GUI, after Finished or during controlled shutdown.
    pub fn join(mut self) -> Result<(), CaptureError> {
        let worker = self.worker.take().ok_or(CaptureError::Worker)?;
        worker.join().map_err(|_| CaptureError::Worker)?
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.request_stop();
    }
}

fn selected_device(name: Option<&str>) -> Result<cpal::Device, CaptureError> {
    let host = cpal::default_host();
    let Some(name) = name else {
        return host
            .default_input_device()
            .ok_or(CaptureError::DeviceUnavailable);
    };
    let mut matching = host
        .input_devices()
        .map_err(|error| CaptureError::Native(error.to_string()))?
        .filter(|device| device.name().is_ok_and(|actual| actual == name));
    let device = matching.next().ok_or(CaptureError::DeviceUnavailable)?;
    if matching.next().is_some() {
        return Err(CaptureError::DeviceUnavailable);
    }
    Ok(device)
}

fn stream_for<T: cpal::SizedSample + Copy>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut port: CapturePort,
) -> Result<cpal::Stream, CaptureError>
where
    f32: FromSample<T>,
{
    let failure = Arc::clone(&port.gate);
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| port.publish(samples),
            move |_| {
                failure.error.store(4, Ordering::Release);
            },
            None,
        )
        .map_err(|error| CaptureError::Native(error.to_string()))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    port: CapturePort,
) -> Result<cpal::Stream, CaptureError> {
    let format = config.sample_format();
    let config = config.config();
    match format {
        cpal::SampleFormat::F32 => stream_for::<f32>(device, &config, port),
        cpal::SampleFormat::F64 => stream_for::<f64>(device, &config, port),
        cpal::SampleFormat::I8 => stream_for::<i8>(device, &config, port),
        cpal::SampleFormat::I16 => stream_for::<i16>(device, &config, port),
        cpal::SampleFormat::I32 => stream_for::<i32>(device, &config, port),
        cpal::SampleFormat::I64 => stream_for::<i64>(device, &config, port),
        cpal::SampleFormat::U8 => stream_for::<u8>(device, &config, port),
        cpal::SampleFormat::U16 => stream_for::<u16>(device, &config, port),
        cpal::SampleFormat::U32 => stream_for::<u32>(device, &config, port),
        cpal::SampleFormat::U64 => stream_for::<u64>(device, &config, port),
        _ => Err(CaptureError::UnsupportedSampleFormat),
    }
}

struct OwnedStream {
    stream: Option<cpal::Stream>,
    gate: Arc<Gate>,
}

impl OwnedStream {
    fn play(&self) -> Result<(), CaptureError> {
        let Some(stream) = &self.stream else {
            return Err(CaptureError::Stream);
        };
        stream
            .play()
            .map_err(|error| CaptureError::Native(error.to_string()))
    }
}

impl Drop for OwnedStream {
    fn drop(&mut self) {
        self.gate.publications.fetch_or(CLOSED, Ordering::AcqRel);
        drop(self.stream.take());
        while self.gate.publications.load(Ordering::Acquire) != CLOSED {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn record(
    session: u64,
    device_name: Option<&str>,
    events: &Sender<CaptureEvent>,
    stop: &AtomicBool,
    maximum: &mut bool,
) -> Result<Vec<f32>, CaptureError> {
    if stop.load(Ordering::Acquire) {
        return Ok(Vec::new());
    }
    let device = selected_device(device_name)?;
    let config = device
        .default_input_config()
        .map_err(|error| CaptureError::Native(error.to_string()))?;
    let format = AudioFormat {
        sample_rate: config.sample_rate().0,
        channels: config.channels(),
    };
    let mut processor = AudioProcessor::new(format)?;
    let (port, mut reader) = capture_buffer(format)?;
    let stream = OwnedStream {
        stream: Some(build_stream(&device, &config, port)?),
        gate: Arc::clone(&reader.gate),
    };
    stream.play()?;
    let started = Instant::now();
    if let Err(error) = send_event(
        events,
        CaptureEvent::Started {
            session,
            format,
            started,
        },
    ) {
        reader.close();
        drop(stream);
        return Err(error);
    }
    let mut warned = false;
    let processing = (|| -> Result<(), CaptureError> {
        loop {
            let elapsed = started.elapsed();
            if !warned && elapsed >= Duration::from_secs((MAX_SECONDS - 30) as u64) {
                send_event(events, CaptureEvent::MaximumWarning { session })?;
                warned = true;
            }
            if elapsed >= Duration::from_secs(MAX_SECONDS as u64) || reader.maximum_reached() {
                *maximum = true;
                break;
            }
            if stop.load(Ordering::Acquire) {
                break;
            }
            let chunk = reader.read()?;
            if chunk.is_empty() {
                thread::sleep(Duration::from_millis(2));
            } else {
                processor.push_interleaved(chunk)?;
            }
        }
        Ok(())
    })();
    reader.close();
    drop(stream);
    // OwnedStream's destructor closed admission and awaited all publications,
    // including error paths. Only now may the final DSP drain begin.
    processing?;
    loop {
        let chunk = reader.read()?;
        if chunk.is_empty() {
            break;
        }
        processor.push_interleaved(chunk)?;
    }
    processor.finish().map_err(CaptureError::Audio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saturated_event_consumer_cannot_block_the_microphone_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let (sender, _receiver) = crossbeam_channel::bounded(1);
        send_event(&sender, CaptureEvent::MaximumWarning { session: 1 })?;
        let started = Instant::now();
        assert!(matches!(
            send_event(&sender, CaptureEvent::MaximumWarning { session: 1 }),
            Err(CaptureError::EventOverflow)
        ));
        assert!(started.elapsed() < Duration::from_millis(100));
        Ok(())
    }

    #[test]
    fn dropping_a_capture_handle_requests_stop_without_waiting_for_its_worker()
    -> Result<(), Box<dyn std::error::Error>> {
        let stop = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&stop);
        let (finished, done) = crossbeam_channel::bounded(1);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !observed.load(Ordering::Acquire) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            finished
                .send(observed.load(Ordering::Acquire))
                .map_err(|_| CaptureError::ControllerGone)
        });
        let (_events, incoming) = crossbeam_channel::bounded(3);
        let handle = CaptureHandle {
            stop,
            worker: Some(worker),
            events: incoming,
        };
        let started = Instant::now();
        drop(handle);
        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(done.recv_timeout(Duration::from_millis(500))?);
        Ok(())
    }

    #[test]
    fn automatic_sample_limit_closes_on_a_whole_frame_without_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut port, mut reader) = capture_buffer(AudioFormat {
            sample_rate: 8_000,
            channels: 1,
        })?;
        let second = vec![0.1_f32; 8_000];
        let mut accepted = 0;
        for _ in 0..299 {
            port.publish(&second);
            loop {
                let chunk = reader.read()?;
                if chunk.is_empty() {
                    break;
                }
                accepted += chunk.len();
            }
        }
        assert!(!reader.maximum_reached());
        port.publish(&vec![0.2_f32; 10_000]);
        assert!(reader.maximum_reached());
        assert!(reader.publication_finished());
        port.publish(&second);
        loop {
            let chunk = reader.read()?;
            if chunk.is_empty() {
                break;
            }
            assert!(chunk.iter().all(|sample| *sample == 0.2));
            accepted += chunk.len();
        }
        assert_eq!(accepted, 300 * 8_000);
        Ok(())
    }

    #[test]
    fn callback_publishes_complete_frames_and_closing_preserves_accepted_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = dictation_core::audio::AudioFormat {
            sample_rate: 16_000,
            channels: 2,
        };
        let (mut port, mut reader) = capture_buffer(format)?;
        port.publish(&[i16::MIN, i16::MAX, 0, 0]);
        reader.close();
        port.publish(&[32_000_i16, 32_000, 32_000, 32_000]);
        assert_eq!(reader.read()?, &[-1.0, 32_767.0 / 32_768.0, 0.0, 0.0]);
        assert!(reader.read()?.is_empty());
        assert!(reader.publication_finished());
        Ok(())
    }

    #[test]
    fn overflow_invalid_samples_and_partial_frames_invalidate_the_entire_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let format = AudioFormat {
            sample_rate: 16_000,
            channels: 2,
        };
        let (mut port, mut reader) = capture_buffer(format)?;
        port.publish(&vec![0.1_f32; 32_002]);
        assert!(matches!(reader.read(), Err(CaptureError::Overflow)));
        let (mut port, mut reader) = capture_buffer(format)?;
        port.publish(&[f32::NAN, 0.1]);
        assert!(matches!(reader.read(), Err(CaptureError::InvalidSamples)));
        let (mut port, mut reader) = capture_buffer(format)?;
        port.publish(&[0.1_f32]);
        assert!(matches!(reader.read(), Err(CaptureError::IncompleteFrames)));
        Ok(())
    }

    #[test]
    fn wrapped_chunks_remain_in_order_and_frames_remain_whole()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut port, mut reader) = capture_buffer(AudioFormat {
            sample_rate: 8_000,
            channels: 1,
        })?;
        port.publish(&vec![0.1_f32; 7_000]);
        let mut output = Vec::new();
        loop {
            let chunk = reader.read()?;
            if chunk.is_empty() {
                break;
            }
            output.extend_from_slice(chunk);
        }
        port.publish(&vec![0.2_f32; 7_000]);
        reader.close();
        loop {
            let chunk = reader.read()?;
            if chunk.is_empty() {
                break;
            }
            output.extend_from_slice(chunk);
        }
        assert_eq!(output.len(), 14_000);
        assert!(output[..7_000].iter().all(|sample| *sample == 0.1));
        assert!(output[7_000..].iter().all(|sample| *sample == 0.2));
        Ok(())
    }
}
