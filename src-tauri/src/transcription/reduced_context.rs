//! Prospective developer-only audio-context candidate. No segmentation or PCM changes.
use super::{InferenceError, Language, ObservationSink, speech_gate, transcribe_using};

/// Frozen first candidate: encoder context positions (20.48 s), not PCM samples.
pub const REDUCED_AUDIO_CTX: i32 = 1024;
pub const MAX_PREPARED_FRAMES: usize = 320_000;
pub const EXPECTED_MODEL_AUDIO_CTX: i32 = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preparation {
    NotPrepared,
    EnergyEmpty,
    VadRejected,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRecord {
    pub input_frames: usize,
    pub prepared_frames: usize,
    pub preparation: Preparation,
    /// Selected FullParams selector: 0 means the verified model default.
    pub audio_ctx_parameter: Option<i32>,
    /// Selected encoder context size; only attempted natively if decode_attempted.
    pub effective_audio_ctx: Option<i32>,
    /// True only immediately before state.full; state allocation may fail earlier.
    pub decode_attempted: bool,
}

#[derive(Debug)]
pub struct Transcription {
    pub result: Result<String, InferenceError>,
    pub observation: ObservationRecord,
}

impl ObservationSink for ObservationRecord {
    fn full_started(&mut self) {
        self.decode_attempted = true;
    }
    fn prepared(&mut self, _: usize, range: Option<std::ops::Range<usize>>, frames: usize) {
        self.prepared_frames = frames;
        self.preparation = match (range, frames) {
            (None, _) => Preparation::EnergyEmpty,
            (Some(_), 0) => Preparation::VadRejected,
            _ => Preparation::Ready,
        };
    }
}

pub(super) fn transcribe<D: speech_gate::Detector>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    model_audio_ctx: i32,
    decode: impl FnOnce(&[f32], Language, i32, &mut ObservationRecord) -> Result<String, InferenceError>,
) -> Transcription {
    transcribe_policy::<true, D>(samples, language, gate, model_audio_ctx, decode)
}

pub(super) fn transcribe_reference<D: speech_gate::Detector>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    model_audio_ctx: i32,
    decode: impl FnOnce(&[f32], Language, i32, &mut ObservationRecord) -> Result<String, InferenceError>,
) -> Transcription {
    transcribe_policy::<false, D>(samples, language, gate, model_audio_ctx, decode)
}

