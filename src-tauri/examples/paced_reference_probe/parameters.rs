//! Exact pre-run freeze of the six-case protocol and compiled execution sources.
#[allow(
    dead_code,
    reason = "reuse the compiled runtime template; the single-case validator does not govern this batch protocol"
)]
#[path = "../paced_runtime_probe/parameters.rs"]
mod baseline;
use crate::{batch, input};
use serde_json::{Value, json};
pub fn template() -> Result<Value, serde_json::Error> {
    let mut value = baseline::template()?;
    value["configurationId"] = json!("paced-six-case-native-timestamps-reference-v2");
    value["status"] = json!("six-case L measurement only; no quality or P05 qualification");
    value["limits"] = json!({"noNativeDelivery":true,"notMicrophoneAcceptance":true,"notFullP05Qualification":true,
        "aggregateLRequiresAll181RunsAndConfirmedJoin":true,"qualityAndAnchorReviewSeparate":true});
    let measurement = &mut value["measurement"];
    if let Some(object) = measurement.as_object_mut() {
        object.remove("measuredRuns");
    }
    measurement["warmupRuns"] = json!(1);
    measurement["warmupFixture"] = json!("first canonical case: de5");
    measurement["measuredRunsPerCase"] = json!(batch::RUNS_PER_CASE);
    measurement["totalRows"] = json!(batch::TOTAL_RUNS);
    measurement["caseOrder"] = json!(["de5", "de10", "de15", "en5", "en10", "en15"]);
    measurement["runOrder"] =
        json!("one warmup; case-major, exactly thirty sequential runs per case; no retries");
    measurement["manifest"] = json!(
        "exactly six unique clean acceptance cases, DE/EN x5/10/15s at16000Hz; hashed assets preloaded before model load"
    );
    measurement["L"] = json!(
        "unweighted arithmetic mean of the six stop-to-text medians, only after all181 rowsok and cleanup join"
    );
    measurement["absoluteLimits"] =
        json!({"caseMedianNanos":1000000000u64,"caseP95Nanos":1500000000u64});
    measurement["hypotheses"] = json!(
        "private per-slot text for separate published-reference WER and original-content review; no exact-reference equality required by timing harness"
    );
    value["batchSources"] = sources();
    Ok(value)
}
fn sources() -> Value {
    macro_rules! source {($name:literal,$path:literal)=>{json!({"filename":$name,"sha256":input::digest(include_bytes!($path))})};}
    json!({
        "main":source!("src-tauri/examples/paced_reference_probe.rs","../paced_reference_probe.rs"),
        "batch":source!("src-tauri/examples/paced_reference_probe/batch.rs","batch.rs"),
        "manifest":source!("src-tauri/examples/paced_reference_probe/manifest.rs","manifest.rs"),
        "options":source!("src-tauri/examples/paced_reference_probe/options.rs","options.rs"),
        "parameters":source!("src-tauri/examples/paced_reference_probe/parameters.rs","parameters.rs"),
        "host":source!("src-tauri/examples/paced_reference_probe/host.rs","host.rs"),
        "report":source!("src-tauri/examples/paced_reference_probe/report.rs","report.rs"),
        "sharedInput":source!("src-tauri/examples/corpus_probe/input.rs","../corpus_probe/input.rs"),
        "sharedParameters":source!("src-tauri/examples/paced_runtime_probe/parameters.rs","../paced_runtime_probe/parameters.rs"),
        "control":source!("src-tauri/src/control.rs","../../src/control.rs"),
        "controller":source!("src-tauri/src/controller.rs","../../src/controller.rs"),
        "coreSession":source!("crates/dictation-core/src/session.rs","../../../crates/dictation-core/src/session.rs"),
        "recordingLimits":source!("crates/dictation-core/src/recording_limits.rs","../../../crates/dictation-core/src/recording_limits.rs"),
        "deliveryToken":source!("crates/macos-integration/src/delivery.rs","../../../crates/macos-integration/src/delivery.rs"),
        "cargoConfig":source!(".cargo/config.toml","../../../.cargo/config.toml"),
        "coreManifest":source!("crates/dictation-core/Cargo.toml","../../../crates/dictation-core/Cargo.toml"),
        "nativeManifest":source!("crates/macos-integration/Cargo.toml","../../../crates/macos-integration/Cargo.toml")
    })
}
pub fn validate(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("parameter document exceeds1MiB".into());
    }
    let supplied: Value = serde_json::from_slice(bytes).map_err(|_| "invalid parameter JSON")?;
    let actual = template().map_err(|_| "compiled parameter template is invalid")?;
    if supplied != actual {
        return Err("parameter document differs from this compiled six-case protocol; freeze before model work".into());
    }
    Ok(supplied)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freeze_rejects_changed_counts_model_parameters_sources_and_timing_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = template()?;
        assert!(validate(&serde_json::to_vec(&expected)?).is_ok());
        assert_eq!(expected["measurement"]["warmupRuns"], 1);
        assert_eq!(expected["measurement"]["totalRows"], 181);
        for field in ["warmup", "runs", "jitter", "decoder", "source"] {
            let mut wrong = expected.clone();
            match field {
                "warmup" => wrong["measurement"]["warmupRuns"] = json!(6),
                "runs" => wrong["measurement"]["measuredRunsPerCase"] = json!(29),
                "jitter" => wrong["measurement"]["jitterToleranceNanos"] = json!(20000000),
                "decoder" => wrong["runtime"]["fullParams"]["n_threads"] = json!(1),
                _ => wrong["batchSources"]["batch"]["sha256"] = json!("changed"),
            }
            assert!(validate(&serde_json::to_vec(&wrong)?).is_err());
        }
        assert!(validate(b"{}").is_err());
        assert!(validate(b"invalid").is_err());
        Ok(())
    }
}
