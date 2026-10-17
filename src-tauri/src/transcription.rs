//! One serial owner, warm ASR/VAD weights, and fresh ASR state per inference.

use crate::model::{ModelDefinition, ModelError, verify_file};
use dictation_core::audio::{EnergyProfile, MAX_SAMPLES, speech_range};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

mod speech_gate;

#[cfg(feature = "measurement")]
pub mod observation;

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
    #[cfg(feature = "measurement")]
    #[error("Invalid or oversized native transcription observation")]
    Observation,
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
            decode(context, input, language, &mut NoObservation)
        })
    }
    /// Developer-only accepted-segment observation on the same serial engine path.
    /// Does not expose encoder windows that produced no accepted segment.
    #[cfg(feature = "measurement")]
    pub fn transcribe_observed(
        &mut self,
        samples: &[f32],
        language: Language,
    ) -> Result<observation::ObservedTranscription, InferenceError> {
        let Self { context, gate } = self;
        let mut observation = observation::Observation::default();
        let text = transcribe_using(
            samples,
            language,
            gate,
            &mut observation,
            |input, language, observation| decode(context, input, language, observation),
        )?;
        Ok(observation::ObservedTranscription { text, observation })
    }
}

trait ObservationSink {
    fn prepared(&mut self, _: usize, _: Option<std::ops::Range<usize>>, _: usize) {}
    fn segment(
        &mut self,
        _: &whisper_rs::WhisperSegment<'_>,
        _: &str,
    ) -> Result<(), InferenceError> {
        Ok(())
    }
}
struct NoObservation;
impl ObservationSink for NoObservation {}

fn transcribe_with<D: speech_gate::Detector>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    decode: impl FnOnce(&[f32], Language) -> Result<String, InferenceError>,
) -> Result<String, InferenceError> {
    transcribe_using(
        samples,
        language,
        gate,
        &mut NoObservation,
        |input, language, _| decode(input, language),
    )
}

fn transcribe_using<D: speech_gate::Detector, O: ObservationSink>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    observation: &mut O,
    decode: impl FnOnce(&[f32], Language, &mut O) -> Result<String, InferenceError>,
) -> Result<String, InferenceError> {
    if samples.len() > MAX_SAMPLES || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(InferenceError::InvalidAudio);
    }
    let Some(range) = speech_range(samples, EnergyProfile::default()) else {
        observation.prepared(samples.len(), None, 0);
        return Ok(String::new());
    };
    if !gate.allows(samples)? {
        observation.prepared(samples.len(), Some(range), 0);
        return Ok(String::new());
    }
    let selected = &samples[range.clone()];
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
    observation.prepared(samples.len(), Some(range), input.len());
    decode(input, language, observation)
}

fn decode<O: ObservationSink>(
    context: &WhisperContext,
    input: &[f32],
    language: Language,
    observation: &mut O,
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
    parameters.set_suppress_nst(false);
    parameters.set_no_speech_thold(0.6);
    parameters.set_logprob_thold(-1.0);
    parameters.set_entropy_thold(2.4);
    state
        .full(parameters, input)
        .map_err(|_| InferenceError::Decode)?;
    let mut text = String::new();
    for segment in state.as_iter() {
        let segment_text = segment.to_str().map_err(|_| InferenceError::InvalidText)?;
        if text.len() + segment_text.len() > 256 * 1024 {
            return Err(InferenceError::OutputLimit);
        }
        observation.segment(&segment, segment_text)?;
        text.push_str(segment_text);
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
    #[cfg(feature = "measurement")]
    #[test]
    fn observed_and_normal_preparation_feed_identical_original_pcm_and_text()
    -> Result<(), InferenceError> {
        let mut samples = vec![0.0; 64000];
        samples[32000..35200].fill(0.02);
        let mut normal_pcm = Vec::new();
        let mut normalized = Vec::new();
        let scores = vec![0.9; 125];
        let mut gate = SpeechGate::from_detector(RecordingDetector {
            scores: &scores,
            observed: &mut normalized,
        });
        let normal = transcribe_with(&samples, Language::English, &mut gate, |input, language| {
            assert_eq!(language, Language::English);
            normal_pcm.extend_from_slice(input);
            Ok("unchanged full text".into())
        })?;
        let mut observation = observation::Observation::default();
        let observed = transcribe_using(
            &samples,
            Language::English,
            &mut gate,
            &mut observation,
            |input, language, _| {
                assert_eq!(language, Language::English);
                assert_eq!(input, normal_pcm.as_slice());
                assert_eq!(&input[..8800], &samples[29600..38400]);
                assert!(input[8800..].iter().all(|x| *x == 0.0));
                Ok("unchanged full text".into())
            },
        )?;
        assert_eq!(normal, observed);
        assert_eq!(observation.input_frames, 64000);
        assert_eq!(observation.selected_range, Some(29600..38400));
        assert_eq!(observation.selected_frames, 8800);
        assert_eq!(observation.padded_frames, 7200);
        assert_eq!(observation.asr_input_frames, 16000);
        Ok(())
    }

    #[cfg(feature = "measurement")]
    #[test]
    fn observed_gate_rejections_preserve_preparation_but_do_not_claim_asr()
    -> Result<(), InferenceError> {
        for (samples, expected_range) in
            [(vec![0.0; 3200], None), (vec![0.01; 3200], Some(0..3200))]
        {
            let mut normalized = Vec::new();
            let mut gate = SpeechGate::from_detector(RecordingDetector {
                scores: &[0.0; 7],
                observed: &mut normalized,
            });
            let mut observation = observation::Observation::default();
            let mut called = false;
            let result = transcribe_using(
                &samples,
                Language::German,
                &mut gate,
                &mut observation,
                |_, _, _| {
                    called = true;
                    Ok("must not decode".into())
                },
            )?;
            assert!(!called);
            assert!(result.is_empty());
            assert_eq!(observation.input_frames, 3200);
            assert_eq!(observation.selected_range, expected_range);
            assert_eq!(observation.asr_input_frames, 0);
            assert_eq!(observation.padded_frames, 0);
            assert!(observation.segments.is_empty());
        }
        Ok(())
    }

    #[cfg(feature = "measurement")]
    #[test]
    fn observation_and_gate_errors_propagate_instead_of_silence() {
        let mut normalized = Vec::new();
        let mut gate = SpeechGate::from_detector(RecordingDetector {
            scores: &[0.9; 7],
            observed: &mut normalized,
        });
        let mut observation = observation::Observation::default();
        let result = transcribe_using(
            &[0.01; 3200],
            Language::German,
            &mut gate,
            &mut observation,
            |_, _, observation| {
                observation.record(0, 0, 1, f32::NAN, "invalid native result")?;
                Ok(String::new())
            },
        );
        assert!(matches!(result, Err(InferenceError::Observation)));
        let mut failed_gate = SpeechGate::from_detector(FailedDetector);
        let result = transcribe_using(
            &[0.01; 3200],
            Language::German,
            &mut failed_gate,
            &mut observation,
            |_, _, _| Ok(String::new()),
        );
        assert!(matches!(result, Err(InferenceError::Vad)));
        for input in [vec![f32::NAN], vec![0.01; MAX_SAMPLES + 1]] {
            let result = transcribe_using(
                &input,
                Language::German,
                &mut gate,
                &mut observation,
                |_, _, _| Ok(String::new()),
            );
            assert!(matches!(result, Err(InferenceError::InvalidAudio)));
        }
    }
}
