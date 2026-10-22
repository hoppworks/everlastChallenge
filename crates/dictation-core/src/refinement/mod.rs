//! Bounded, deterministic acceptance of local text edits. No model or OS access.

mod correction;
mod literals;
mod policy;
mod punctuation;
mod text;
mod validator;

pub use policy::{ProcessingDecision, RefinementSignal, SkipReason, decide};

/// Maximum UTF-8 length of a complete original transcript.
pub const MAX_ORIGINAL_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    German,
    English,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    Filler,
    SelfCorrection,
    PunctuationCase,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProposedEdit {
    pub kind: EditKind,
    pub before: String,
    pub after: String,
    #[serde(default)]
    pub left_context: Option<String>,
    #[serde(default)]
    pub right_context: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct OriginalTranscript(String);

impl OriginalTranscript {
    pub fn new(text: String) -> Result<Self, ValidationError> {
        if text.len() > MAX_ORIGINAL_BYTES {
            return Err(ValidationError::Limit);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AcceptedTranscript {
    text: String,
    edit_count: usize,
}

impl AcceptedTranscript {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn edit_count(&self) -> usize {
        self.edit_count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("Text refinement exceeds its limits")]
    Limit,
    #[error("Text edit could not be safely validated")]
    InvalidEdit,
}

pub fn validate(
    original: &OriginalTranscript,
    edits: &[ProposedEdit],
    language: Language,
) -> Result<AcceptedTranscript, ValidationError> {
    validator::validate(original, edits, language)
}