fn transcribe_policy<const REDUCED: bool, D: speech_gate::Detector>(
    samples: &[f32],
    language: Language,
    gate: &mut speech_gate::SpeechGate<D>,
    model_audio_ctx: i32,
    decode: impl FnOnce(&[f32], Language, i32, &mut ObservationRecord) -> Result<String, InferenceError>,
) -> Transcription {
    let mut observation = ObservationRecord {
        input_frames: samples.len(),
        prepared_frames: 0,
        preparation: Preparation::NotPrepared,
        audio_ctx_parameter: None,
        effective_audio_ctx: None,
        decode_attempted: false,
    };
    if model_audio_ctx != EXPECTED_MODEL_AUDIO_CTX {
        return Transcription {
            result: Err(InferenceError::UnsupportedAudioContext {
                actual: model_audio_ctx,
            }),
            observation,
        };
    }
    let result = transcribe_using(
        samples,
        language,
        gate,
        &mut observation,
        |input, lang, record| {
            let ctx = if REDUCED && input.len() <= MAX_PREPARED_FRAMES {
                REDUCED_AUDIO_CTX
            } else {
                0
            };
            record.audio_ctx_parameter = Some(ctx);
            record.effective_audio_ctx = Some(if ctx == 0 { model_audio_ctx } else { ctx });
            decode(input, lang, ctx, record)
        },
    );
    Transcription {
        result,
        observation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Positive(Vec<f32>);
    impl speech_gate::Detector for Positive {
        fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
            self.0.resize(samples.len().div_ceil(512), 0.9);
            Ok(&self.0)
        }
    }
    #[test]
    fn candidate_preserves_full_boundary_pcm_and_language() -> Result<(), InferenceError> {
        for (frames, expected_ctx) in [(320_000, 1024), (320_001, 0)] {
            let samples = vec![0.02; frames];
            let mut gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
            let output = transcribe(
                &samples,
                Language::English,
                &mut gate,
                1500,
                |pcm, language, ctx, record| {
                    record.full_started();
                    assert_eq!(pcm, samples);
                    assert_eq!(language, Language::English);
                    assert_eq!(ctx, expected_ctx);
                    Ok("complete result".into())
                },
            );
            assert_eq!(output.result?, "complete result");
            assert_eq!(output.observation.prepared_frames, frames);
            assert_eq!(output.observation.audio_ctx_parameter, Some(expected_ctx));
            assert_eq!(
                output.observation.effective_audio_ctx,
                Some(if expected_ctx == 0 {
                    1500
                } else {
                    expected_ctx
                })
            );
            assert!(output.observation.decode_attempted);
        }
        Ok(())
    }
    #[test]
    fn unsupported_model_context_fails_before_detector_or_decode() {
        struct Counted<'a>(&'a mut usize);
        impl speech_gate::Detector for Counted<'_> {
            fn probabilities(&mut self, _: &[f32]) -> Result<&[f32], InferenceError> {
                *self.0 += 1;
                Err(InferenceError::Vad)
            }
        }
        for actual in [-1, 0, 1023, 1024, 1499, 1501] {
            let mut detector_calls = 0;
            let mut gate = speech_gate::SpeechGate::from_detector(Counted(&mut detector_calls));
            let mut decoded = false;
            let output = transcribe(
                &[0.02; 3200],
                Language::German,
                &mut gate,
                actual,
                |_, _, _, _| {
                    decoded = true;
                    Ok("must not run".into())
                },
            );
            assert!(
                matches!(output.result, Err(InferenceError::UnsupportedAudioContext { actual: value }) if value == actual)
            );
            assert!(!decoded);
            assert_eq!(detector_calls, 0);
            assert_eq!(output.observation.preparation, Preparation::NotPrepared);
            assert_eq!(output.observation.effective_audio_ctx, None);
            assert!(!output.observation.decode_attempted);
        }
    }

    #[test]
    fn state_failure_does_not_claim_native_full_execution() {
        let mut gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
        let output = transcribe(
            &[0.02; 3200],
            Language::German,
            &mut gate,
            1500,
            |_, _, _, _| Err(InferenceError::Context),
        );
        assert!(matches!(output.result, Err(InferenceError::Context)));
        assert_eq!(output.observation.preparation, Preparation::Ready);
        assert_eq!(output.observation.prepared_frames, 16000);
        assert_eq!(output.observation.effective_audio_ctx, Some(1024));
        assert!(!output.observation.decode_attempted);
    }

    #[test]
    fn short_candidate_uses_the_same_padded_pcm_as_reference() -> Result<(), InferenceError> {
        let samples = [0.02, -0.01, 0.03];
        let mut reference_gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
        let mut expected = Vec::new();
        let reference = super::super::transcribe_with(
            &samples,
            Language::German,
            &mut reference_gate,
            |pcm, _| {
                expected.extend_from_slice(pcm);
                Ok("same complete text".into())
            },
        )?;
        let mut gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
        let output = transcribe(
            &samples,
            Language::German,
            &mut gate,
            1500,
            |pcm, language, ctx, record| {
                assert_eq!(pcm, expected);
                assert_eq!(&pcm[..3], samples);
                assert!(pcm[3..].iter().all(|value| *value == 0.0));
                assert_eq!(pcm.len(), 16000);
                assert_eq!(language, Language::German);
                assert_eq!(ctx, 1024);
                record.full_started();
                Ok(reference.clone())
            },
        );
        assert_eq!(output.result?, reference);
        assert_eq!(output.observation.input_frames, 3);
        assert_eq!(output.observation.prepared_frames, 16000);
        assert_eq!(output.observation.preparation, Preparation::Ready);
        assert_eq!(output.observation.audio_ctx_parameter, Some(1024));
        assert!(output.observation.decode_attempted);
        Ok(())
    }

    struct Reject;
    impl speech_gate::Detector for Reject {
        fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
            assert_eq!(samples.len(), 3200);
            Ok(&[0.0; 7])
        }
    }
    #[test]
    fn energy_and_vad_rejections_have_no_fake_context_or_decode() -> Result<(), InferenceError> {
        for (samples, expected) in [
            (vec![0.0; 3200], Preparation::EnergyEmpty),
            (vec![0.02; 3200], Preparation::VadRejected),
        ] {
            let mut gate = speech_gate::SpeechGate::from_detector(Reject);
            let mut decoded = false;
            let output = transcribe(&samples, Language::German, &mut gate, 1500, |_, _, _, _| {
                decoded = true;
                Ok("unexpected".into())
            });
            assert!(output.result?.is_empty());
            assert!(!decoded);
            assert_eq!(output.observation.preparation, expected);
            assert_eq!(output.observation.prepared_frames, 0);
            assert_eq!(output.observation.audio_ctx_parameter, None);
            assert_eq!(output.observation.effective_audio_ctx, None);
            assert!(!output.observation.decode_attempted);
        }
        Ok(())
    }

    struct Failing;
    impl speech_gate::Detector for Failing {
        fn probabilities(&mut self, _: &[f32]) -> Result<&[f32], InferenceError> {
            Err(InferenceError::Vad)
        }
    }
    #[test]
    fn gate_and_invalid_audio_errors_never_become_silence_or_decode() {
        let mut gate = speech_gate::SpeechGate::from_detector(Failing);
        for (samples, vad) in [
            (vec![0.02; 3200], true),
            (vec![f32::NAN], false),
            (vec![0.02; dictation_core::audio::MAX_SAMPLES + 1], false),
        ] {
            let mut decoded = false;
            let output = transcribe(
                &samples,
                Language::English,
                &mut gate,
                1500,
                |_, _, _, _| {
                    decoded = true;
                    Ok("unexpected".into())
                },
            );
            assert!(matches!(
                (&output.result, vad),
                (Err(InferenceError::Vad), true) | (Err(InferenceError::InvalidAudio), false)
            ));
            assert!(!decoded);
            assert_eq!(output.observation.preparation, Preparation::NotPrepared);
            assert_eq!(output.observation.audio_ctx_parameter, None);
            assert_eq!(output.observation.effective_audio_ctx, None);
            assert!(!output.observation.decode_attempted);
        }
    }

    #[test]
    fn native_decode_error_retains_chosen_context_and_attempt() {
        let mut gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
        let output = transcribe(
            &[0.02; 3200],
            Language::German,
            &mut gate,
            1500,
            |_, _, _, record| {
                record.full_started();
                Err(InferenceError::Decode)
            },
        );
        assert!(matches!(output.result, Err(InferenceError::Decode)));
        assert_eq!(output.observation.audio_ctx_parameter, Some(1024));
        assert_eq!(output.observation.effective_audio_ctx, Some(1024));
        assert!(output.observation.decode_attempted);
    }
    #[test]
    fn reference_receipt_uses_same_prepared_pcm_with_model_default() -> Result<(), InferenceError> {
        let mut gate = speech_gate::SpeechGate::from_detector(Positive(Vec::new()));
        let output = transcribe_reference(
            &[0.02; 3200],
            Language::English,
            &mut gate,
            1500,
            |pcm, language, ctx, record| {
                assert_eq!(pcm.len(), 16000);
                assert_eq!(language, Language::English);
                assert_eq!(ctx, 0);
                record.full_started();
                Ok("reference text".into())
            },
        );
        assert_eq!(output.result?, "reference text");
        assert_eq!(output.observation.audio_ctx_parameter, Some(0));
        assert_eq!(output.observation.effective_audio_ctx, Some(1500));
        assert!(output.observation.decode_attempted);
        Ok(())
    }
}
