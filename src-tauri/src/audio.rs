//! Native microphone owner and a preallocated callback-to-DSP boundary.

use cpal::{
    FromSample, Sample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use dictation_core::{
    audio::{AudioError, AudioFormat, AudioProcessor, MAX_SECONDS},
    recording_limits::{LimitDecision, RecordingLimits},
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};
use std::{
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const CLOSED: usize = 1 << (usize::BITS - 1);

mod full_prefix;
#[cfg(feature = "measurement")]
pub mod pretranscription;
#[cfg(feature = "measurement")]
pub mod replay;

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
    energy: f64,
    measured_samples: u64,
    elapsed_millis: u64,
}

/// Latest input RMS, linear full-scale amplitude, rounded to 0..=1000.
/// Duration is real owner elapsed time, capped at the recording limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureProgress {
    pub session: u64,
    pub elapsed_millis: u64,
    pub level_permille: u16,
}

/// One replaceable value, independent of the reserved lifecycle-event channel.
/// Only the owner/controller use this lock; never the realtime callback.
#[derive(Default)]
pub(crate) struct ProgressSlot(Mutex<Option<CaptureProgress>>);
impl ProgressSlot {
    pub(crate) fn publish(&self, value: CaptureProgress) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);
    }
    pub(crate) fn take(&self) -> Option<CaptureProgress> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
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
            energy: 0.0,
            measured_samples: 0,
            elapsed_millis: 0,
        },
    ))
}

impl CapturePort {
    /// The same bounded error callback is used by native streams and connector tests.
    pub(crate) fn stream_error_callback(&self) -> impl FnMut(cpal::StreamError) + Send + 'static {
        let gate = Arc::clone(&self.gate);
        move |_| {
            gate.error.store(4, Ordering::Release);
        }
    }

    /// Called only by the native callback. No allocation, locks, logs or DSP.
    /// A complete callback is published atomically or the entire session fails.
    pub fn publish<T: Sample + Copy>(&mut self, samples: &[T])
    where
        f32: FromSample<T>,
    {
        let Some(_publication) = self.gate.enter() else {
            return;
        };
        Self::publish_admitted(
            &mut self.producer,
            self.channels,
            &self.gate,
            &mut self.remaining_frames,
            samples,
        );
    }

    /// Enter before the bounded developer-clock wait. A closed gate never
    /// admits a new reservation; an admitted chunk participates in the exact
    /// same owner close barrier as a native callback already in progress.
    #[cfg(feature = "measurement")]
    fn reserve_replay(&mut self) -> Option<ReplayPublication<'_>> {
        let publication = self.gate.enter()?;
        Some(ReplayPublication {
            producer: &mut self.producer,
            channels: self.channels,
            remaining_frames: &mut self.remaining_frames,
            publication,
        })
    }

    fn publish_admitted<T: Sample + Copy>(
        producer: &mut Producer<f32>,
        channels: usize,
        gate: &Gate,
        remaining_frames: &mut usize,
        samples: &[T],
    ) where
        f32: FromSample<T>,
    {
        if !samples.len().is_multiple_of(channels) {
            gate.error.store(2, Ordering::Release);
            return;
        }
        let published = samples.len().min(*remaining_frames * channels);
        let Ok(mut chunk) = producer.write_chunk(published) else {
            gate.error.store(1, Ordering::Release);
            return;
        };
        let (first, second) = chunk.as_mut_slices();
        for (out, input) in first.iter_mut().chain(second.iter_mut()).zip(samples) {
            let normalized = input.to_sample::<f32>();
            if !normalized.is_finite() {
                gate.error.store(3, Ordering::Release);
                return;
            }
            *out = normalized;
        }
        chunk.commit_all();
        *remaining_frames -= published / channels;
        if *remaining_frames == 0 {
            gate.maximum.store(true, Ordering::Release);
            gate.publications.fetch_or(CLOSED, Ordering::AcqRel);
        }
    }
}

