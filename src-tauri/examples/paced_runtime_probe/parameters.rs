#[path = "../shared/context_configuration.rs"]
mod context_configuration;
// Immutable parameter template tied to the actual compiled measurement sources.
use crate::input;
#[path = "full_prefix_configuration.rs"]
mod full_prefix_configuration;
#[path = "pretranscription_configuration.rs"]
mod pretranscription;
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
    value["configurationId"] = json!("paced-full-recording-native-timestamps-rtf-empty-qos-v5");
    // Preserve the v4 baseline except diagnosed punctuation suppression and
    // native-timestamp suppression; both correctness corrections are explicit.
    // Source pins below bind the actual decoder and probe implementation.
    value["runtime"]["fullParams"]["suppress_nst"] = json!(false);
    value["runtime"]["fullParams"]["no_timestamps"] = json!(false);
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
        "expectedOutcome":"explicit Local default or --expect-empty; never inferred from input",
        "engineRtf":"actual Decoder::decode elapsed / (passed 16-kHz frames / 16000); absent timing is not zero; not pure ASR",
        "terminalTimestamp":"accepted Local/Empty controller completion; Empty has no text timestamp or L",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Baseline,
    TimeConstrained,
    ContextReference,
    ContextReduced,
    PretransReference,
    PretransCandidate,
    FullPrefixReference,
    FullPrefixCandidate,
}
pub fn scheduling_template() -> Result<Value, serde_json::Error> {
    use macos_integration::realtime;
    let mut value = template()?;
    value["configurationId"] = json!("paced-full-recording-native-timestamps-rtf-tc-owned-v6");
    if let Some(object) = value["measurement"].as_object_mut() {
        object.remove("publisherQos");
    }
    value["measurement"]["publisherThread"] = json!({
        "policy":"timeConstrained","periodNanos":realtime::PERIOD.as_nanos(),
        "computationNanos":realtime::COMPUTATION.as_nanos(),"constraintNanos":realtime::CONSTRAINT.as_nanos(),"preemptible":true,
        "scope":"fresh owned publisher child; normal parent, Runtime actor, audio/DSP and inference workers unchanged",
        "readyBeforeParentStart":true,"normalParentRequestedQosChanged":false,"publisherQosOverride":false,
        "inputWaitBudgetSeconds":120,"readyWaitBudgetSeconds":120,
        "joinBudget":"ceil fixture duration seconds plus120; bounded completion-channel wait, then post-read/thread-exit poll using same deadline; unverified child output explicit on timeout",
        "order":"policy-ready; parent measured Start/ACK/ReplayInput; child beforeFeed, unchanged feed_observed, bound Stop, portdrop, afterFeed; child policy ends/joins; parent terminal polling",
        "stop":"SID-bound StopHandle; FIRST operation Instant::now before shared admission/control lock",
        "validation":"configured,beforeFeed,afterFeed,post and each invalid-publication snapshot: exact requested ticks/defaultfalse/basic2; raw basic2 alone is not continuous RT proof",
        "snapshots":"sequential OS reads, not atomic; outlier observer after publication timestamps may affect following chunks",
        "fixtureOwnership":"one Arc<Vec<f32>> per loaded fixture; no per-run full PCM copy; StopHandle has no Runtime worker owner",
        "waitPolicy":"unchanged20ms input,10ms budget,1ms std sleep,250us final spin; no Mach waiter substitution",
        "failurePolicy":"retain feed/stop/policy/text/Engine evidence when available; failed policy invalidates run; next rows unavailable; no retry"});
    Ok(value)
}
pub fn context_template(reduced: bool) -> Result<Value, serde_json::Error> {
    Ok(context_configuration::decorate(
        scheduling_template()?,
        reduced,
    ))
}
pub fn pretrans_template(candidate: bool) -> Result<Value, serde_json::Error> {
    Ok(pretranscription::decorate(
        scheduling_template()?,
        candidate,
    ))
}
pub fn full_prefix_template(candidate: bool) -> Result<Value, serde_json::Error> {
    Ok(full_prefix_configuration::decorate(
        scheduling_template()?,
        candidate,
    ))
}
impl ExecutionMode {
    pub fn spawn_runtime(
        self,
        service: everlast_dictation::inference_service::InferenceService,
        generation: u64,
        format: dictation_core::audio::AudioFormat,
    ) -> Result<
        everlast_dictation::runtime::measurement::MeasuredRuntime,
        everlast_dictation::runtime::RuntimeError,
    > {
        use everlast_dictation::runtime::measurement::MeasuredRuntime;
        if let Some(candidate) = self.full_prefix() {
            return if candidate {
                MeasuredRuntime::spawn_full_prefix_revision(service, generation, format)
            } else {
                MeasuredRuntime::spawn_pretranscription_reference(service, generation, format)
            };
        }
        match self.pretranscription() {
            Some(true) => MeasuredRuntime::spawn_pretranscription(service, generation, format),
            Some(false) => {
                MeasuredRuntime::spawn_pretranscription_reference(service, generation, format)
            }
            None => MeasuredRuntime::spawn(service, generation, format),
        }
    }

