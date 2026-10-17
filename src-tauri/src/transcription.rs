//! One serial owner, warm ASR/VAD weights, and fresh ASR state per inference.

use crate::model::{ModelDefinition, ModelError, verify_file};
use dictation_core::audio::{EnergyProfile, MAX_SAMPLES, speech_range};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

mod speech_gate;

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
    #[error("Voice activity detection failed")]
    Vad,
    #[error("Voice activity model could not be verified: {0}")]
    VadModel(#[source] ModelError),
    #[error("Insufficient memory for bounded inference buffers")]
    Memory,
}

pub struct Engine {
    context: WhisperContext,
    gate: speech_gate::SpeechGate,
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
        let vad_path = path
            .parent()
            .ok_or(InferenceError::ModelPath)?
            .join(crate::model::VAD.file_name());
        let gate = speech_gate::SpeechGate::load(&vad_path)?;
        Ok(Self { context, gate })
    }

    /// Call only on the serial model worker. ASR state is fresh, VAD resets per call.
    /// No abort callback; the complete worker operation may finish after revocation.
    /// The language is fixed, with neither translation nor text postprocessing.
    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
    ) -> Result<String, InferenceError> {
        let Self { context, gate } = self;
        transcribe_with(samples, language, gate, |input, language| {
            decode(context, input, language)
        })
    }
}

fn transcribe_with<D: speech_gate::Detector>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    decode: impl FnOnce(&[f32], Language) -> Result<String, InferenceError>,
) -> Result<String, InferenceError> {
    if samples.len() > MAX_SAMPLES || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(InferenceError::InvalidAudio);
    }
    let Some(range) = speech_range(samples, EnergyProfile::default()) else {
        return Ok(String::new());
    };
    if !gate.allows(samples)? {
        return Ok(String::new());
    }
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
    decode(input, language)
}

fn decode(
    context: &WhisperContext,
    input: &[f32],
    language: Language,
) -> Result<String, InferenceError> {
    let mut state = context
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

#[cfg(test)]
mod tests {
    use super::*;
    use speech_gate::{Detector, SpeechGate};

    struct FailedDetector;
    impl Detector for FailedDetector {
        fn probabilities(&mut self, _: &[f32]) -> Result<&[f32], InferenceError> {
            Err(InferenceError::Vad)
        }
    }

    #[test]
    fn gate_failure_never_invokes_asr_or_reports_silence() {
        let mut gate = SpeechGate::from_detector(FailedDetector);
        let mut called = false;
        let result = transcribe_with(&[0.1; 3200], Language::German, &mut gate, |_, _| {
            called = true;
            Ok("unexpected".into())
        });
        assert!(matches!(result, Err(InferenceError::Vad)));
        assert!(!called);
    }

    struct RecordingDetector<'a> {
        scores: &'a [f32],
        observed: &'a mut Vec<f32>,
    }
    impl Detector for RecordingDetector<'_> {
        fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
            self.observed.clear();
            self.observed.extend_from_slice(samples);
            Ok(self.scores)
        }
    }

    #[test]
    fn negative_whole_recording_returns_empty_without_asr() -> Result<(), InferenceError> {
        let mut observed = Vec::new();
        let mut gate = SpeechGate::from_detector(RecordingDetector {
            scores: &[0.1; 7],
            observed: &mut observed,
        });
        let mut called = false;
        let text = transcribe_with(&[0.01; 3200], Language::German, &mut gate, |_, _| {
            called = true;
            Ok("unexpected".into())
        })?;
        assert!(text.is_empty());
        assert!(!called);
        assert_eq!(observed.len(), 3200);
        Ok(())
    }

    #[test]
    fn quiet_short_speech_uses_normalized_whole_audio_but_asr_keeps_original_and_padding()
    -> Result<(), InferenceError> {
        let mut samples = vec![0.0001; 3200];
        samples[3199] = -0.0001;
        let original = samples.clone();
        let mut observed = Vec::new();
        let mut gate = SpeechGate::from_detector(RecordingDetector {
            // Only the final 128-sample partial window admits this recording.
            scores: &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5],
            observed: &mut observed,
        });
        let text = transcribe_with(&samples, Language::English, &mut gate, |input, language| {
            assert_eq!(language, Language::English);
            assert_eq!(input.len(), 16000);
            assert_eq!(&input[..3200], original.as_slice());
            assert!(input[3200..].iter().all(|v| *v == 0.0));
            Ok("preserved text".into())
        })?;
        assert_eq!(text, "preserved text");
        assert_eq!(samples, original);
        assert_eq!(observed.len(), 3200);
        assert!(observed[..3199].iter().all(|v| *v == 0.95));
        assert_eq!(observed[3199], -0.95);
        Ok(())
    }

    #[test]
    fn invalid_scores_never_invoke_asr_and_invalid_pcm_never_reaches_detector() {
        for scores in [
            vec![],
            vec![0.5; 6],
            vec![0.5; 8],
            vec![f32::NAN; 7],
            vec![1.1; 7],
            vec![-0.1; 7],
        ] {
            let mut observed = Vec::new();
            let mut gate = SpeechGate::from_detector(RecordingDetector {
                scores: &scores,
                observed: &mut observed,
            });
            let mut called = false;
            assert!(matches!(
                transcribe_with(&[0.1; 3200], Language::German, &mut gate, |_, _| {
                    called = true;
                    Ok(String::new())
                }),
                Err(InferenceError::Vad)
            ));
            assert!(!called);
        }
        for samples in [
            vec![f32::NAN],
            vec![f32::INFINITY],
            vec![0.1; MAX_SAMPLES + 1],
        ] {
            let mut observed = Vec::new();
            let mut gate = SpeechGate::from_detector(RecordingDetector {
                scores: &[0.9],
                observed: &mut observed,
            });
            assert!(matches!(
                transcribe_with(&samples, Language::German, &mut gate, |_, _| Ok(
                    String::new()
                )),
                Err(InferenceError::InvalidAudio)
            ));
            assert!(observed.is_empty());
        }
    }

    #[test]
    fn accepted_long_audio_reaches_asr_without_a_copy_or_vad_scaling() -> Result<(), InferenceError>
    {
        let samples = vec![0.02; 32000];
        let mut observed = Vec::new();
        let mut probabilities = vec![0.0; 63];
        probabilities[62] = 0.8;
        let mut gate = SpeechGate::from_detector(RecordingDetector {
            scores: &probabilities,
            observed: &mut observed,
        });
        assert_eq!(
            transcribe_with(&samples, Language::German, &mut gate, |audio, _| {
                assert!(std::ptr::eq(audio.as_ptr(), samples.as_ptr()));
                assert_eq!(audio, samples.as_slice());
                Ok("original input".into())
            })?,
            "original input"
        );
        assert_eq!(observed.len(), 32000);
        assert!(observed.iter().all(|v| *v == 0.95));
        Ok(())
    }
}