#[cfg(feature = "measurement")]
struct ReplayPublication<'a> {
    producer: &'a mut Producer<f32>,
    channels: usize,
    remaining_frames: &'a mut usize,
    publication: Publication<'a>,
}
#[cfg(feature = "measurement")]
impl ReplayPublication<'_> {
    fn publish(self, samples: &[f32]) {
        CapturePort::publish_admitted(
            self.producer,
            self.channels,
            self.publication.0,
            self.remaining_frames,
            samples,
        );
        // Publication's Drop releases the close barrier, including on unwind.
    }
}

impl CaptureReader {
    /// Measure accepted interleaved samples since the previous call. An empty
    /// interval has zero level; channels contribute equally, before downmixing.
    /// Supply time since stream start from the owner, never a UI clock.
    pub fn progress(&mut self, session: u64, elapsed: Duration) -> CaptureProgress {
        self.elapsed_millis = self
            .elapsed_millis
            .max(elapsed.as_millis().min(300_000) as u64);
        let rms = if self.measured_samples == 0 {
            0.0
        } else {
            (self.energy / self.measured_samples as f64).sqrt()
        };
        self.energy = 0.0;
        self.measured_samples = 0;
        CaptureProgress {
            session,
            elapsed_millis: self.elapsed_millis,
            level_permille: (rms.min(1.0) * 1_000.0).round() as u16,
        }
    }

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
        self.energy += self
            .scratch
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum::<f64>();
        self.measured_samples += self.scratch.len() as u64;
        Ok(&self.scratch)
    }
}

pub struct SealedPrefix {
    pub timeline_range: std::ops::Range<usize>,
    pub samples: Arc<Vec<f32>>,
}

pub enum CaptureEvent {
    Segment {
        session: u64,
        prefix: SealedPrefix,
    },
    #[cfg(feature = "measurement")]
    SegmentedFinished {
        session: u64,
        base: usize,
        result: Result<Vec<f32>, CaptureError>,
        maximum: bool,
    },
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

impl CaptureEvent {
    pub(crate) fn is_finished(&self) -> bool {
        match self {
            Self::Finished { .. } => true,
            #[cfg(feature = "measurement")]
            Self::SegmentedFinished { .. } => true,
            _ => false,
        }
    }
    pub(crate) fn result_mut(&mut self) -> Option<&mut Result<Vec<f32>, CaptureError>> {
        match self {
            Self::Finished { result, .. } => Some(result),
            #[cfg(feature = "measurement")]
            Self::SegmentedFinished { result, .. } => Some(result),
            _ => None,
        }
    }
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
    progress: Arc<ProgressSlot>,
    #[cfg(feature = "measurement")]
    segmentation: Option<Arc<pretranscription::SegmentLink>>,
    full_prefix: Option<Arc<full_prefix::PrefixLink>>,
    session: u64,
}

impl CaptureHandle {
    pub fn start(session: u64, device_name: Option<String>) -> Result<Self, CaptureError> {
        Self::start_with_policy(session, device_name, crate::pipeline::Policy::Reference)
    }

    pub(crate) fn start_with_policy(
        session: u64,
        device_name: Option<String>,
        policy: crate::pipeline::Policy,
    ) -> Result<Self, CaptureError> {
        Self::spawn_for_policy(
            session,
            policy,
            move |events, stop, maximum, progress, link| {
                record(
                    session,
                    device_name.as_deref(),
                    events,
                    stop,
                    maximum,
                    progress,
                    link,
                )
            },
        )
    }

    fn spawn_for_policy(
        session: u64,
        policy: crate::pipeline::Policy,
        run: impl FnOnce(
            &Sender<CaptureEvent>,
            &AtomicBool,
            &mut bool,
            &ProgressSlot,
            Option<&full_prefix::PrefixLink>,
        ) -> Result<Vec<f32>, CaptureError>
        + Send
        + 'static,
    ) -> Result<Self, CaptureError> {
        let link = policy
            .is_full_prefix()
            .then(|| Arc::new(full_prefix::PrefixLink::default()));
        let owner_link = link.clone();
        let mut handle = Self::spawn(session, move |events, stop, maximum, progress| {
            run(events, stop, maximum, progress, owner_link.as_deref())
        })?;
        handle.full_prefix = link;
        Ok(handle)
    }

