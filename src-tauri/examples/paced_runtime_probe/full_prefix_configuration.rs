//! Isolated full-prefix experiment; never a production configuration selector.
use serde_json::{Value, json};

pub fn decorate(mut value: Value, candidate: bool) -> Value {
    value["configurationId"] = json!(if candidate {
        "p06-full-prefix-candidate-v1"
    } else {
        "p06-full-prefix-reference-v1"
    });
    value["fullPrefix"] = json!({
        "candidate":candidate,
        "productionDefaultChanged":false,
        "windowFrames":320,"startRms":0.00005,"endRms":0.000025,
        "prerollFrames":2400,"tailFrames":3200,"confirmationFrames":8000,
        "earlyInput":"0..cut; cut=last voiced window end+3200; eligible after voice end+8000 published stable frames",
        "revisions":"replace internal text; no early final text or native delivery",
        "finalInput":"always complete0..N after AudioReleased, drain and flush; never infer an empty suffix",
        "storage":"active full journal plus at most one actual job snapshot; no new snapshot while old worker retains PCM; final journal moved",
        "initialization":"both arms use load_prefix_cache; Engine/cache initialization and all decodes owned by serial local-inference worker",
        "decoderPolicy":"unchanged full reference; audio_ctx0/model1500; flash attentionfalse; native timestamps enabled",
        "cache":"after current validation/energy/VAD/padding; exact prepared f32LE SHA256+frames+language; immutable per-decoder Engine/model/config",
        "scope":"SID+generation+actual token owner; strictly increasing job IDs; fresh session each warmup and measured run; revoked/failed/not-admitted clears entry",
        "miss":"fresh state and normal native decode; no text equality, silence-tail or empty-suffix heuristic",
        "receipt":"current tags, prepared frames/hash or explicit null if not reached; hit names prior native origin in same scope; no decode/full attempt on hit",
        "maximumJobs":601,"metadataContainsTextOrPcm":false,
        "inputProof":"host-submit versus complete after_publish times; early cut+4800 independently reconstructed; final N only after AudioReleased",
        "qualification":false
    });
    value["preprocessing"]["mode"] = json!(if candidate {
        "continuous stable DSP; cumulative full-prefix revisions and complete final recording; current normal energy/VAD/preparation before every lookup"
    } else {
        "matched full recording only; same scoped cache decoder and normal energy/VAD/preparation; fresh scope cannot reuse prior session"
    });
    value["measurement"]["mode"] = json!("paced-full-prefix-revision-v1");
    value["measurement"]["variant"] = json!(if candidate {
        "full-prefix-revisions"
    } else {
        "matched-full-recording-reference"
    });
    value["measurement"]["engineRtf"] = json!(
        "sum actual scoped decoder call elapsed including VAD/hash/hit costs / (unique final recorded N/16000); processed overlapping frames reported separately; absent timing is not zero"
    );
    if let Some(state) = value["state"].as_object_mut() {
        state.remove("freshWhisperStateEachJob");
        state.insert("freshWhisperStateEachNativeDecode".into(), json!(true));
        state.insert("cacheHitCreatesWhisperState".into(), json!(false));
    }
    macro_rules! bind {
        ($key:literal, $name:literal, $path:literal) => {
            value["sourceHashes"][$key] = json!({"filename":$name,"sha256":crate::input::digest(include_bytes!($path))});
        };
    }
    bind!(
        "fullPrefixAudio",
        "src-tauri/src/audio/full_prefix.rs",
        "../../src/audio/full_prefix.rs"
    );
    bind!(
        "pipelinePolicy",
        "src-tauri/src/pipeline.rs",
        "../../src/pipeline.rs"
    );
    bind!("appLibrary", "src-tauri/src/lib.rs", "../../src/lib.rs");
    bind!(
        "fullPrefixController",
        "src-tauri/src/controller/full_prefix.rs",
        "../../src/controller/full_prefix.rs"
    );
    bind!(
        "preparedCache",
        "src-tauri/src/transcription/prefix_cache.rs",
        "../../src/transcription/prefix_cache.rs"
    );
    bind!(
        "cacheEvidence",
        "src-tauri/src/transcription/prefix_cache/evidence.rs",
        "../../src/transcription/prefix_cache/evidence.rs"
    );
    bind!(
        "fullPrefixAudit",
        "src-tauri/examples/paced_runtime_probe/full_prefix.rs",
        "full_prefix.rs"
    );
    bind!(
        "fullPrefixConfiguration",
        "src-tauri/examples/paced_runtime_probe/full_prefix_configuration.rs",
        "full_prefix_configuration.rs"
    );
    bind!(
        "engineObservation",
        "src-tauri/src/transcription/observation.rs",
        "../../src/transcription/observation.rs"
    );
    bind!(
        "enginePhases",
        "src-tauri/src/transcription/phase.rs",
        "../../src/transcription/phase.rs"
    );
    bind!(
        "boundedCorpusInput",
        "src-tauri/examples/corpus_probe/input.rs",
        "../corpus_probe/input.rs"
    );
    bind!(
        "cargoBuildConfig",
        ".cargo/config.toml",
        "../../../.cargo/config.toml"
    );
    value
}
