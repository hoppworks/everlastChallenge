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

pub use baseline::ExecutionMode;
#[cfg(feature = "mlx")]
pub use baseline::load_mlx_service;
pub fn scheduling_template() -> Result<Value, serde_json::Error> {
    let mut value = template()?;
    value["configurationId"] = json!("paced-six-case-native-timestamps-owned-tc-v3");
    if let Some(object) = value["measurement"].as_object_mut() {
        object.remove("publisherQos");
    }
    value["measurement"]["publisherThread"] =
        baseline::scheduling_template()?["measurement"]["publisherThread"].clone();
    Ok(value)
}
pub fn mlx_template() -> Result<Value, serde_json::Error> {
    let mut value = baseline::mlx_template()?;
    let batch = scheduling_template()?;
    value["measurement"] = batch["measurement"].clone();
    value["measurement"]["variant"] = json!("complete-recording-mlx-reference-encoder");
    value["batchSources"] = sources();
    value["limits"] = batch["limits"].clone();
    value["configurationId"] = json!("native-mlx-reference-encoder-six-case-v1");
    Ok(value)
}
pub fn context_template(reduced: bool) -> Result<Value, serde_json::Error> {
    let mut value = baseline::context_template(reduced)?;
    let batch = template()?;
    value["measurement"] = batch["measurement"].clone();
    if let Some(m) = value["measurement"].as_object_mut() {
        m.remove("publisherQos");
    }
    value["measurement"]["publisherThread"] =
        scheduling_template()?["measurement"]["publisherThread"].clone();
    value["batchSources"] = sources();
    value["limits"] = batch["limits"].clone();
    value["configurationId"] = json!(if reduced {
        "p06-six-case-context-reduced-v1"
    } else {
        "p06-six-case-context-reference-v1"
    });
    Ok(value)
}
pub fn pretrans_template(candidate: bool) -> Result<Value, serde_json::Error> {
    let mut value = baseline::pretrans_template(candidate)?;
    let batch = template()?;
    let publisher = value["measurement"]["publisherThread"].clone();
    value["measurement"] = batch["measurement"].clone();
    if let Some(m) = value["measurement"].as_object_mut() {
        m.remove("publisherQos");
    }
    value["measurement"]["publisherThread"] = publisher;
    value["measurement"]["mode"] = json!("paced-runtime-pretranscription-experiment");
    value["measurement"]["variant"] = json!(if candidate {
        "confirmed-pause-pretranscription"
    } else {
        "matched-full-recording-reference"
    });
    value["batchSources"] = sources();
    value["limits"] = batch["limits"].clone();
    value["configurationId"] = json!(if candidate {
        "p06-six-case-pretranscription-candidate-v1"
    } else {
        "p06-six-case-pretranscription-reference-v1"
    });
    Ok(value)
}
/// The existing full-prefix policy and decoder/source closure, decorated only
/// with the unchanged one-warmup/six-by-thirty batch protocol.
pub fn full_prefix_template(candidate: bool) -> Result<Value, serde_json::Error> {
    let mut value = baseline::full_prefix_template(candidate)?;
    let batch = template()?;
    if let Some(measurement) = value["measurement"].as_object_mut() {
        measurement.remove("measuredRuns");
    }
    for key in [
        "warmupRuns",
        "warmupFixture",
        "measuredRunsPerCase",
        "totalRows",
        "caseOrder",
        "runOrder",
        "manifest",
        "L",
        "absoluteLimits",
        "hypotheses",
    ] {
        value["measurement"][key] = batch["measurement"][key].clone();
    }
    value["batchSources"] = sources();
    value["limits"] = batch["limits"].clone();
    value["status"] = json!("six-case full-prefix experiment; no quality or P05/P06 qualification");
    value["configurationId"] = json!(if candidate {
        "p06-six-case-full-prefix-candidate-v1"
    } else {
        "p06-six-case-full-prefix-reference-v1"
    });
    Ok(value)
}
pub fn print_template(flag: &str) -> Result<Option<Value>, serde_json::Error> {
    Ok(Some(match flag {
        "--print-full-prefix-reference-parameters" => full_prefix_template(false)?,
        "--print-full-prefix-candidate-parameters" => full_prefix_template(true)?,
        "--print-parameters" => template()?,
        #[cfg(feature = "mlx")]
        "--print-mlx-reference-encoder-parameters" => mlx_template()?,
        "--print-scheduling-parameters" => scheduling_template()?,
        "--print-context-reference-parameters" => context_template(false)?,
        "--print-context-candidate-parameters" => context_template(true)?,
        "--print-pretrans-reference-parameters" => pretrans_template(false)?,
        "--print-pretrans-candidate-parameters" => pretrans_template(true)?,
        _ => return Ok(None),
    }))
}
pub fn select(bytes: &[u8]) -> Result<(Value, ExecutionMode), String> {
    if bytes.len() > 1024 * 1024 {
        return Err("parameter document exceeds1MiB".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid parameter JSON")?;
    for (expected, mode) in [
        (mlx_template(), ExecutionMode::MlxReferenceEncoder),
        (
            full_prefix_template(false),
            ExecutionMode::FullPrefixReference,
        ),
        (
            full_prefix_template(true),
            ExecutionMode::FullPrefixCandidate,
        ),
        (pretrans_template(false), ExecutionMode::PretransReference),
        (pretrans_template(true), ExecutionMode::PretransCandidate),
        (template(), ExecutionMode::Baseline),
        (scheduling_template(), ExecutionMode::TimeConstrained),
        (context_template(false), ExecutionMode::ContextReference),
        (context_template(true), ExecutionMode::ContextReduced),
    ] {
        if value == expected.map_err(|e| e.to_string())? {
            return Ok((value, mode));
        }
    }
    Err("parameter document differs from all compiled six-case configurations".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pretranscription_print_freezes_keep_six_case_protocol_and_exact_typed_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        for (flag, candidate, mode) in [
            (
                "--print-pretrans-reference-parameters",
                false,
                ExecutionMode::PretransReference,
            ),
            (
                "--print-pretrans-candidate-parameters",
                true,
                ExecutionMode::PretransCandidate,
            ),
        ] {
            let frozen = print_template(flag)?.ok_or("missing explicit print template")?;
            assert_eq!(select(&serde_json::to_vec(&frozen)?)?.1, mode);
            assert_eq!(mode.pretranscription(), Some(candidate));
            assert!(mode.scheduled());
            assert_eq!(frozen["measurement"]["totalRows"], 181);
            assert_eq!(frozen["measurement"]["measuredRunsPerCase"], 30);
            let mut altered = frozen;
            altered["pretranscription"]["tailFrames"] = json!(3199);
            assert!(select(&serde_json::to_vec(&altered)?).is_err());
        }
        assert!(print_template("--print-pretrans-unknown")?.is_none());
        Ok(())
    }
    #[test]
    fn freeze_rejects_changed_counts_model_parameters_sources_and_timing_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = template()?;
        assert!(select(&serde_json::to_vec(&expected)?).is_ok());
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
            assert!(select(&serde_json::to_vec(&wrong)?).is_err());
        }
        assert!(select(b"{}").is_err());
        assert!(select(b"invalid").is_err());
        Ok(())
    }
    #[test]
    fn six_case_tc_selection_preserves_counts_and_rejects_mixed_freezes()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = template()?;
        let scheduled = scheduling_template()?;
        assert!(
            original != scheduled,
            "scheduling freeze must be explicit and distinct"
        );
        assert_eq!(
            select(&serde_json::to_vec(&scheduled)?)?.1,
            ExecutionMode::TimeConstrained
        );
        assert_eq!(scheduled["measurement"]["totalRows"], 181);
        assert_eq!(scheduled["measurement"]["warmupRuns"], 1);
        assert_eq!(scheduled["measurement"]["measuredRunsPerCase"], 30);
        let mut mixed = scheduled;
        mixed["configurationId"] = original["configurationId"].clone();
        assert!(select(&serde_json::to_vec(&mixed)?).is_err());
        Ok(())
    }
    #[test]
    fn six_case_context_freezes_select_tc_variants_and_bind_full_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        for (reduced, mode) in [
            (false, ExecutionMode::ContextReference),
            (true, ExecutionMode::ContextReduced),
        ] {
            let value = context_template(reduced)?;
            assert_eq!(select(&serde_json::to_vec(&value)?)?.1, mode);
            assert!(mode.scheduled());
            assert_eq!(mode.context_reduced(), Some(reduced));
            let mut wrong = value;
            wrong["measurement"]["totalRows"] = json!(180);
            assert!(select(&serde_json::to_vec(&wrong)?).is_err());
        }
        Ok(())
    }
}

