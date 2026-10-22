//! Freeze the actual runtime configuration before any resource workload.
#[allow(
    dead_code,
    reason = "resource workload freezes the baseline template; it does not select paced publisher execution"
)]
#[path = "../paced_runtime_probe/parameters.rs"]
mod baseline;

use crate::{input, runner};
use serde_json::{Value, json};

pub fn template() -> Result<Value, serde_json::Error> {
    Ok(json!({
        "schemaVersion":1,
        "configurationId":"runtime-lifecycle-resource-native-timestamps-v2",
        "runtimeBaseline":baseline::template()?,
        "baselineLatencyProtocolExecuted":false,
        "workload":{
            "warmupStopSessions":runner::WARMUP_RUNS,
            "measuredSessions":runner::MEASURED_RUNS,
            "order":["StopComplete","CancelCapture","CancelDecoderEntry"],
            "language":"de",
            "captureCancelFrames":320,
            "sampleRate":16000,
            "decoderCancellation":"hold wrapper entry before real Engine decode, cancel runtime, release real decode, discard cancelled result",
            "textCheck":"stop result equals supplied prior baseline; no independent accuracy claim",
            "decoderInputCheck":"every complete-input wrapper invocation equals one shared immutable fixture; capture-cancel has no decoder comparison",
            "fixtureOwnership":"one shared original fixture plus bounded production journals; no per-row fixture copies",
            "metrics":"RSS after each completed cycle; bounded vmmap summary after warmup and last cycle",
            "failurePolicy":"preserve failed and unavailable rows; no silent retry of a failed cycle",
            "timingScope":"resource workload with deliberate barriers and checkpoint subprocesses; no L benchmark"
        },
        "resourceQualified":false,
        "limits":["RSS is resident memory, not live heap allocations",
            "no arbitrary RSS threshold converts observations into a leak qualification",
            "decoder wrapper entry is not an instrumented native kernel boundary",
            "no microphone, hotkey, native paste or installed bundle acceptance"],
        "workloadSources":{
            "probe":source("src-tauri/examples/runtime_resource_probe.rs",include_bytes!("../runtime_resource_probe.rs")),
            "runner":source("src-tauri/examples/runtime_resource_probe/runner.rs",include_bytes!("runner.rs")),
            "metrics":source("src-tauri/examples/runtime_resource_probe/metrics.rs",include_bytes!("metrics.rs")),
            "parameters":source("src-tauri/examples/runtime_resource_probe/parameters.rs",include_bytes!("parameters.rs")),
            "launcher":source("scripts/run-runtime-resource.ts",include_bytes!("../../../scripts/run-runtime-resource.ts")),
            "processOwner":source("scripts/controlled-probe.ts",include_bytes!("../../../scripts/controlled-probe.ts")),
            "groupCleanup":source("scripts/tooling/owned-probe.ts",include_bytes!("../../../scripts/tooling/owned-probe.ts")),
            "fileTools":source("scripts/tooling/files.ts",include_bytes!("../../../scripts/tooling/files.ts")),
            "ioTools":source("scripts/tooling/io.ts",include_bytes!("../../../scripts/tooling/io.ts")),
            "audioTools":source("scripts/tooling/audio-data.ts",include_bytes!("../../../scripts/tooling/audio-data.ts")),
            "packageLock":source("package-lock.json",include_bytes!("../../../package-lock.json"))
        }
    }))
}

fn source(filename: &str, bytes: &[u8]) -> Value {
    json!({"filename":filename,"sha256":input::digest(bytes)})
}

pub fn validate(bytes: &[u8]) -> Result<Value, String> {
    let supplied: Value =
        serde_json::from_slice(bytes).map_err(|_| "invalid bounded resource parameter JSON")?;
    let runtime_bytes = serde_json::to_vec(&supplied["runtimeBaseline"])
        .map_err(|_| "resource runtime configuration could not be encoded")?;
    baseline::validate(&runtime_bytes)?;
    let actual = template().map_err(|_| "compiled resource template is invalid")?;
    let matches = supplied == actual;
    #[cfg(feature = "mlx")]
    let matches = matches || supplied == mlx_template().map_err(|e| e.to_string())?;
    if !matches {
        return Err("resource parameter document differs from this compiled workload; freeze a new document before running".into());
    }
    Ok(supplied)
}

#[cfg(feature = "mlx")]
pub fn mlx_template() -> Result<Value, serde_json::Error> {
    let mut value = template()?;
    value["configurationId"] = json!("runtime-lifecycle-resource-mlx-reference-encoder-v1");
    value["runtimeBaseline"] = baseline::mlx_template()?;
    value["workload"]["backendInitialization"] =
        json!("factory on permanent inference worker; wrapper forwards backend finish");
    Ok(value)
}

pub fn select(
    bytes: &[u8],
    definition: everlast_dictation::model::ModelDefinition,
) -> Result<Value, String> {
    let value = validate(bytes)?;
    let (_, mode) = baseline::select(
        &serde_json::to_vec(&value["runtimeBaseline"]).map_err(|e| e.to_string())?,
    )?;
    mode.validate_model(definition)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "mlx")]
    #[test]
    fn frozen_mlx_resource_workload_rejects_reference_model_or_missing_guard_asset()
    -> Result<(), Box<dyn std::error::Error>> {
        use everlast_dictation::model::{MLX_REFERENCE_COEFFICIENTS, REFERENCE};
        let document = mlx_template()?;
        let bytes = serde_json::to_vec(&document)?;
        assert!(select(&bytes, MLX_REFERENCE_COEFFICIENTS).is_ok());
        assert!(select(&bytes, REFERENCE).is_err());
        assert!(
            select(
                &serde_json::to_vec(&template()?)?,
                MLX_REFERENCE_COEFFICIENTS
            )
            .is_err()
        );
        let mut changed = document;
        changed["runtimeBaseline"]["models"]
            .as_array_mut()
            .ok_or("missing model list")?
            .pop();
        assert!(select(&serde_json::to_vec(&changed)?, MLX_REFERENCE_COEFFICIENTS).is_err());
        Ok(())
    }

    #[test]
    fn frozen_resource_workload_requires_exact_counts_sources_and_runtime_parameters()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = template()?;
        assert_eq!(expected["workload"]["warmupStopSessions"], 5);
        assert_eq!(expected["workload"]["measuredSessions"], 100);
        assert!(validate(&serde_json::to_vec(&expected)?).is_ok());
        for key in ["count", "source", "decoder"] {
            let mut changed = expected.clone();
            match key {
                "count" => changed["workload"]["measuredSessions"] = json!(99),
                "source" => changed["workloadSources"]["runner"]["sha256"] = json!("wrong"),
                _ => changed["runtimeBaseline"]["runtime"]["fullParams"]["n_threads"] = json!(99),
            }
            assert!(validate(&serde_json::to_vec(&changed)?).is_err());
        }
        assert!(validate(b"{broken").is_err());
        Ok(())
    }
}