    pub fn full_prefix(self) -> Option<bool> {
        match self {
            Self::FullPrefixReference => Some(false),
            Self::FullPrefixCandidate => Some(true),
            _ => None,
        }
    }

    pub fn pretranscription(self) -> Option<bool> {
        match self {
            Self::PretransReference => Some(false),
            Self::PretransCandidate => Some(true),
            Self::Baseline
            | Self::TimeConstrained
            | Self::ContextReference
            | Self::ContextReduced
            | Self::FullPrefixReference
            | Self::FullPrefixCandidate => None,
        }
    }
    pub fn context_reduced(self) -> Option<bool> {
        match self {
            Self::ContextReference => Some(false),
            Self::ContextReduced => Some(true),
            Self::Baseline
            | Self::TimeConstrained
            | Self::PretransReference
            | Self::PretransCandidate
            | Self::FullPrefixReference
            | Self::FullPrefixCandidate => None,
        }
    }
    pub fn scheduled(self) -> bool {
        match self {
            Self::Baseline => false,
            Self::TimeConstrained
            | Self::ContextReference
            | Self::ContextReduced
            | Self::PretransReference
            | Self::PretransCandidate
            | Self::FullPrefixReference
            | Self::FullPrefixCandidate => true,
        }
    }
}
pub fn print_template(flag: &str) -> Result<Option<Value>, serde_json::Error> {
    Ok(Some(match flag {
        "--print-parameters" => template()?,
        "--print-scheduling-parameters" => scheduling_template()?,
        "--print-context-reference-parameters" => context_template(false)?,
        "--print-context-candidate-parameters" => context_template(true)?,
        "--print-pretrans-reference-parameters" => pretrans_template(false)?,
        "--print-pretrans-candidate-parameters" => pretrans_template(true)?,
        "--print-full-prefix-reference-parameters" => full_prefix_template(false)?,
        "--print-full-prefix-candidate-parameters" => full_prefix_template(true)?,
        _ => return Ok(None),
    }))
}
pub fn select(bytes: &[u8]) -> Result<(Value, ExecutionMode), String> {
    if bytes.len() > 1024 * 1024 {
        return Err("parameter document exceeds1MiB".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid parameter JSON")?;
    for (template, mode) in [
        (
            full_prefix_template(false).map_err(|e| e.to_string())?,
            ExecutionMode::FullPrefixReference,
        ),
        (
            full_prefix_template(true).map_err(|e| e.to_string())?,
            ExecutionMode::FullPrefixCandidate,
        ),
        (
            pretrans_template(false).map_err(|e| e.to_string())?,
            ExecutionMode::PretransReference,
        ),
        (
            pretrans_template(true).map_err(|e| e.to_string())?,
            ExecutionMode::PretransCandidate,
        ),
        (
            template().map_err(|e| e.to_string())?,
            ExecutionMode::Baseline,
        ),
        (
            scheduling_template().map_err(|e| e.to_string())?,
            ExecutionMode::TimeConstrained,
        ),
        (
            context_template(false).map_err(|e| e.to_string())?,
            ExecutionMode::ContextReference,
        ),
        (
            context_template(true).map_err(|e| e.to_string())?,
            ExecutionMode::ContextReduced,
        ),
    ] {
        if value == template {
            return Ok((value, mode));
        }
    }
    Err("parameter document differs from all exact compiled configurations".into())
}
#[allow(
    dead_code,
    reason = "legacy validation entry point is also reused by other developer executables; this executable selects a typed configuration"
)]
pub fn validate(bytes: &[u8]) -> Result<Value, String> {
    select(bytes).map(|(v, _)| v)
}

