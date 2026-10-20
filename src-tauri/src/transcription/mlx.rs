//! Native MLX inference. Every MLX object stays on the creating serial worker.
use super::{InferenceError, Language, speech_gate::SpeechGate, transcribe_with};
use crate::{
    inference_service::Decoder,
    model::{ModelDefinition, verify_file},
};
#[cfg(test)]
use mlxrs::audio::stt::models::whisper::decoding;
use mlxrs::audio::stt::models::whisper::{
    audio::{N_FRAMES, N_SAMPLES, log_mel_spectrogram_whisper},
    config::ModelDimensions,
    decoding::{DecodingOptions, SuppressSpec, Task, TranscribeOptions},
    model::WhisperModel,
    tokenizer::HFTokenizerWrapper,
};
use mlxrs::{Array, Device, Dtype, Stream, tokenizer::Tokenizer};
use std::{fs, path::Path};

#[derive(Clone, Copy)]
enum TimestampPolicy {
    Native,
    #[cfg(feature = "measurement")]
    TextOnly,
    #[cfg(feature = "measurement")]
    SingleWindowText,
}
impl TimestampPolicy {
    fn without_timestamps(self, _prepared_samples: usize) -> bool {
        match self {
            Self::Native => false,
            #[cfg(feature = "measurement")]
            Self::TextOnly => true,
            // Only one model window: no timestamp cursor is needed to advance
            // into another window. Longer recordings retain native seek rules.
            #[cfg(feature = "measurement")]
            Self::SingleWindowText => _prepared_samples <= N_SAMPLES,
        }
    }
}

mod complete_decode;

// Explicit developer instrumentation; ordinary builds never read this flag.
fn load_stage<T>(
    name: &str,
    load: impl FnOnce() -> Result<T, InferenceError>,
) -> Result<T, InferenceError> {
    #[cfg(feature = "measurement")]
    let started = (std::env::var_os("EVERLAST_MLX_LOAD_DIAGNOSTICS").as_deref()
        == Some(std::ffi::OsStr::new("1")))
    .then(std::time::Instant::now);
    let result = load();
    #[cfg(feature = "measurement")]
    if let Some(started) = started {
        eprintln!(
            "{}",
            serde_json::json!({"kind":"mlx-load-stage", "stage":name,
            "elapsedMicros":started.elapsed().as_micros(), "ok":result.is_ok()})
        );
    }
    #[cfg(not(feature = "measurement"))]
    let _ = name;
    result
}

pub struct MlxEngine {
    model: WhisperModel,
    tokenizer: Tokenizer,
    gate: SpeechGate,
    best_of: Option<usize>,
    timestamps: TimestampPolicy,
    multiwindow_reference: Option<super::Engine>,
    reference_encoder: bool,
    // Last field: all model-owned arrays drop before the fallback cache cleanup.
    cache_budget: Option<mlxrs::memory::CacheLimitGuard>,
}

impl MlxEngine {
    pub fn load(path: &Path, definition: ModelDefinition) -> Result<Self, InferenceError> {
        if definition.is_reference_encoder() {
            return Self::load_reference_encoder(path);
        }
        Self::load_profile(
            path,
            definition,
            Dtype::F16,
            None,
            None,
            TimestampPolicy::Native,
        )
    }

    /// Developer-only numerical precision comparison; pinned payload is unchanged.
    #[cfg(feature = "measurement")]
    pub fn load_f32(path: &Path, definition: ModelDefinition) -> Result<Self, InferenceError> {
        Self::load_profile(
            path,
            definition,
            Dtype::F32,
            None,
            None,
            TimestampPolicy::Native,
        )
    }

    fn load_profile(
        path: &Path,
        definition: ModelDefinition,
        dtype: Dtype,
        best_of: Option<usize>,
        payload: Option<ModelDefinition>,
        timestamps: TimestampPolicy,
    ) -> Result<Self, InferenceError> {
        let result = Self::load_inner(path, definition, dtype, best_of, payload, timestamps);
        if result.is_err() {
            // Failed initialization also ends this dedicated worker's MLX life.
            cleanup_streams()?;
        }
        result
    }

