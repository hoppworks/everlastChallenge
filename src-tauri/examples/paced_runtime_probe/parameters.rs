//! Immutable parameter template tied to the actual compiled measurement sources.
use crate::input;
use everlast_dictation::audio::replay::{
    clock::{FINAL_SPIN, MAXIMUM_SLEEP},
    schedule::JITTER_TOLERANCE,
};
use serde_json::{Value, json};

pub fn template() -> Result<Value, serde_json::Error> {
    let mut value: Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/corpus/reference-parameters-v4.json"
    ))?;
    value["schemaVersion"] = json!(2);
    value["configurationId"] = json!("paced-full-recording-reference-qos-v3");
    // Preserve the v4 policy baseline except the diagnosed punctuation mask.
    // Source pins below bind the actual decoder and probe implementation.
    value["runtime"]["fullParams"]["suppress_nst"] = json!(false);
    value["status"] = json!("measurement baseline; not a model, quality or latency qualification");
    value["preprocessing"]["mode"] = json!(
        "finite/bounds then energy-empty guard; full-recording VAD; unchanged outer energy-range ASR; no speculative segments"
    );
    if let Some(runtime) = value["runtime"].as_object_mut() {
        runtime.remove("whisperRsSysSourceRevision");
        runtime.insert(
            "whisperRsUpstreamRevision".into(),
            json!("7558e1b72f54f2f22a53589afb77e65681834c36"),
        );
        runtime.insert(
            "whisperCppUpstreamRevision".into(),
            json!("2eeeba56e9edd762b4b38467bab96c2517163158"),
        );
        runtime.insert(
            "localCorrections".into(),
            json!([
                "initial SOT no-speech row",
                "VAD failure propagation",
                "VAD CString ownership"
            ]),
        );
    }
    if let Some(state) = value["state"].as_object_mut() {
        state.remove("oneWarmModelContext");
        state.insert("warmAsrContexts".into(), json!(1));
        state.insert("warmVadContexts".into(), json!(1));
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("measurementHost");
        object.remove("changeFromV2");
        object.remove("changeFromV3");
    }
    value["limits"] = json!({"pacedInput":true,"controllerTimestamps":true,
        "notAggregateL":true,"notMicrophoneAcceptance":true,"notFullP05Qualification":true,
        "noNativeDelivery":true,"maximumEndpointAcceptancePending":true});
    value["measurement"] = json!({
        "mode":"paced-complete-recording", "inputFormat":"PCM16 mono 16000 Hz decoded to f32",
        "chunkMillis":20,"jitterToleranceNanos":JITTER_TOLERANCE.as_nanos(),
        "maximumSleepNanos":MAXIMUM_SLEEP.as_nanos(),"finalSpinNanos":FINAL_SPIN.as_nanos(),
        "publisherQos": {"class":"userInteractive", "relativePriority":0,
            "baseRequestedQosChanged":false,
            "scope":"current publisher only; after worker startup, before runtime start through feed, stop and source drop; ended before observer polling",
            "cleanupFailure":"invalid setup; retain failure and mark following runs unavailable",
            "guarantee":"best effort scheduling; no realtime or jitter guarantee"},
        "warmupRuns":1,"measuredRuns":"explicit CLI count 1..30",
        "stopTimestamp":"host entry before controller queue/lock",
        "textTimestamp":"directly after accepted final nonempty controller text",
        "clock":"same monotone Instant, offsets relative to actual capture t0",
        "reset":"actor rejects non-idle audio/inference; fresh session data, warm model context",
        "failurePolicy":"retain every failed or unexecuted run; no partial-sample statistics",
        "variant":"complete-recording-reference; no speculative or audio_ctx optimization"
    });
    value["build"] =
        json!({"debugAssertions":cfg!(debug_assertions),"architecture":std::env::consts::ARCH});
    value["sourceHashes"] = compiled_sources();
    Ok(value)
}

pub fn validate(bytes: &[u8]) -> Result<Value, String> {
    let supplied: Value =
        serde_json::from_slice(bytes).map_err(|_| "invalid bounded parameter JSON")?;
    let actual = template().map_err(|_| "compiled parameter template is invalid")?;
    if supplied != actual {
        return Err("parameter document differs from this compiled configuration; freeze a new document before running".into());
    }
    Ok(supplied)
}

