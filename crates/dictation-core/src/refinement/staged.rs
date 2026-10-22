//! Compose validation proofs without treating an intermediate result as the
//! original dictation. Neither stage can escape the existing content guards.
use super::{
    AcceptedTranscript, EditKind, Language, OriginalTranscript, StagedEdits, ValidationError,
    validate,
};

pub fn validate_staged(
    original: &OriginalTranscript,
    proposal: &StagedEdits,
    language: Language,
) -> Result<AcceptedTranscript, ValidationError> {
    let count = proposal
        .structural
        .len()
        .checked_add(proposal.punctuation.len())
        .filter(|&count| count <= super::validator::MAX_EDITS)
        .ok_or(ValidationError::Limit)?;
    if proposal
        .structural
        .iter()
        .any(|edit| edit.kind == EditKind::PunctuationCase)
        || proposal
            .punctuation
            .iter()
            .any(|edit| edit.kind != EditKind::PunctuationCase)
    {
        return Err(ValidationError::InvalidEdit);
    }
    let structural = validate(original, &proposal.structural, language)?;
    let intermediate = OriginalTranscript::new(structural.text)?;
    let punctuation = validate(&intermediate, &proposal.punctuation, language)?;
    Ok(AcceptedTranscript {
        text: punctuation.text,
        edit_count: count,
        original: original.clone(),
        language,
    })
}