    /// Controlled source into the same owner/policy selection as native capture.
    /// Only device construction is replaced; no CPAL or system input is started.
    #[cfg(test)]
    pub(crate) fn start_test_source(
        session: u64,
        format: AudioFormat,
        ready: Sender<CapturePort>,
        policy: crate::pipeline::Policy,
    ) -> Result<Self, CaptureError> {
        Self::spawn_for_policy(
            session,
            policy,
            move |events, stop, maximum, progress, link| {
                let processor = AudioProcessor::new(format)?;
                let (port, reader) = capture_buffer(format)?;
                RecordingOwner {
                    session,
                    events,
                    stop,
                    maximum,
                    progress,
                }
                .run_inner(
                    processor,
                    reader,
                    (),
                    Instant::now(),
                    move || {
                        ready
                            .try_send(port)
                            .map_err(|_| CaptureError::ControllerGone)
                    },
                    link.map(RecordingSegments::FullPrefix),
                )
            },
        )
    }

    /// Developer-only source: no device selection, permission request or pacing.
    /// The caller must provide a bounded readiness channel with available room.
    #[cfg(feature = "measurement")]
    pub fn start_replay(
        session: u64,
        format: AudioFormat,
        ready: Sender<replay::ReplayInput>,
    ) -> Result<Self, CaptureError> {
        if ready.capacity().is_none() {
            return Err(CaptureError::EventOverflow);
        }
        Self::spawn(session, move |events, stop, maximum, progress| {
            replay::record(session, format, ready, events, stop, maximum, progress)
        })
    }

    #[cfg(feature = "measurement")]
    pub fn start_pretranscription_replay(
        session: u64,
        format: AudioFormat,
        ready: Sender<replay::ReplayInput>,
    ) -> Result<Self, CaptureError> {
        if ready.capacity().is_none() {
            return Err(CaptureError::EventOverflow);
        }
        let link = Arc::new(pretranscription::SegmentLink::default());
        let owner_link = Arc::clone(&link);
        let mut handle = Self::spawn(session, move |events, stop, maximum, progress| {
            replay::record_segmented(
                RecordingOwner {
                    session,
                    events,
                    stop,
                    maximum,
                    progress,
                },
                format,
                ready,
                &owner_link,
            )
        })?;
        handle.segmentation = Some(link);
        Ok(handle)
    }

    /// Whole-prefix experiment: keep the active journal intact until final flush.
    #[cfg(feature = "measurement")]
    pub fn start_full_prefix_replay(
        session: u64,
        format: AudioFormat,
        ready: Sender<replay::ReplayInput>,
    ) -> Result<Self, CaptureError> {
        if ready.capacity().is_none() {
            return Err(CaptureError::EventOverflow);
        }
        let link = Arc::new(full_prefix::PrefixLink::default());
        let owner_link = Arc::clone(&link);
        let mut handle = Self::spawn(session, move |events, stop, maximum, progress| {
            full_prefix::record(
                RecordingOwner {
                    session,
                    events,
                    stop,
                    maximum,
                    progress,
                },
                format,
                ready,
                &owner_link,
            )
        })?;
        handle.full_prefix = Some(link);
        Ok(handle)
    }

    pub(crate) fn allow_seal(&self, epoch: u64) {
        #[cfg(feature = "measurement")]
        if let Some(link) = &self.segmentation {
            link.allow(epoch);
        }
        if let Some(link) = &self.full_prefix {
            link.allow(epoch);
        }
    }

