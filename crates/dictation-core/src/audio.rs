//! Continuous downmix/resampling. Only the DSP worker calls this interface.

use rubato::audioadapter_buffers::direct::{InterleavedSlice, SequentialSliceOfVecs};
use rubato::{Fft, FixedSync, Resampler, WindowFunction};

pub mod pauses;

pub const OUTPUT_RATE: usize = 16_000;
pub const MAX_SECONDS: usize = 300;
pub const MAX_SAMPLES: usize = OUTPUT_RATE * MAX_SECONDS;
const CHUNK: usize = 1_024;

/// Provisional calibration values, not a semantic voice detector.
#[derive(Debug, Clone, Copy)]
pub struct EnergyProfile {
    start_rms: f32,
    end_rms: f32,
}

impl Default for EnergyProfile {
    fn default() -> Self {
        Self {
            // The previous 0.01 gate discarded fourteen calibration clips
            // before decoding. The weakest quiet clip peaks near 0.000105 RMS.
            // Keep a margin below that level; these remain signal-energy
            // thresholds, never a semantic claim that noise contains no voice.
            start_rms: 0.00005,
            end_rms: 0.000025,
        }
    }
}

/// End-of-recording energy range with 150-ms preroll and 200-ms tail.
/// No duration minimum: even a single energy-bearing frame remains eligible.
pub fn speech_range(samples: &[f32], profile: EnergyProfile) -> Option<std::ops::Range<usize>> {
    const FRAME: usize = 320;
    let rms = |frame: &[f32]| {
        (frame
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum::<f64>()
            / frame.len() as f64)
            .sqrt() as f32
    };
    let first = samples
        .chunks(FRAME)
        .position(|frame| rms(frame) >= profile.start_rms)?;
    let last = samples
        .chunks(FRAME)
        .enumerate()
        .filter(|(_, frame)| rms(frame) >= profile.end_rms)
        .map(|(index, _)| index)
        .next_back()?;
    Some((first * FRAME).saturating_sub(2_400)..((last + 1) * FRAME + 3_200).min(samples.len()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioFormat {
    pub fn validate(self) -> Result<(), AudioError> {
        if [8_000, 16_000, 24_000, 32_000, 44_100, 48_000].contains(&self.sample_rate)
            && (1..=8).contains(&self.channels)
        {
            Ok(())
        } else {
            Err(AudioError::UnsupportedFormat)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Unsupported microphone format")]
    UnsupportedFormat,
    #[error("Incomplete input frame")]
    IncompleteFrame,
    #[error("Non-finite microphone sample")]
    NonFinite,
    #[error("Maximum recording duration exceeded")]
    DurationLimit,
    #[error("Insufficient memory for bounded audio buffers")]
    Allocation,
    #[error("Resampler failed: {0}")]
    Resampler(String),
}

pub struct AudioProcessor {
    format: AudioFormat,
    resampler: Option<Fft<f32>>,
    pending: Vec<f32>,
    output: Vec<Vec<f32>>,
    journal: Vec<f32>,
    remaining_delay: usize,
    input_frames: usize,
    sealed_frames: usize,
}

impl AudioProcessor {
    pub fn new(format: AudioFormat) -> Result<Self, AudioError> {
        format.validate()?;
        let resampler = if format.sample_rate as usize == OUTPUT_RATE {
            None
        } else {
            Some(
                Fft::new_custom(
                    format.sample_rate as usize,
                    OUTPUT_RATE,
                    CHUNK,
                    2,
                    1,
                    WindowFunction::BlackmanHarris2,
                    FixedSync::Input,
                )
                .map_err(|error| AudioError::Resampler(error.to_string()))?,
            )
        };
        let remaining_delay = resampler.as_ref().map_or(0, Resampler::output_delay);
        let output = resampler.as_ref().map_or_else(Vec::new, |resampler| {
            vec![vec![0.0; resampler.output_frames_max()]]
        });
        let mut journal = Vec::new();
        journal
            .try_reserve_exact(MAX_SAMPLES)
            .map_err(|_| AudioError::Allocation)?;
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(CHUNK)
            .map_err(|_| AudioError::Allocation)?;
        Ok(Self {
            format,
            resampler,
            pending,
            output,
            journal,
            remaining_delay,
            input_frames: 0,
            sealed_frames: 0,
        })
    }

    /// Input must contain complete, finite, interleaved normalized frames.
    /// All storage is bounded; accepted frames are never silently discarded.
    pub fn push_interleaved(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let channels = usize::from(self.format.channels);
        if !samples.len().is_multiple_of(channels) {
            return Err(AudioError::IncompleteFrame);
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AudioError::NonFinite);
        }
        let frames = samples.len() / channels;
        if frames > self.format.sample_rate as usize * MAX_SECONDS - self.input_frames {
            return Err(AudioError::DurationLimit);
        }
        self.input_frames += frames;
        for frame in samples.chunks_exact(channels) {
            let mono = (frame.iter().map(|sample| f64::from(*sample)).sum::<f64>()
                / channels as f64) as f32;
            if self.resampler.is_none() {
                self.journal.push(mono);
            } else {
                self.pending.push(mono);
                if self.pending.len() == CHUNK {
                    self.process_block(MAX_SAMPLES)?;
                }
            }
        }
        Ok(())
    }

    /// Irreversibly stable output on the global recording timeline. Rounded
    /// final output may be shorter than resampler look-ahead, so never seal it.
    pub fn stable_samples(&self) -> (usize, &[f32]) {
        let stable_end = (self.input_frames * OUTPUT_RATE / self.format.sample_rate as usize)
            .min(self.sealed_frames + self.journal.len());
        (
            self.sealed_frames,
            &self.journal[..stable_end - self.sealed_frames],
        )
    }

    /// Move the existing journal into immutable storage, copying only its
    /// unsealed suffix. Caller must release the previous frozen storage first.
    pub fn seal_prefix(&mut self, global_cut: usize) -> Result<Vec<f32>, AudioError> {
        let (base, stable) = self.stable_samples();
        if global_cut <= base || global_cut > base + stable.len() {
            return Err(AudioError::IncompleteFrame);
        }
        let local_cut = global_cut - base;
        let mut suffix = Vec::new();
        suffix
            .try_reserve_exact(MAX_SAMPLES)
            .map_err(|_| AudioError::Allocation)?;
        suffix.extend_from_slice(&self.journal[local_cut..]);
        self.sealed_frames = global_cut;
        Ok(std::mem::replace(&mut self.journal, suffix))
    }

    fn process_block(&mut self, limit: usize) -> Result<(), AudioError> {
        let Some(resampler) = &mut self.resampler else {
            return Ok(());
        };
        let input = InterleavedSlice::new(&self.pending, 1, self.pending.len())
            .map_err(|error| AudioError::Resampler(error.to_string()))?;
        let output_frames = self.output[0].len();
        let mut output = SequentialSliceOfVecs::new_mut(&mut self.output, 1, output_frames)
            .map_err(|error| AudioError::Resampler(error.to_string()))?;
        let (_, produced) = resampler
            .process_into_buffer(&input, &mut output, None)
            .map_err(|error| AudioError::Resampler(error.to_string()))?;
        self.pending.clear();
        let skip = self.remaining_delay.min(produced);
        self.remaining_delay -= skip;
        let take =
            (produced - skip).min(limit.saturating_sub(self.sealed_frames + self.journal.len()));
        self.journal
            .extend_from_slice(&self.output[0][skip..skip + take]);
        Ok(())
    }

    /// Consume exactly once after callback publication has closed and drained.
    /// Delay correction and zero-flushing preserve the rounded signal duration.
    pub fn finish(mut self) -> Result<Vec<f32>, AudioError> {
        let expected = (self.input_frames * OUTPUT_RATE + self.format.sample_rate as usize / 2)
            / self.format.sample_rate as usize;
        if self.resampler.is_some() && expected > 0 {
            if !self.pending.is_empty() {
                self.pending.resize(CHUNK, 0.0);
                self.process_block(expected)?;
            }
            for _ in 0..8 {
                if self.sealed_frames + self.journal.len() >= expected {
                    break;
                }
                self.pending.resize(CHUNK, 0.0);
                self.process_block(expected)?;
            }
        }
        if self.sealed_frames + self.journal.len() < expected {
            return Err(AudioError::Resampler(
                "flush did not produce complete audio".into(),
            ));
        }
        self.journal.truncate(expected - self.sealed_frames);
        Ok(self.journal)
    }
}
