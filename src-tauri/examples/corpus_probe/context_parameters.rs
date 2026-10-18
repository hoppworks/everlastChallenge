//! Exact corpus-only experiment freeze, sharing all Engine/SDK policy sources.
#[allow(
    dead_code,
    reason = "reuse exact compiled decoder configuration, not the paced execution protocol"
)]
#[path = "../paced_runtime_probe/parameters.rs"]
mod baseline;
use crate::input;
use serde_json::{Value, json};
pub fn template(reduced: bool) -> Result<Value, serde_json::Error> {
    let mut value = baseline::context_template(reduced)?;
    value["configurationId"] = json!(if reduced {
        "p06-corpus-context-reduced-v1"
    } else {
        "p06-corpus-context-reference-v1"
    });
    value["measurement"] = json!({"mode":"file-backend-only","realTimeLatencyProtocolExecuted":false,"maximumCases":500,"noWarmupPerCase":true,"order":"manifest order; one warm serial worker; first failed context job blocks later rows","receiptAdmission":"explicit current Session/Generation/Job/Range before submit; full identity checked on completed result before receipt collection"});
    value["limits"] = json!({"fileBackendOnly":true,"notRealTimeLatency":true,"notResourceQualification":true,"notMicrophoneOrDeliveryAcceptance":true,"privateHypotheses":true,"noPublisherPolicySet":true});
    value["sourceHashes"]["corpusMain"] = json!({"filename":"src-tauri/examples/corpus_probe.rs","sha256":input::digest(include_bytes!("../corpus_probe.rs"))});
    value["sourceHashes"]["corpusContextParameters"] = json!({"filename":"src-tauri/examples/corpus_probe/context_parameters.rs","sha256":input::digest(include_bytes!("context_parameters.rs"))});
    value["sourceHashes"]["corpusInput"] = json!({"filename":"src-tauri/examples/corpus_probe/input.rs","sha256":input::digest(include_bytes!("input.rs"))});
    Ok(value)
}
pub fn validate(bytes: &[u8], reduced: bool) -> Result<Value, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("context parameters exceed1MiB".into());
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| "invalid context parameter JSON")?;
    if value != template(reduced).map_err(|e| e.to_string())? {
        return Err(
            "corpus context parameters differ from the exact explicitly selected variant".into(),
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corpus_context_freeze_is_exact_for_the_selected_variant_and_input_protocol()
    -> Result<(), Box<dyn std::error::Error>> {
        for reduced in [false, true] {
            let value = template(reduced)?;
            assert!(validate(&serde_json::to_vec(&value)?, reduced).is_ok());
            assert!(validate(&serde_json::to_vec(&value)?, !reduced).is_err());
            let mut changed = value.clone();
            changed["audioContextExperiment"]["maximumPreparedFrames"] = json!(320001);
            assert!(validate(&serde_json::to_vec(&changed)?, reduced).is_err());
            let mut changed = value;
            changed["measurement"]["mode"] = json!("paced");
            assert!(validate(&serde_json::to_vec(&changed)?, reduced).is_err());
        }
        Ok(())
    }
}