    /// Developer-only quality comparison using five ranked sampled trajectories.
    #[cfg(feature = "measurement")]
    pub fn load_best_of_five(
        path: &Path,
        definition: ModelDefinition,
    ) -> Result<Self, InferenceError> {
        Self::load_profile(
            path,
            definition,
            Dtype::F16,
            Some(5),
            None,
            TimestampPolicy::Native,
        )
    }

    /// Developer-only speed profile: no unused timestamp prediction. Every
    /// 30-second window is still decoded in full with fresh decoder caches.
    #[cfg(feature = "measurement")]
    pub fn load_text_only(
        path: &Path,
        definition: ModelDefinition,
    ) -> Result<Self, InferenceError> {
        Self::load_profile(
            path,
            definition,
            Dtype::F16,
            None,
            None,
            TimestampPolicy::TextOnly,
        )
    }

    /// Candidate uses the model's actual prepared-window boundary, for every
    /// language/input. Multiwindow recordings retain timestamp-guided seeks.
    #[cfg(feature = "measurement")]
    pub fn load_single_window_text(
        path: &Path,
        definition: ModelDefinition,
    ) -> Result<Self, InferenceError> {
        Self::load_profile(
            path,
            definition,
            Dtype::F16,
            None,
            None,
            TimestampPolicy::SingleWindowText,
        )
    }

    /// Explicit hybrid experiment. Keep both verified models warm on this same
    /// worker; long prepared recordings use the unchanged reference decoder.
    /// Routing depends only on the actual model-window capacity, in all languages.
    #[cfg(feature = "measurement")]
    pub fn load_single_window_reference(
        path: &Path,
        definition: ModelDefinition,
    ) -> Result<Self, InferenceError> {
        Self::load_window_reference_profile(path, definition, None)
    }

    #[cfg(feature = "measurement")]
    pub fn load_single_window_reference_best_of_five(
        path: &Path,
        definition: ModelDefinition,
    ) -> Result<Self, InferenceError> {
        Self::load_window_reference_profile(path, definition, Some(5))
    }

    #[cfg(feature = "measurement")]
    fn load_window_reference_profile(
        path: &Path,
        definition: ModelDefinition,
        best_of: Option<usize>,
    ) -> Result<Self, InferenceError> {
        let directory = path.parent().ok_or(InferenceError::ModelPath)?;
        let reference = load_stage("reference-decoder-and-gate", || {
            super::Engine::load(
                &directory.join(crate::model::REFERENCE.file_name()),
                crate::model::REFERENCE,
            )
        })?;
        let mut engine = Self::load_profile(
            path,
            definition,
            Dtype::F16,
            best_of,
            None,
            TimestampPolicy::SingleWindowText,
        )?;
        engine.multiwindow_reference = Some(reference);
        Ok(engine)
    }

    /// Developer-only coefficient identity control. All five files remain pinned;
    /// the locally derived payload has no invented publisher/download endpoint.
    #[cfg(feature = "measurement")]
    pub fn load_coefficient_control(path: &Path) -> Result<Self, InferenceError> {
        Self::load_profile(
            path,
            crate::model::MLX_REFERENCE,
            Dtype::F32,
            None,
            Some(crate::model::COEFFICIENT_CONTROL),
            TimestampPolicy::Native,
        )
    }

    /// Same prepared-window policy with all coefficients taken bit-for-bit from
    /// the pinned CPP reference. A numerical control, never a publisher model.
    #[cfg(feature = "measurement")]
    pub fn load_coefficient_window_reference(path: &Path) -> Result<Self, InferenceError> {
        let reference = super::Engine::load(
            &path
                .parent()
                .ok_or(InferenceError::ModelPath)?
                .join(crate::model::REFERENCE.file_name()),
            crate::model::REFERENCE,
        )?;
        let mut engine = Self::load_profile(
            path,
            crate::model::MLX_REFERENCE,
            Dtype::F32,
            None,
            Some(crate::model::COEFFICIENT_CONTROL),
            TimestampPolicy::SingleWindowText,
        )?;
        engine.multiwindow_reference = Some(reference);
        Ok(engine)
    }

