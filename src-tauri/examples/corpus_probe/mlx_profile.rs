//! Explicit, mutually exclusive native quality/performance experiment profiles.
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MlxProfile {
    NativeTimestamps,
    Float32,
    BestOfFive,
    ReferenceCoefficients,
    ReferenceCoefficientsSingleWindow,
    ReferenceEncoder,
    TextOnly,
    SingleWindowText,
    SingleWindowReference,
    SingleWindowReferenceBestOfFive,
}

impl MlxProfile {
    pub fn parse_flag(flag: &str) -> Option<Self> {
        match flag {
            "--mlx-reference-encoder" => Some(Self::ReferenceEncoder),
            "--mlx-f32" => Some(Self::Float32),
            "--mlx-best-of-five" => Some(Self::BestOfFive),
            "--mlx-q5-coefficients-f32" => Some(Self::ReferenceCoefficients),
            "--mlx-q5-coefficients-single-window-reference" => {
                Some(Self::ReferenceCoefficientsSingleWindow)
            }
            "--mlx-text-only" => Some(Self::TextOnly),
            "--mlx-single-window-text" => Some(Self::SingleWindowText),
            "--mlx-single-window-reference" => Some(Self::SingleWindowReference),
            "--mlx-single-window-reference-best-of-five" => {
                Some(Self::SingleWindowReferenceBestOfFive)
            }
            _ => None,
        }
    }
    pub fn dtype(self) -> &'static str {
        match self {
            Self::Float32
            | Self::ReferenceCoefficients
            | Self::ReferenceCoefficientsSingleWindow => "fp32",
            Self::ReferenceEncoder => {
                #[cfg(feature = "mlx")]
                {
                    everlast_dictation::transcription::mlx::MlxEngine::reference_encoder_compute_precision()
                }
                #[cfg(not(feature = "mlx"))]
                {
                    "not-compiled"
                }
            }
            _ => "fp16",
        }
    }
    pub fn ranked(self) -> bool {
        matches!(
            self,
            Self::BestOfFive | Self::SingleWindowReferenceBestOfFive
        )
    }
    #[cfg(all(feature = "mlx", feature = "measurement"))]
    pub fn coefficient_control(self) -> bool {
        matches!(
            self,
            Self::ReferenceCoefficients
                | Self::ReferenceCoefficientsSingleWindow
                | Self::ReferenceEncoder
        )
    }
    pub fn timestamp_policy(self) -> Value {
        match self {
            Self::ReferenceEncoder => {
                json!({"mode":"MLX encoder from exact-Q5 reference coefficients for prepared <=480000; reference CPP decoder with native timestamps/fallback/seek; subsequent windows and longer prepared inputs fully native CPP","encoderComputeDtype":self.dtype(),"checkpointDtype":"fp32","singleWindowMaxPreparedSamples":480000,"decoderBackend":"whisper.cpp-reference","encoderFeatureLayout":"F32 [1,1500,1280] contiguous, model dimensions transposed only by GGML memory order"})
            }
            Self::TextOnly => json!({"mode":"text-only; all window lengths"}),
            Self::SingleWindowText => {
                json!({"mode":"text-only for a single prepared Whisper window; native timestamps for multiple windows", "singleWindowMaxPreparedSamples":480000,"onMissingEndOfText":"retry entire same mel with native timestamps and fresh caches"})
            }
            Self::SingleWindowReference
            | Self::SingleWindowReferenceBestOfFive
            | Self::ReferenceCoefficientsSingleWindow => {
                json!({"mode":"text-only MLX for a single prepared Whisper window; unchanged whisper.cpp reference for multiple windows",
                    "singleWindowMaxPreparedSamples":480000,
                    "onMissingEndOfText":"retry entire same mel with native MLX timestamps and fresh caches",
                    "multiwindowReference":everlast_dictation::model::REFERENCE,
                    "bothAsrModelsResident":true})
            }
            _ => json!({"mode":"native timestamps; all window lengths"}),
        }
    }
}

#[cfg(all(feature = "mlx", feature = "measurement"))]
pub fn load(
    path: &std::path::Path,
    definition: everlast_dictation::model::ModelDefinition,
    profile: MlxProfile,
) -> Result<
    everlast_dictation::transcription::mlx::MlxEngine,
    everlast_dictation::transcription::InferenceError,
> {
    use everlast_dictation::transcription::mlx::MlxEngine;
    match profile {
        MlxProfile::ReferenceEncoder => MlxEngine::load_reference_encoder(path),
        MlxProfile::NativeTimestamps => MlxEngine::load(path, definition),
        MlxProfile::Float32 => MlxEngine::load_f32(path, definition),
        MlxProfile::BestOfFive => MlxEngine::load_best_of_five(path, definition),
        MlxProfile::ReferenceCoefficients => MlxEngine::load_coefficient_control(path),
        MlxProfile::ReferenceCoefficientsSingleWindow => {
            MlxEngine::load_coefficient_window_reference(path)
        }
        MlxProfile::TextOnly => MlxEngine::load_text_only(path, definition),
        MlxProfile::SingleWindowText => MlxEngine::load_single_window_text(path, definition),
        MlxProfile::SingleWindowReference => {
            MlxEngine::load_single_window_reference(path, definition)
        }
        MlxProfile::SingleWindowReferenceBestOfFive => {
            MlxEngine::load_single_window_reference_best_of_five(path, definition)
        }
    }
}