    fn spawn(
        session: u64,
        run: impl FnOnce(
            &Sender<CaptureEvent>,
            &AtomicBool,
            &mut bool,
            &ProgressSlot,
        ) -> Result<Vec<f32>, CaptureError>
        + Send
        + 'static,
    ) -> Result<Self, CaptureError> {
        // Exactly one Started, at most one Warning, exactly one Finished.
        // Progress and replay readiness never occupy these three reserved slots.
        let (events, incoming) = crossbeam_channel::bounded(3);
        let stop = Arc::new(AtomicBool::new(false));
        let requested = Arc::clone(&stop);
        let progress = Arc::new(ProgressSlot::default());
        let published = Arc::clone(&progress);
        let worker = thread::Builder::new()
            .name("microphone-dsp".into())
            .spawn(move || {
                let mut maximum = false;
                let result = run(&events, &requested, &mut maximum, &published);
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
            progress,
            #[cfg(feature = "measurement")]
            segmentation: None,
            full_prefix: None,
            session,
        })
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn next_event(&self) -> Option<CaptureEvent> {
        // Disconnection has no additional event; join retains worker errors.
        #[cfg(feature = "measurement")]
        if let Some(link) = &self.segmentation {
            #[cfg(test)]
            if !link.pause_poll() {
                return None;
            }
            return link.poll(self.session, &self.events);
        }
        if let Some(link) = &self.full_prefix {
            return link.poll(self.session, &self.events);
        }
        self.events.try_recv().ok()
    }

    pub fn take_progress(&self) -> Option<CaptureProgress> {
        self.progress.take()
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
    // CPAL input_devices probes supported formats by constructing AudioUnits.
    // Verify input presence through read-only properties, then select from the
    // unfiltered device-ID iterator. Ambiguous names remain fail-closed.
    if !crate::devices::input_devices()
        .map_err(CaptureError::Native)?
        .iter()
        .any(|device| device.name == name && device.available)
    {
        return Err(CaptureError::DeviceUnavailable);
    }
    let mut matching = host
        .devices()
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
    let failure = port.stream_error_callback();
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| port.publish(samples),
            failure,
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
    progress: &ProgressSlot,
    full_prefix: Option<&full_prefix::PrefixLink>,
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
    let processor = AudioProcessor::new(format)?;
    let (port, reader) = capture_buffer(format)?;
    let stream = OwnedStream {
        stream: Some(build_stream(&device, &config, port)?),
        gate: Arc::clone(&reader.gate),
    };
    stream.play()?;
    let started = Instant::now();
    let owner = RecordingOwner {
        session,
        events,
        stop,
        maximum,
        progress,
    };
    if let Some(link) = full_prefix {
        owner.run_inner(
            processor,
            reader,
            stream,
            started,
            || Ok(()),
            Some(RecordingSegments::FullPrefix(link)),
        )
    } else {
        owner.run(processor, reader, stream, started, || Ok(()))
    }
}

struct RecordingOwner<'a> {
    session: u64,
    events: &'a Sender<CaptureEvent>,
    stop: &'a AtomicBool,
    maximum: &'a mut bool,
    progress: &'a ProgressSlot,
}

/// Keeps publication admission and source lifetime paired even during unwind.
/// Native OwnedStream retains its independent setup-time RAII protection.
struct RecordingSource<S> {
    source: Option<S>,
    gate: Arc<Gate>,
}

impl<S> Drop for RecordingSource<S> {
    fn drop(&mut self) {
        self.gate.publications.fetch_or(CLOSED, Ordering::AcqRel);
        drop(self.source.take());
        while self.gate.publications.load(Ordering::Acquire) != CLOSED {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[derive(Clone, Copy)]
enum RecordingSegments<'a> {
    #[cfg(feature = "measurement")]
    Suffix(&'a pretranscription::SegmentLink),
    FullPrefix(&'a full_prefix::PrefixLink),
}
impl RecordingSegments<'_> {
    fn advance(
        self,
        processor: &mut AudioProcessor,
        pauses: &mut dictation_core::audio::pauses::ConfirmedPauses,
    ) -> Result<(), CaptureError> {
        match self {
            #[cfg(feature = "measurement")]
            Self::Suffix(link) => link.advance(processor, pauses),
            Self::FullPrefix(link) => link.advance(processor, pauses),
        }
    }
}

impl RecordingOwner<'_> {
    fn run<S>(
        self,
        processor: AudioProcessor,
        reader: CaptureReader,
        source: S,
        started: Instant,
        activate: impl FnOnce() -> Result<(), CaptureError>,
    ) -> Result<Vec<f32>, CaptureError> {
        self.run_inner(processor, reader, source, started, activate, None)
    }

    fn run_inner<S>(
        self,
        mut processor: AudioProcessor,
        mut reader: CaptureReader,
        source: S,
        started: Instant,
        activate: impl FnOnce() -> Result<(), CaptureError>,
        segmentation: Option<RecordingSegments<'_>>,
    ) -> Result<Vec<f32>, CaptureError> {
        let mut pauses = dictation_core::audio::pauses::ConfirmedPauses::default();
        let source = RecordingSource {
            source: Some(source),
            gate: Arc::clone(&reader.gate),
        };
        let Self {
            session,
            events,
            stop,
            maximum,
            progress,
        } = self;
        let mut limits = RecordingLimits::default();
        let mut next_progress = Duration::from_millis(50);
        let processing = (|| -> Result<(), CaptureError> {
            send_event(
                events,
                CaptureEvent::Started {
                    session,
                    format: reader.format(),
                    started,
                },
            )?;
            activate()?;
            loop {
                match limits.advance(started.elapsed(), reader.maximum_reached()) {
                    LimitDecision::Continue => {}
                    LimitDecision::Warn => {
                        send_event(events, CaptureEvent::MaximumWarning { session })?
                    }
                    LimitDecision::Stop => {
                        *maximum = true;
                        break;
                    }
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
                if let Some(link) = segmentation {
                    link.advance(&mut processor, &mut pauses)?;
                }
                let elapsed = started.elapsed();
                if elapsed >= next_progress {
                    progress.publish(reader.progress(session, elapsed));
                    next_progress = elapsed + Duration::from_millis(50);
                }
            }
            Ok(())
        })();
        reader.close();
        drop(source);
        // Close admission, close the source, then await every publication. Native
        // OwnedStream retains its own RAII barrier, also during setup failures.
        // Replay closes admission even if its external producer keeps its port.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_measures_accepted_energy_and_monotone_elapsed_time()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut port, mut reader) = capture_buffer(AudioFormat {
            sample_rate: 16_000,
            channels: 2,
        })?;
        port.publish(&[0.3_f32, 0.4, -0.3, -0.4]);
        assert_eq!(reader.read()?.len(), 4);
        let measured = reader.progress(7, Duration::from_millis(120));
        assert_eq!(measured.session, 7);
        assert_eq!(measured.elapsed_millis, 120);
        assert_eq!(measured.level_permille, 354);
        port.publish(&[f32::MAX, -f32::MAX]);
        reader.read()?;
        let clipped = reader.progress(7, Duration::from_millis(119));
        assert_eq!(clipped.elapsed_millis, 120);
        assert_eq!(clipped.level_permille, 1_000);
        assert_eq!(
            reader
                .progress(7, Duration::from_millis(150))
                .level_permille,
            0
        );
        assert_eq!(
            reader.progress(7, Duration::from_secs(301)).elapsed_millis,
            300_000
        );
        assert_eq!(reader.progress(7, Duration::ZERO).elapsed_millis, 300_000);
        Ok(())
    }

    #[test]
    fn progress_flood_does_not_use_lifecycle_capacity_or_hide_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let (events, incoming) = crossbeam_channel::bounded(3);
        let stop = Arc::new(AtomicBool::new(false));
        let handle = CaptureHandle {
            stop: Arc::clone(&stop),
            worker: None,
            events: incoming,
            progress: Arc::new(ProgressSlot::default()),
            #[cfg(feature = "measurement")]
            segmentation: None,
            full_prefix: None,
            session: 4,
        };
        send_event(
            &events,
            CaptureEvent::Started {
                session: 4,
                format: AudioFormat {
                    sample_rate: 16_000,
                    channels: 1,
                },
                started: Instant::now(),
            },
        )?;
        send_event(&events, CaptureEvent::MaximumWarning { session: 4 })?;
        send_event(
            &events,
            CaptureEvent::Finished {
                session: 4,
                result: Ok(vec![0.25]),
                maximum: false,
            },
        )?;
        for elapsed_millis in 0..10_000 {
            handle.progress.publish(CaptureProgress {
                session: 4,
                elapsed_millis,
                level_permille: 250,
            });
        }
        handle.request_stop();
        assert!(stop.load(Ordering::Acquire));
        assert_eq!(
            handle.take_progress().map(|value| value.elapsed_millis),
            Some(9_999)
        );
        assert!(handle.take_progress().is_none());
        assert!(matches!(
            handle.next_event(),
            Some(CaptureEvent::Started { session: 4, .. })
        ));
        assert!(matches!(
            handle.next_event(),
            Some(CaptureEvent::MaximumWarning { session: 4 })
        ));
        assert!(
            matches!(handle.next_event(), Some(CaptureEvent::Finished { session: 4, result: Ok(audio), .. }) if audio == vec![0.25])
        );
        assert!(handle.next_event().is_none());
        Ok(())
    }

