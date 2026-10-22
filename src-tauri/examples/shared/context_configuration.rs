//! Exact shared experiment policy/source configuration; no decoder execution here.
use everlast_dictation::transcription::reduced_context;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub fn decorate(mut value: Value, reduced: bool) -> Value {
    value["configurationId"] = json!(format!(
        "p06-audio-context-{}-v1",
        if reduced { "reduced" } else { "reference" }
    ));
    value["audioContextExperiment"] = json!({"variant":if reduced {"Reduced"}else{"Reference"},"expectedModelAudioCtx":reduced_context::EXPECTED_MODEL_AUDIO_CTX,"reducedAudioCtx":reduced_context::REDUCED_AUDIO_CTX,"maximumPreparedFrames":reduced_context::MAX_PREPARED_FRAMES,"referenceParameter":0,"referenceEffectiveAudioCtx":1500,"longInputParameter":0,"longInputEffectiveAudioCtx":1500,"eligibility":"actual prepared ASR frames, including padding; Reference always0, Reduced1024 iff<=320000 else0","modelInitializationOwner":"developer-main-before-serial-worker","sharedWrapperBothVariants":true,"records":"one armed/inflight/completed receipt, bounded; identity validated at caller admission before submit/feed; receipt failure invalidates subsequent rows","noSegmentation":true,"decoderTimingScope":"actual Decoder::decode includes receipt admission/validation/serialization around Engine; identical wrapper for both variants"});
    value["runtime"]["fullParams"]["audio_ctx"] = if !reduced {
        json!(0)
    } else {
        json!({"policy":"audioContextExperiment","atMostPreparedFrames":reduced_context::MAX_PREPARED_FRAMES,"then":reduced_context::REDUCED_AUDIO_CTX,"otherwise":0,"modelDefault":reduced_context::EXPECTED_MODEL_AUDIO_CTX})
    };
    value["sourceHashes"]["audioContextAdapter"] = json!({"filename":"src-tauri/examples/shared/audio_context.rs","sha256":(Sha256::digest(include_bytes!("audio_context.rs"))).iter().map(|byte| format!("{byte:02x}")).collect::<String>()});
    value["sourceHashes"]["audioContextPolicy"] = json!({"filename":"src-tauri/src/transcription/reduced_context.rs","sha256":(Sha256::digest(include_bytes!("../../src/transcription/reduced_context.rs"))).iter().map(|byte| format!("{byte:02x}")).collect::<String>()});
    value["sourceHashes"]["audioContextConfiguration"] = json!({"filename":"src-tauri/examples/shared/context_configuration.rs","sha256":(Sha256::digest(include_bytes!("context_configuration.rs"))).iter().map(|byte| format!("{byte:02x}")).collect::<String>()});
    value
}
