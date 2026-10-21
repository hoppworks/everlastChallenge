//! Frozen MLX decoder identity; capture, publisher and acceptance remain shared.
use everlast_dictation::model::{MLX_REFERENCE, MLX_REFERENCE_COEFFICIENTS, REFERENCE, VAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn encoder_precision() -> &'static str {
    #[cfg(feature = "mlx")]
    {
        everlast_dictation::transcription::mlx::MlxEngine::reference_encoder_compute_precision()
    }
    #[cfg(not(feature = "mlx"))]
    {
        "not-compiled"
    }
}

pub fn decorate(mut value: Value, configuration: &str) -> Value {
    value["configurationId"] = json!(configuration);
    value["models"] = json!(MLX_REFERENCE.installation_assets().chain(std::iter::once(REFERENCE)).map(|asset| json!({
        "id":asset.id(),"file":asset.file_name(),"bytes":asset.bytes(),"sha256":asset.sha256(),"url":asset.url()
    })).collect::<Vec<_>>());
    value["models"][0] = json!({"id":MLX_REFERENCE_COEFFICIENTS.id(),"file":MLX_REFERENCE_COEFFICIENTS.file_name(),
        "bytes":MLX_REFERENCE_COEFFICIENTS.bytes(),"sha256":MLX_REFERENCE_COEFFICIENTS.sha256(),"url":null,
        "derivedFromSha256":REFERENCE.sha256(),"conversion":"safe Rust Q5/F16/F32 conversion; whole output SHA independently established using pinned native GGML to_float; supplied encoder positions"});
    value["runtime"]["backend"] = json!("MLX reference encoder + CPP reference decoder");
    value["runtime"]["decoderBackend"] = json!("whisper.cpp-reference");
    value["runtime"]["encoderDtype"] = json!(encoder_precision());
    value["runtime"]["encoderCheckpointDtype"] = json!("fp32");
    value["runtime"]["encoderFeatureDtype"] = json!("fp32");
    value["runtime"]["encoderModelSha256"] = json!(MLX_REFERENCE_COEFFICIENTS.sha256());
    value["runtime"]["decoderModelSha256"] = json!(REFERENCE.sha256());
    value["runtime"]["audioContext"] = json!(1500);
    value["runtime"]["melWindow"] = json!("MLX periodic Hann400, hop160, reflect center");
    value["runtime"]["timestampPolicy"] = json!({"singleWindowMaxPreparedSamples":480000,
        "singleWindow":"MLX encoder from exact reference coefficients; CPP native timestamps/decoder/fallback; first seek0 external F32 features",
        "subsequentSeeks":"CPP native encoder and unchanged Full loop",
        "multipleWindows":"entire unchanged CPP reference; same prepared PCM"});
    value["runtime"]["freshDecoderStatePerJob"] = json!(true);
    value["runtime"]["completeGpuSynchronizationBeforeResult"] = json!(true);
    value["runtime"]["vadBackend"] = json!("whisper.cpp CPU Silero; unchanged common preparation");
    value["runtime"]["vadModel"] = json!({"id":VAD.id(),"sha256":VAD.sha256()});
    value["runtime"]["modelOwner"] =
        json!("initialize/use/drop/synchronize/clear on the dedicated serial worker");
    value["state"] = json!({"warmAsrContexts":2,"warmVadContexts":2,"vadContextsUsedPerJob":1,"freshDecoderCachesPerJob":true,
        "crossJobGpuCacheReuse":false,"precomputation":false,"completeRecording":true});
    value["measurement"]["variant"] = json!("complete-recording-mlx-reference-encoder");
    macro_rules! source {($name:literal,$path:literal)=>{json!({"filename":$name,"sha256":format!("{:x}",Sha256::digest(include_bytes!($path)))})};}
    value["mlxSources"] = json!({
        "cppEncoderBridge":source!("vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp","../../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"),
        "cppEncoderHeader":source!("vendor/whisper-rs-sys/whisper.cpp/include/whisper.h","../../../vendor/whisper-rs-sys/whisper.cpp/include/whisper.h"),
        "cppBuild":source!("vendor/whisper-rs-sys/build.rs","../../../vendor/whisper-rs-sys/build.rs"),
        "cppEncoderWrapper":source!("vendor/whisper-rs/src/whisper_state/mod.rs","../../../vendor/whisper-rs/src/whisper_state/mod.rs"),
        "cppEncoderPatch":source!("vendor/whisper-rs-sys/EVERLAST-ENCODER-PATCH.json","../../../vendor/whisper-rs-sys/EVERLAST-ENCODER-PATCH.json"),
        "adapter":source!("src-tauri/src/transcription/mlx.rs","../../src/transcription/mlx.rs"),
        "completeDecoder":source!("src-tauri/src/transcription/mlx/complete_decode.rs","../../src/transcription/mlx/complete_decode.rs"),
        "manifest":source!("src-tauri/src/model/mlx_manifest.rs","../../src/model/mlx_manifest.rs"),
        "configuration":source!("src-tauri/examples/shared/mlx_configuration.rs","mlx_configuration.rs"),
        "decoding":source!("vendor/mlxrs/src/audio/stt/models/whisper/decoding.rs","../../../vendor/mlxrs/src/audio/stt/models/whisper/decoding.rs"),
        "model":source!("vendor/mlxrs/src/audio/stt/models/whisper/model.rs","../../../vendor/mlxrs/src/audio/stt/models/whisper/model.rs"),
        "dsp":source!("vendor/mlxrs/src/audio/dsp.rs","../../../vendor/mlxrs/src/audio/dsp.rs"),
        "audio":source!("vendor/mlxrs/src/audio/stt/models/whisper/audio.rs","../../../vendor/mlxrs/src/audio/stt/models/whisper/audio.rs"),
        "tokenizer":source!("vendor/mlxrs/src/audio/stt/models/whisper/tokenizer.rs","../../../vendor/mlxrs/src/audio/stt/models/whisper/tokenizer.rs"),
        "mlxPatch":source!("vendor/mlxrs/EVERLAST-PATCH.json","../../../vendor/mlxrs/EVERLAST-PATCH.json"),
        "tokenizerPatch":source!("vendor/tokenizers/EVERLAST-PATCH.json","../../../vendor/tokenizers/EVERLAST-PATCH.json"),
        "nativePatch":source!("vendor/mlxrs-sys/EVERLAST-PATCH.json","../../../vendor/mlxrs-sys/EVERLAST-PATCH.json"),
        "nativeBuild":source!("config/mlx-build-inputs.json","../../../config/mlx-build-inputs.json"),
        "toolchain":source!("rust-toolchain.toml","../../../rust-toolchain.toml"),
        "lockfile":source!("Cargo.lock","../../../Cargo.lock")
    });
    value
}

#[cfg(feature = "mlx")]
pub fn load_service(
    path: std::path::PathBuf,
    definition: everlast_dictation::model::ModelDefinition,
) -> Result<
    everlast_dictation::inference_service::InferenceService,
    everlast_dictation::inference_service::ServiceError,
> {
    if !definition.is_reference_encoder() {
        return Err(
            everlast_dictation::inference_service::ServiceError::Backend(
                everlast_dictation::transcription::InferenceError::Context,
            ),
        );
    }
    everlast_dictation::inference_service::InferenceService::load(path, definition)
}