    /// Pinned reference coefficient encoder with the full CPP decoder.
    /// Long recordings and later seeks retain their native reference encoder.
    pub fn load_reference_encoder(path: &Path) -> Result<Self, InferenceError> {
        if cfg!(feature = "mlx-encoder-f16") {
            // Fixed experimental profile selected by the RAM/speed screen.
            // This limits reusable buffers, not active model allocations.
            Self::load_reference_encoder_with_cache_budget(path, 256 * 1024 * 1024)
        } else {
            Self::load_reference_encoder_with_dtype(path, Self::reference_encoder_compute_dtype())
        }
    }

    fn reference_encoder_compute_dtype() -> Dtype {
        if cfg!(feature = "mlx-encoder-f16") {
            Dtype::F16
        } else {
            Dtype::F32
        }
    }

    /// Precision selected by this build, used by the frozen probe parameters.
    pub fn reference_encoder_compute_precision() -> &'static str {
        match Self::reference_encoder_compute_dtype() {
            Dtype::F16 => "fp16",
            _ => "fp32",
        }
    }

    fn load_reference_encoder_with_dtype(
        path: &Path,
        dtype: Dtype,
    ) -> Result<Self, InferenceError> {
        let reference = load_stage("reference-decoder-and-gate", || {
            super::Engine::load(
                &path
                    .parent()
                    .ok_or(InferenceError::ModelPath)?
                    .join(crate::model::REFERENCE.file_name()),
                crate::model::REFERENCE,
            )
        })?;
        let mut engine = Self::load_profile(
            path,
            crate::model::MLX_REFERENCE,
            dtype,
            None,
            Some(crate::model::MLX_REFERENCE_COEFFICIENTS),
            TimestampPolicy::Native,
        )?;
        engine.multiwindow_reference = Some(reference);
        engine.reference_encoder = true;
        #[cfg(feature = "measurement")]
        if std::env::var_os("EVERLAST_MLX_SEGMENT_DIAGNOSTICS").is_some_and(|flag| flag == "1") {
            let actual = match engine.model.dtype() {
                Dtype::F16 => "fp16",
                Dtype::F32 => "fp32",
                _ => "unsupported",
            };
            eprintln!(
                "{}",
                serde_json::json!({
                    "kind":"mlx-reference-encoder-loaded", "encoderComputeDtype":actual,
                    "checkpointDtype":"fp32", "bridgeFeatureDtype":"fp32",
                    "checkpointSha256":crate::model::MLX_REFERENCE_COEFFICIENTS.sha256(),
                    "decoderModelSha256":crate::model::REFERENCE.sha256()
                })
            );
        }
        Ok(engine)
    }

    /// Diagnostic arm only. The F32 checkpoint is cast to F16 by the existing
    /// validated loader; the full reference CPP decoder and fallback stay fixed.
    #[cfg(feature = "measurement")]
    pub fn load_reference_encoder_f16_for_measurement(path: &Path) -> Result<Self, InferenceError> {
        Self::load_reference_encoder_with_dtype(path, Dtype::F16)
    }

    /// Explicit baseline even when the build selects the F16 candidate.
    #[cfg(feature = "measurement")]
    pub fn load_reference_encoder_f32_for_measurement(path: &Path) -> Result<Self, InferenceError> {
        Self::load_reference_encoder_with_dtype(path, Dtype::F32)
    }

    /// Resource-policy experiment with the same encoder/decoder values and
    /// preparation. The cache budget lasts for this worker-owned model only.
    #[cfg(feature = "measurement")]
    pub fn load_reference_encoder_with_cache_budget_for_measurement(
        path: &Path,
        bytes: u64,
    ) -> Result<Self, InferenceError> {
        Self::load_reference_encoder_with_cache_budget(path, bytes)
    }

    fn load_reference_encoder_with_cache_budget(
        path: &Path,
        bytes: u64,
    ) -> Result<Self, InferenceError> {
        let budget =
            mlxrs::memory::CacheLimitGuard::install(bytes).map_err(|_| InferenceError::Context)?;
        let mut engine =
            Self::load_reference_encoder_with_dtype(path, Self::reference_encoder_compute_dtype())?;
        #[cfg(feature = "measurement")]
        if std::env::var_os("EVERLAST_MLX_SEGMENT_DIAGNOSTICS").is_some_and(|flag| flag == "1") {
            eprintln!(
                "{}",
                serde_json::json!({
                    "kind":"mlx-cache-budget-installed", "bytes":budget.limit(),
                    "previousBytes":budget.old_limit(), "scope":"model-owning-worker"
                })
            );
        }
        engine.cache_budget = Some(budget);
        Ok(engine)
    }

    #[cfg(feature = "measurement")]
    pub fn cache_budget_for_measurement(&self) -> Option<u64> {
        self.cache_budget.as_ref().map(|budget| budget.limit())
    }

    /// Observes the actual model field; does not infer dtype from a CLI label.
    #[cfg(feature = "measurement")]
    pub fn reference_encoder_dtype_for_measurement(&self) -> Option<Dtype> {
        self.reference_encoder.then(|| self.model.dtype())
    }

    #[cfg(feature = "measurement")]
    pub fn coefficient_control_metadata() -> serde_json::Value {
        let derived = crate::model::COEFFICIENT_CONTROL;
        serde_json::json!({"id":derived.id(),"file":derived.file_name(),
            "bytes":derived.bytes(),"sha256":derived.sha256(),"dtype":"fp32",
            "downloadUrl":null,"localCoefficientControl":true,
            "derivedFrom":{"id":crate::model::REFERENCE.id(),"sha256":crate::model::REFERENCE.sha256()},
            "conversion":"pinned ggml_get_type_traits to_float, F32; native names/conv layout; supplied encoder positions"})
    }

    fn load_inner(
        path: &Path,
        definition: ModelDefinition,
        dtype: Dtype,
        best_of: Option<usize>,
        payload: Option<ModelDefinition>,
        timestamps: TimestampPolicy,
    ) -> Result<Self, InferenceError> {
        if !definition.is_mlx() || path.file_name() != Some(definition.file_name().as_ref()) {
            return Err(InferenceError::Context);
        }
        let directory = path.parent().ok_or(InferenceError::ModelPath)?;
        for asset in definition.installation_assets() {
            let checked = if asset.id() == definition.id() {
                payload.unwrap_or(asset)
            } else {
                asset
            };
            load_stage(&format!("verify:{}", checked.file_name()), || {
                verify_file(&directory.join(checked.file_name()), checked).map_err(Into::into)
            })?;
        }
        // The upstream loader merges every safetensors file and auto-detects
        // CoreML directories. Only the pinned MLX payload is admitted here.
        for entry in fs::read_dir(directory).map_err(|_| InferenceError::Context)? {
            let entry = entry.map_err(|_| InferenceError::Context)?;
            let file = entry.path();
            if file
                .extension()
                .is_some_and(|extension| extension == "mlmodelc")
                || (file
                    .extension()
                    .is_some_and(|extension| extension == "safetensors")
                    && file.file_name() != Some(definition.file_name().as_ref()))
            {
                return Err(InferenceError::Context);
            }
        }
        Device::gpu()
            .and_then(|device| device.set_default())
            .map_err(|_| InferenceError::Context)?;
        let config: serde_json::Value = serde_json::from_slice(
            &fs::read(directory.join("config.json")).map_err(|_| InferenceError::Context)?,
        )
        .map_err(|_| InferenceError::Context)?;
        if config
            .get("quantization")
            .is_some_and(|value| !value.is_null())
        {
            return Err(InferenceError::Context);
        }
        let dimensions =
            ModelDimensions::from_dict(&config).map_err(|_| InferenceError::Context)?;
        if !dimensions.is_multilingual() || dimensions.n_audio_ctx() != 1500 {
            return Err(InferenceError::Context);
        }
        let model = load_stage("mlx-whisper-model", || {
            WhisperModel::load(directory, dimensions, dtype).map_err(|_| InferenceError::Context)
        })?;
        let tokenizer = load_stage("tokenizer", || {
            Tokenizer::from_path(directory, None).map_err(|_| InferenceError::Context)
        })?;
        whisper_rs::install_logging_hooks();
        let gate = load_stage("mlx-speech-gate", || {
            SpeechGate::load(&directory.join(crate::model::VAD.file_name()))
        })?;
        Ok(Self {
            model,
            tokenizer,
            gate,
            best_of,
            timestamps,
            multiwindow_reference: None,
            reference_encoder: false,
            cache_budget: None,
        })
    }

    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
    ) -> Result<String, InferenceError> {
        let Self {
            model,
            tokenizer,
            gate,
            best_of,
            timestamps,
            multiwindow_reference,
            reference_encoder,
            ..
        } = self;
        transcribe_with(samples, language, gate, |input, language| {
            if input.len() > N_SAMPLES
                && let Some(reference) = multiwindow_reference
            {
                // Preparation/VAD already ran once above. Pass exactly those
                // original PCM values to the same full reference operation.
                let result = super::decode::<false, _>(
                    &reference.context,
                    input,
                    language,
                    &mut super::NoObservation,
                );
                #[cfg(feature = "measurement")]
                if std::env::var_os("EVERLAST_MLX_SEGMENT_DIAGNOSTICS")
                    .is_some_and(|flag| flag == "1")
                {
                    eprintln!(
                        "{}",
                        serde_json::json!({"kind":"mlx-window-reference-diagnostic",
                        "preparedFrames":input.len(),"backend":"whisper.cpp-reference",
                        "modelSha256":crate::model::REFERENCE.sha256()})
                    );
                }
                return result;
            }
            let length = i32::try_from(input.len()).map_err(|_| InferenceError::InvalidAudio)?;
            let waveform =
                Array::from_slice::<f32>(input, &[length]).map_err(|_| InferenceError::Decode)?;
            let mel = log_mel_spectrogram_whisper(&waveform, model.dims().n_mels(), N_SAMPLES)
                .map_err(|_| InferenceError::Decode)?;
            if *reference_encoder {
                use mlxrs::audio::stt::model::AutoregressiveStt;
                use mlxrs::audio::stt::models::whisper::audio::pad_or_trim;
                let reference = multiwindow_reference
                    .as_ref()
                    .ok_or(InferenceError::Context)?;
                let window = pad_or_trim(&mel, N_FRAMES, 0).map_err(|_| InferenceError::Decode)?;
                let mut features = mlxrs::ops::shape::contiguous(
                    &model
                        .encode(&window)
                        .map_err(|_| InferenceError::Decode)?
                        .astype(Dtype::F32)
                        .map_err(|_| InferenceError::Decode)?,
                    false,
                )
                .map_err(|_| InferenceError::Decode)?;
                features.eval().map_err(|_| InferenceError::Decode)?;
                synchronize()?;
                if features.shape() != [1, 1500, 1280] {
                    return Err(InferenceError::Decode);
                }
                let values = features
                    .to_vec::<f32>()
                    .map_err(|_| InferenceError::Memory)?;
                return super::decode_with_features::<false, _>(
                    &reference.context,
                    input,
                    language,
                    &mut super::NoObservation,
                    0,
                    Some(&values),
                );
            }
            let content_frames = mel.shape()[0].saturating_sub(N_FRAMES);
            let tokenizer = HFTokenizerWrapper::new(
                tokenizer,
                model.dims().is_multilingual(),
                model.dims().num_languages(),
                Some(language.code()),
                Task::Transcribe,
            )
            .map_err(|_| InferenceError::Decode)?;
            let without_timestamps = timestamps.without_timestamps(input.len());
            let options = TranscribeOptions {
                decode: decode_options(&tokenizer, language, *best_of, without_timestamps),
                temperatures: if best_of.is_some() {
                    vec![0.2, 0.4, 0.6, 0.8, 1.0]
                } else {
                    vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
                },
                condition_on_previous_text: false,
                word_timestamps: false,
                initial_prompt: None,
                no_speech_threshold: Some(0.6),
                logprob_threshold: Some(-1.0),
                compression_ratio_threshold: Some(2.4),
                ..TranscribeOptions::default()
            };
            let backend = model.backend(false, *best_of);
            // The lower API creates fresh per-window decoder caches. Returning
            // String requires actual token materialization, not merely enqueue.
            let complete_decode::Complete {
                result,
                native_timestamp_retry: _native_timestamp_retry,
                without_timestamps: _actual_without_timestamps,
            } = complete_decode::transcribe(&backend, &tokenizer, &mel, content_frames, &options)?;
            #[cfg(feature = "measurement")]
            if std::env::var_os("EVERLAST_MLX_SEGMENT_DIAGNOSTICS").is_some_and(|flag| flag == "1")
            {
                // Explicit developer-only numeric receipts; no transcript/audio content.
                eprintln!(
                    "{}",
                    serde_json::json!({"kind":"mlx-segment-diagnostic", "preparedFrames":input.len(),"withoutTimestamps":_actual_without_timestamps,"textOnlyTokenBudgetRetry":_native_timestamp_retry,
                    "segments":result.segments.iter().map(|segment| serde_json::json!({
                        "start":segment.start,"end":segment.end,"temperature":segment.temperature,
                        "avgLogprob":segment.avg_logprob,"noSpeechProb":segment.no_speech_prob,
                        "compressionRatio":segment.compression_ratio,"tokens":segment.tokens.len()
                    })).collect::<Vec<_>>() })
                );
            }
            if result.text.len() > 256 * 1024 {
                return Err(InferenceError::OutputLimit);
            }
            Ok(result.text.trim().to_owned())
        })
    }
}