    #[test]
    fn every_supported_ring_holds_exactly_one_second_of_complete_frames()
    -> Result<(), Box<dyn std::error::Error>> {
        for sample_rate in [8_000, 16_000, 24_000, 32_000, 44_100, 48_000] {
            for channels in [1, 2, 8] {
                let format = AudioFormat {
                    sample_rate,
                    channels,
                };
                let second = vec![0.25_f32; sample_rate as usize * usize::from(channels)];
                let (mut port, mut reader) = capture_buffer(format)?;
                port.publish(&second);
                reader.close();
                let mut accepted = 0;
                loop {
                    let chunk = reader.read()?;
                    if chunk.is_empty() {
                        break;
                    }
                    assert!(chunk.len().is_multiple_of(usize::from(channels)));
                    assert!(chunk.iter().all(|sample| *sample == 0.25));
                    accepted += chunk.len();
                }
                assert_eq!(accepted, second.len());

                let (mut port, mut reader) = capture_buffer(format)?;
                port.publish(&second);
                port.publish(&vec![0.5_f32; usize::from(channels)]);
                assert!(matches!(reader.read(), Err(CaptureError::Overflow)));
            }
        }
        Ok(())
    }

    #[test]
    fn one_hundred_short_capture_lifetimes_drain_or_discard_without_cross_session_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        // Exercise the real ring/DSP ownership boundary without opening a device.
        // Keep at most one previous result, as an in-flight decoder may do.
        // This checks payload capacities and isolation, not process RSS or CPAL cleanup.
        let mut previous: Option<Vec<f32>> = None;
        let rates = [8_000, 16_000, 24_000, 32_000, 44_100, 48_000];
        for session in 0..100 {
            let rate = rates[session % rates.len()];
            let channels = [1, 2, 8][(session / rates.len()) % 3];
            let format = AudioFormat {
                sample_rate: rate,
                channels,
            };
            let (mut port, mut reader) = capture_buffer(format)?;
            let mut processor = AudioProcessor::new(format)?;
            let amplitude = if session % 2 == 0 { 0.25 } else { -0.5 };
            let frames = rate as usize / 10 + 7;
            let input = vec![amplitude; frames * usize::from(channels)];
            for chunk in input.chunks(137 * usize::from(channels)) {
                port.publish(chunk);
                // Deliberately leave the last publication for the Stop drain.
                if chunk.len() == 137 * usize::from(channels) {
                    processor.push_interleaved(reader.read()?)?;
                }
            }
            reader.close();
            assert!(reader.publication_finished());
            if session % 3 == 0 {
                // Cancel discards both accepted ring data and partially processed data.
                drop(processor);
                continue;
            }
            loop {
                let chunk = reader.read()?;
                if chunk.is_empty() {
                    break;
                }
                processor.push_interleaved(chunk)?;
            }
            let output = processor.finish()?;
            assert_eq!(
                output.len(),
                (frames * 16_000 + rate as usize / 2) / rate as usize
            );
            assert!(
                output[600..1_000]
                    .iter()
                    .all(|sample| (*sample - amplitude).abs() < 0.001),
                "stale audio or filter state in session {session}, rate {rate}"
            );
            // Vec capacity is observable on the transferred journal, not inferred
            // from its short length. No geometrically growing history is retained.
            assert_eq!(output.capacity(), 4_800_000);
            if let Some(old) = &previous {
                assert_eq!(old.capacity(), 4_800_000);
            }
            previous = Some(output);
        }
        Ok(())
    }

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
            progress: Arc::new(ProgressSlot::default()),
            #[cfg(feature = "measurement")]
            segmentation: None,
            full_prefix: None,
            session: 4,
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
