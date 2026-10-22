//! One complete model result, independently reconciled against its source.
use crate::{selection, word_grammar};
use dictation_core::refinement::{
    EditKind, OriginalTranscript, StagedEdits, postprocess, proposal_candidates, reconcile,
    validate_staged,
};
use serde_json::Value;
use text_refinement_protocol::{FailureKind, Language};

const MAX_OUTPUT_BYTES: usize = 32 * 1024;

pub struct Plan {
    original: OriginalTranscript,
    language: Language,
    input: Value,
    pattern: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    text: String,
}

impl Plan {
    pub fn new(text: &str, language: Language) -> Result<Self, FailureKind> {
        let original = OriginalTranscript::new(text.into()).map_err(|_| FailureKind::InputLimit)?;
        let mut choices =
            proposal_candidates(&original, language.into()).map_err(|_| FailureKind::InputLimit)?;
        choices.retain(|edit| edit.kind != EditKind::PunctuationCase);
        let options = selection::prompt_options(&original, &choices)?;
        let pattern = word_grammar::pattern(&original, &choices)?;
        let input = serde_json::json!({"language":language,"dictation":text,"choices":options});
        Ok(Self {
            original,
            language,
            input,
            pattern,
        })
    }

    /// The consuming FnOnce boundary admits one model call and no hidden retry.
    pub fn generate(
        self,
        infer: impl FnOnce(&Value, Value) -> Result<String, FailureKind>,
    ) -> Result<StagedEdits, FailureKind> {
        let schema = serde_json::json!({
            "type":"object",
            "properties":{"text":{"type":"string","pattern":self.pattern}},
            "required":["text"],
            "additionalProperties":false
        });
        let response = infer(&self.input, schema)?;
        if response.len() > MAX_OUTPUT_BYTES {
            return Err(FailureKind::InputLimit);
        }
        let output: Output =
            serde_json::from_str(&response).map_err(|_| FailureKind::InvalidCandidate)?;
        let language = self.language.into();
        // Reject invalid model output before any deterministic finishing.
        let proposal = reconcile(&self.original, &output.text, language)
            .map_err(|_| FailureKind::InvalidCandidate)?;
        let accepted = validate_staged(&self.original, &proposal, language)
            .map_err(|_| FailureKind::InvalidCandidate)?;
        let model_text = OriginalTranscript::new(accepted.as_str().into())
            .map_err(|_| FailureKind::InputLimit)?;
        let finishing =
            postprocess(&model_text, language).map_err(|_| FailureKind::InvalidCandidate)?;
        let finished = validate_staged(&model_text, &finishing, language)
            .map_err(|_| FailureKind::InvalidCandidate)?;
        reconcile(&self.original, finished.as_str(), language)
            .map_err(|_| FailureKind::InvalidCandidate)
    }
}