fn decode_options(
    tokenizer: &HFTokenizerWrapper<'_>,
    language: Language,
    best_of: Option<usize>,
    without_timestamps: bool,
) -> DecodingOptions {
    DecodingOptions {
        language: Some(language.code().into()),
        task: Task::Transcribe,
        // whisper.cpp always excludes language switches, independently of
        // its optional non-speech token suppression. Keep the fixed-language
        // prompt but never sample another language special in the output.
        suppress_tokens: SuppressSpec::Ids(tokenizer.all_language_tokens()),
        without_timestamps,
        best_of,
        ..DecodingOptions::default()
    }
}

fn synchronize() -> Result<(), InferenceError> {
    Stream::default_gpu()
        .and_then(|stream| stream.synchronize())
        .map_err(|_| InferenceError::Decode)
}

fn cleanup_streams() -> Result<(), InferenceError> {
    let synchronized = synchronize();
    let cleared = Stream::clear_current_thread_streams().map_err(|_| InferenceError::Context);
    synchronized.and(cleared)
}

impl Decoder for MlxEngine {
    fn finish(mut self) -> Result<(), InferenceError> {
        let synchronized = synchronize();
        let budget = self.cache_budget.take();
        drop(self);
        let released = budget.map_or(Ok(()), |budget| {
            budget.finish().map_err(|_| InferenceError::Context)
        });
        let cleared = Stream::clear_current_thread_streams().map_err(|_| InferenceError::Context);
        synchronized.and(released).and(cleared)
    }

    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        self.transcribe(audio, language)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coefficient_tests;

#[cfg(test)]
mod completion_tests;
