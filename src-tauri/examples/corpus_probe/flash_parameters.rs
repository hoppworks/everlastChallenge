//! Exact file-backend Flash-Attention freeze; never changes production defaults.
use crate::input;
use serde_json::Value;
use serde_json::json;
pub fn template(enabled: bool) -> Result<Value, serde_json::Error> {
    let mut value = super::context_parameters::template(false)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("audioContextExperiment");
    }
    value["configurationId"] = json!(if enabled {
        "p06-corpus-flash-candidate-v1"
    } else {
        "p06-corpus-flash-reference-v1"
    });
    value["runtime"]["context"]["flash_attn"] = json!(enabled);
    value["flashAttentionExperiment"] = json!({
        "variant": if enabled { "FlashCandidate" } else { "FlashReference" },
        "expectedModelAudioCtx":1500,"audioCtxParameter":0,
        "newContextPerVariant":true,"contextMutation":false,"noSegmentation":true,
        "modelInitializationOwner":"developer-main-before-serial-worker",
        "sharedWrapperBothVariants":true,
        "records":"one bounded armed/inflight/completed receipt; actual Session/Generation/Job/Range checked before submit and on result; selected flag distinct from attempted native decode",
        "decoderTimingScope":"same serial Decoder::decode wrapper and reference-context preparation/recording; not pure ASR or real-time L",
        "gpuCoverageProven":false,"bitEqualityProven":false,"resourceQualified":false
    });
    value["measurement"]["order"] = json!(
        "manifest order; one warm serial worker; first failed experiment job blocks later rows"
    );
    value["sourceHashes"]["corpusFlashParameters"] = json!({"filename":"src-tauri/examples/corpus_probe/flash_parameters.rs","sha256":input::digest(include_bytes!("flash_parameters.rs"))});
    value["sourceHashes"]["contextWrapper"] = json!({"filename":"vendor/whisper-rs/src/whisper_ctx.rs","sha256":input::digest(include_bytes!("../../../vendor/whisper-rs/src/whisper_ctx.rs"))});
    value["sourceHashes"]["contextOwner"] = json!({"filename":"vendor/whisper-rs/src/whisper_ctx_wrapper.rs","sha256":input::digest(include_bytes!("../../../vendor/whisper-rs/src/whisper_ctx_wrapper.rs"))});
    Ok(value)
}
pub fn validate(bytes: &[u8], enabled: bool) -> Result<Value, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("flash parameters exceed1MiB".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid flash parameter JSON")?;
    if value != template(enabled).map_err(|e| e.to_string())? {
        return Err("flash parameters differ from the exact explicitly selected variant".into());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_flash_parameters_reject_wrong_flag_context_and_source()
    -> Result<(), Box<dyn std::error::Error>> {
        for enabled in [false, true] {
            let value = template(enabled)?;
            assert!(validate(&serde_json::to_vec(&value)?, enabled).is_ok());
            assert!(validate(&serde_json::to_vec(&value)?, !enabled).is_err());
            for key in ["flag", "audio_ctx", "source"] {
                let mut changed = value.clone();
                match key {
                    "flag" => changed["runtime"]["context"]["flash_attn"] = json!(!enabled),
                    "audio_ctx" => changed["runtime"]["fullParams"]["audio_ctx"] = json!(1024),
                    _ => {
                        changed["sourceHashes"]["corpusFlashParameters"]["sha256"] = json!("wrong")
                    }
                }
                assert!(validate(&serde_json::to_vec(&changed)?, enabled).is_err());
            }
        }
        assert!(validate(&vec![b' '; 1024 * 1024 + 1], true).is_err());
        assert!(validate(b"{}", true).is_err());
        Ok(())
    }
    #[test]
    fn paired_flash_freezes_differ_only_in_context_flag_and_variant_labels()
    -> Result<(), Box<dyn std::error::Error>> {
        let reference = template(false)?;
        let mut candidate = template(true)?;
        assert_eq!(reference["runtime"]["context"]["flash_attn"], false);
        assert_eq!(candidate["runtime"]["context"]["flash_attn"], true);
        assert_eq!(candidate["runtime"]["fullParams"]["audio_ctx"], 0);
        assert!(candidate.get("audioContextExperiment").is_none());
        assert_eq!(
            candidate["flashAttentionExperiment"]["expectedModelAudioCtx"],
            1500
        );
        assert_eq!(
            candidate["flashAttentionExperiment"]["noSegmentation"],
            true
        );
        candidate["runtime"]["context"]["flash_attn"] = json!(false);
        candidate["configurationId"] = reference["configurationId"].clone();
        candidate["flashAttentionExperiment"]["variant"] =
            reference["flashAttentionExperiment"]["variant"].clone();
        assert_eq!(candidate, reference);
        Ok(())
    }
}