fn compiled_sources() -> Value {
    macro_rules! source {
        ($name:literal, $file:literal) => {
            json!({"filename": $name, "sha256": input::digest(include_bytes!($file))})
        };
    }
    json!({
        "pretransConfiguration":source!("src-tauri/examples/paced_runtime_probe/pretranscription_configuration.rs","pretranscription_configuration.rs"),
        "pretransAudit":source!("src-tauri/examples/paced_runtime_probe/pretranscription.rs","pretranscription.rs"),
        "jobTrace":source!("src-tauri/src/runtime/measurement/job_trace.rs","../../src/runtime/measurement/job_trace.rs"),
        "workerObservation":source!("src-tauri/src/inference_service/observation.rs","../../src/inference_service/observation.rs"),
        "pausePolicy":source!("crates/dictation-core/src/audio/pauses.rs","../../../crates/dictation-core/src/audio/pauses.rs"),
        "prefixOwnership":source!("src-tauri/src/audio/pretranscription.rs","../../src/audio/pretranscription.rs"),
        "controller":source!("src-tauri/src/controller.rs","../../src/controller.rs"),
        "sessionLedger":source!("src-tauri/src/controller/pretranscription.rs","../../src/controller/pretranscription.rs"),
        "nativeRealtime":source!("crates/macos-integration/src/realtime.rs","../../../crates/macos-integration/src/realtime.rs"),
        "publisher":source!("src-tauri/examples/paced_runtime_probe/publisher.rs","publisher.rs"),
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
    fn full_prefix_modes_require_their_exact_own_configuration_and_source_closure()
    -> Result<(), Box<dyn std::error::Error>> {
        for candidate in [false, true] {
            let flag = if candidate {
                "--print-full-prefix-candidate-parameters"
            } else {
                "--print-full-prefix-reference-parameters"
            };
            let frozen = print_template(flag)?.ok_or("full-prefix configuration unavailable")?;
            assert_eq!(frozen["fullPrefix"]["candidate"], candidate);
            assert_eq!(
                frozen["measurement"]["mode"],
                "paced-full-prefix-revision-v1"
            );
            assert!(frozen.get("pretranscription").is_none());
            let selected = select(&serde_json::to_vec(&frozen)?)?.1;
            assert_eq!(selected.full_prefix(), Some(candidate));
            assert!(selected.scheduled());
            assert_eq!(selected.pretranscription(), None);
            assert_eq!(selected.context_reduced(), None);
            for key in [
                "fullPrefixAudio",
                "fullPrefixController",
                "preparedCache",
                "cacheEvidence",
                "fullPrefixAudit",
                "fullPrefixConfiguration",
            ] {
                assert!(
                    frozen["sourceHashes"][key]["sha256"]
                        .as_str()
                        .is_some_and(|value| value.len() == 64),
                    "missing source binding {key}"
                );
            }
            let mut wrong = frozen;
            wrong["fullPrefix"]["confirmationFrames"] = json!(7999);
            assert!(select(&serde_json::to_vec(&wrong)?).is_err());
        }
        Ok(())
    }
    #[test]
    fn explicit_pretranscription_freezes_select_both_arms_without_context_or_legacy_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        for (candidate, mode) in [
            (false, ExecutionMode::PretransReference),
            (true, ExecutionMode::PretransCandidate),
        ] {
            let frozen = pretrans_template(candidate)?;
            assert_eq!(select(&serde_json::to_vec(&frozen)?)?.1, mode);
            assert_eq!(mode.pretranscription(), Some(candidate));
            assert!(mode.scheduled());
            assert_eq!(mode.context_reduced(), None);
            let mut mixed = frozen;
            mixed["pretranscription"]["confirmationFrames"] = json!(7999);
            assert!(select(&serde_json::to_vec(&mixed)?).is_err());
        }
        Ok(())
    }
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
    #[test]
    fn explicit_tc_freeze_is_distinct_and_selects_only_the_exact_compiled_mode()
    -> Result<(), Box<dyn std::error::Error>> {
        let legacy = template()?;
        let scheduled = scheduling_template()?;
        assert_ne!(legacy, scheduled);
        assert_eq!(
            select(&serde_json::to_vec(&legacy)?)?.1,
            ExecutionMode::Baseline
        );
        assert_eq!(
            select(&serde_json::to_vec(&scheduled)?)?.1,
            ExecutionMode::TimeConstrained
        );
        let mut changed = scheduled;
        changed["measurement"]["publisherThread"]["constraintNanos"] = json!(2_000_000);
        assert!(select(&serde_json::to_vec(&changed)?).is_err());
        Ok(())
    }
    #[test]
    fn context_modes_are_explicit_exact_tc_and_cannot_select_legacy()
    -> Result<(), Box<dyn std::error::Error>> {
        let reference = context_template(false)?;
        let reduced = context_template(true)?;
        assert_ne!(reference, reduced);
        assert_eq!(
            select(&serde_json::to_vec(&reference)?)?.1,
            ExecutionMode::ContextReference
        );
        assert_eq!(
            select(&serde_json::to_vec(&reduced)?)?.1,
            ExecutionMode::ContextReduced
        );
        let mut wrong = reduced;
        wrong["audioContextExperiment"]["maximumPreparedFrames"] = json!(320001);
        assert!(select(&serde_json::to_vec(&wrong)?).is_err());
        Ok(())
    }
}