fn compiled_sources() -> Value {
    macro_rules! source {
        ($name:literal, $file:literal) => {
            json!({"filename": $name, "sha256": input::digest(include_bytes!($file))})
        };
    }
    json!({
        "publisherQos":source!("crates/macos-integration/src/qos.rs","../../../crates/macos-integration/src/qos.rs"),
        "nativeLibrary":source!("crates/macos-integration/src/lib.rs","../../../crates/macos-integration/src/lib.rs"),
        "nativeManifest":source!("crates/macos-integration/Cargo.toml","../../../crates/macos-integration/Cargo.toml"),
        "engine":source!("src-tauri/src/transcription.rs","../../src/transcription.rs"),
        "energy":source!("crates/dictation-core/src/audio.rs","../../../crates/dictation-core/src/audio.rs"),
        "speechGate":source!("src-tauri/src/transcription/speech_gate.rs","../../src/transcription/speech_gate.rs"),
        "models":source!("src-tauri/src/model.rs","../../src/model.rs"),
        "runtime":source!("src-tauri/src/runtime.rs","../../src/runtime.rs"),
        "measurement":source!("src-tauri/src/runtime/measurement.rs","../../src/runtime/measurement.rs"),
        "capture":source!("src-tauri/src/audio.rs","../../src/audio.rs"),
        "replay":source!("src-tauri/src/audio/replay.rs","../../src/audio/replay.rs"),
        "clock":source!("src-tauri/src/audio/replay/clock.rs","../../src/audio/replay/clock.rs"),
        "schedule":source!("src-tauri/src/audio/replay/schedule.rs","../../src/audio/replay/schedule.rs"),
        "inference":source!("src-tauri/src/inference_service.rs","../../src/inference_service.rs"),
        "cpp":source!("vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp","../../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"),
        "paramsWrapper":source!("vendor/whisper-rs/src/whisper_params.rs","../../../vendor/whisper-rs/src/whisper_params.rs"),
        "vadWrapper":source!("vendor/whisper-rs/src/whisper_vad.rs","../../../vendor/whisper-rs/src/whisper_vad.rs"),
        "cargoLock":source!("Cargo.lock","../../../Cargo.lock"),
        "workspaceManifest":source!("Cargo.toml","../../../Cargo.toml"),
        "options":source!("src-tauri/examples/paced_runtime_probe/options.rs","options.rs"),
        "manifest":source!("src-tauri/Cargo.toml","../../Cargo.toml"),
        "probe":source!("src-tauri/examples/paced_runtime_probe.rs","../paced_runtime_probe.rs"),
        "runner":source!("src-tauri/examples/paced_runtime_probe/runner.rs","runner.rs"),
        "parameters":source!("src-tauri/examples/paced_runtime_probe/parameters.rs","parameters.rs"),
        "statistics":source!("src-tauri/examples/paced_runtime_probe/statistics.rs","statistics.rs")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publisher_qos_policy_and_native_sources_are_frozen_without_relaxing_deadlines()
    -> Result<(), Box<dyn std::error::Error>> {
        let value = template()?;
        assert_eq!(
            value["measurement"]["publisherQos"]["class"],
            "userInteractive"
        );
        assert_eq!(value["measurement"]["publisherQos"]["relativePriority"], 0);
        assert_eq!(
            value["measurement"]["publisherQos"]["baseRequestedQosChanged"],
            false
        );
        assert_eq!(value["measurement"]["chunkMillis"], 20);
        assert_eq!(value["measurement"]["jitterToleranceNanos"], 10_000_000);
        assert_eq!(value["measurement"]["finalSpinNanos"], 250_000);
        for key in ["publisherQos", "nativeLibrary", "nativeManifest"] {
            assert!(
                value["sourceHashes"][key]["sha256"]
                    .as_str()
                    .is_some_and(|hash| hash.len() == 64)
            );
        }
        Ok(())
    }
    #[test]
    fn only_the_exact_compiled_parameter_document_is_accepted()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = template()?;
        assert!(validate(&serde_json::to_vec(&original)?).is_ok());
        for path in ["threads", "hash", "jitter"] {
            let mut wrong = original.clone();
            match path {
                "threads" => wrong["runtime"]["fullParams"]["n_threads"] = json!(99),
                "hash" => wrong["sourceHashes"]["runtime"]["sha256"] = json!("wrong"),
                _ => wrong["measurement"]["jitterToleranceNanos"] = json!(20000000),
            }
            assert!(validate(&serde_json::to_vec(&wrong)?).is_err());
        }
        assert!(validate(b"{broken").is_err());
        Ok(())
    }
}
