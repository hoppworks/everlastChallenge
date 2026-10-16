//! One owner, one warm context, and a fresh state for every complete inference.

use crate::model::{ModelDefinition, ModelError, verify_file};
use dictation_core::audio::{EnergyProfile, MAX_SAMPLES, speech_range};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    German,
    English,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::German => "de",
            Self::English => "en",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("Model path cannot be represented for the local backend")]
    ModelPath,
    #[error("The local multilingual model could not be initialized")]
    Context,
    #[error("Local transcription failed")]
    Decode,
    #[error("Invalid or oversized inference audio")]
    InvalidAudio,
    #[error("The backend returned invalid text")]
    InvalidText,
    #[error("The transcript exceeds the bounded output size")]
    OutputLimit,
    #[error("Insufficient memory for short-utterance padding")]
    Memory,
}

pub struct Engine {
    context: WhisperContext,
}

impl Engine {
    pub fn load(path: &Path, definition: ModelDefinition) -> Result<Self, InferenceError> {
        verify_file(path, definition)?;
        // Neither log_backend nor tracing_backend is enabled. Route backend
        // logging into the safe library hook instead of printing paths/audio.
        whisper_rs::install_logging_hooks();
        let context = WhisperContext::new_with_params(
            path.to_str().ok_or(InferenceError::ModelPath)?,
            WhisperContextParameters {
                use_gpu: true,
                flash_attn: false,
                ..Default::default()
            },
        )
        .map_err(|_| InferenceError::Context)?;
        if !context.is_multilingual() {
            return Err(InferenceError::Context);
        }
        Ok(Self { context })
    }

    /// Call only on the serial model worker. No abort callback or state reuse.
    /// The language is fixed, with neither translation nor text postprocessing.
    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
    ) -> Result<String, InferenceError> {
        if samples.len() > MAX_SAMPLES || samples.iter().any(|sample| !sample.is_finite()) {
            return Err(InferenceError::InvalidAudio);
        }
        let Some(range) = speech_range(samples, EnergyProfile::default()) else {
            return Ok(String::new());
        };
        let selected = &samples[range];
        let mut padded = Vec::new();
        let input = if selected.len() < 16_000 {
            padded
                .try_reserve_exact(16_000)
                .map_err(|_| InferenceError::Memory)?;
            padded.extend_from_slice(selected);
            padded.resize(16_000, 0.0);
            padded.as_slice()
        } else {
            selected
        };
        let mut state = self
            .context
            .create_state()
            .map_err(|_| InferenceError::Context)?;
        let mut parameters = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        parameters.set_n_threads(4);
        parameters.set_language(Some(language.code()));
        parameters.set_translate(false);
        parameters.set_detect_language(false);
        parameters.set_no_context(true);
        parameters.set_no_timestamps(true);
        parameters.set_print_special(false);
        parameters.set_print_progress(false);
        parameters.set_print_realtime(false);
        parameters.set_print_timestamps(false);
        parameters.set_suppress_nst(true);
        parameters.set_no_speech_thold(0.6);
        parameters.set_logprob_thold(-1.0);
        parameters.set_entropy_thold(2.4);
        state
            .full(parameters, input)
            .map_err(|_| InferenceError::Decode)?;
        let mut text = String::new();
        for segment in state.as_iter() {
            let segment = segment.to_str().map_err(|_| InferenceError::InvalidText)?;
            if text.len() + segment.len() > 256 * 1024 {
                return Err(InferenceError::OutputLimit);
            }
            text.push_str(segment);
        }
        Ok(text.trim().to_owned())
    }
}