#[cfg(test)]
mod full_prefix_batch_tests {
    use super::*;
    #[test]
    fn explicit_full_prefix_freezes_keep_batch_counts_and_select_exact_overlap_modes()
    -> Result<(), Box<dyn std::error::Error>> {
        for (flag, candidate, mode) in [
            (
                "--print-full-prefix-reference-parameters",
                false,
                ExecutionMode::FullPrefixReference,
            ),
            (
                "--print-full-prefix-candidate-parameters",
                true,
                ExecutionMode::FullPrefixCandidate,
            ),
        ] {
            let value =
                print_template(flag)?.ok_or("missing explicit FullPrefix batch template")?;
            assert_eq!(select(&serde_json::to_vec(&value)?)?.1, mode);
            assert_eq!(mode.full_prefix(), Some(candidate));
            assert!(mode.pretranscription().is_none());
            assert!(mode.scheduled());
            assert_eq!(value["measurement"]["totalRows"], 181);
            assert_eq!(value["measurement"]["warmupRuns"], 1);
            assert_eq!(value["measurement"]["measuredRunsPerCase"], 30);
            assert_eq!(value["fullPrefix"]["candidate"], candidate);
            assert!(
                value["measurement"]["engineRtf"]
                    .as_str()
                    .ok_or("missing RTF scope")?
                    .contains("unique final")
            );
            assert_eq!(
                value["batchSources"]["main"]["sha256"],
                input::digest(include_bytes!("../paced_reference_probe.rs"))
            );
            assert_eq!(
                value["batchSources"]["parameters"]["sha256"],
                input::digest(include_bytes!("parameters.rs"))
            );
            for target in ["count", "cut", "mode", "pin"] {
                let mut wrong = value.clone();
                match target {
                    "count" => wrong["measurement"]["totalRows"] = json!(180),
                    "cut" => wrong["fullPrefix"]["tailFrames"] = json!(3199),
                    "mode" => wrong["fullPrefix"]["candidate"] = json!(!candidate),
                    _ => wrong["batchSources"]["main"]["sha256"] = json!("changed"),
                };
                assert!(select(&serde_json::to_vec(&wrong)?).is_err());
            }
        }
        assert!(print_template("--print-full-prefix-unknown")?.is_none());
        Ok(())
    }
}
