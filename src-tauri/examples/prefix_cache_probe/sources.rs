use super::input;
use serde_json::{Value, json};

pub fn parameters() -> Result<Value, serde_json::Error> {
    // Reuse only the unchanged decoder/preparation values. Historical source
    // and performance claims in that document are not this binary's metadata.
    let reference: Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/corpus/p06-pretrans-screening-reference-parameters-v1.json"
    ))?;
    Ok(
        json!({"schemaVersion":1,"configurationId":"prepared-prefix-cache-diagnostic-v1",
        "runtime":reference["runtime"],"state":reference["state"],"preprocessing":reference["preprocessing"],
        "diagnostic":{"referenceModelOnly":true,"pairsMaximum":2,"jobsPerPair":3,
            "order":["prefix","same-session-whole","new-session-whole"],
            "sameSessionWholeMustHit":false,"newSessionWholeMustDecodeAndAttemptNativeFull":true,
            "initializer":"CLI main; fresh model/context per pair then actual InferenceService worker",
            "input":"verified PCM16 mono16k WAV; prefix bits equal initial whole range",
            "workerWaitBudgetSeconds":120,"cleanupObservationBudgetSeconds":120,
            "textExport":"SHA256 and UTF8 bytes only","noPacing":true,"noL":true,
            "qualified":false,"productionDefaultChanged":false}}),
    )
}

pub fn hashes() -> Value {
    macro_rules! source {
        ($path:literal) => { json!({"filename":$path,"sha256":input::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../",$path)))}) };
    }
    json!([
        source!("src-tauri/examples/prefix_cache_probe.rs"),
        source!("src-tauri/examples/prefix_cache_probe/contract.rs"),
        source!("src-tauri/examples/prefix_cache_probe/runner.rs"),
        source!("src-tauri/examples/prefix_cache_probe/sources.rs"),
        source!("src-tauri/examples/corpus_probe/input.rs"),
        source!("tests/fixtures/corpus/p06-pretrans-screening-reference-parameters-v1.json"),
        source!("src-tauri/src/lib.rs"),
        source!("src-tauri/src/transcription.rs"),
        source!("src-tauri/src/transcription/prefix_cache.rs"),
        source!("src-tauri/src/transcription/prefix_cache/evidence.rs"),
        source!("src-tauri/src/transcription/phase.rs"),
        source!("src-tauri/src/transcription/observation.rs"),
        source!("src-tauri/src/transcription/speech_gate.rs"),
        source!("src-tauri/src/inference_service.rs"),
        source!("src-tauri/src/inference_service/observation.rs"),
        source!("src-tauri/src/model.rs"),
        source!("src-tauri/src/model/cache.rs"),
        source!("src-tauri/src/model/storage.rs"),
        source!("crates/dictation-core/src/audio.rs"),
        source!(".cargo/config.toml"),
        source!("Cargo.toml"),
        source!("Cargo.lock"),
        source!("src-tauri/Cargo.toml"),
        source!("vendor/whisper-rs/src/whisper_ctx.rs"),
        source!("vendor/whisper-rs/src/whisper_ctx_wrapper.rs"),
        source!("vendor/whisper-rs/src/whisper_params.rs"),
        source!("vendor/whisper-rs/src/whisper_state/mod.rs"),
        source!("vendor/whisper-rs/src/whisper_state/segment.rs"),
        source!("vendor/whisper-rs/src/whisper_vad.rs"),
        source!("vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"),
        source!("vendor/whisper-rs-sys/build.rs"),
        source!("vendor/whisper-rs/Cargo.toml"),
        source!("vendor/whisper-rs-sys/Cargo.toml")
    ])
}
