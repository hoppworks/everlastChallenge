//! Resolve model selections against exact source-bound options, atomically.
use dictation_core::refinement::{EditKind, Language, OriginalTranscript, ProposedEdit, validate};
use text_refinement_protocol::FailureKind;

/// The grammar constrains structure only. Source spans, combined edit limits and
/// byte limits are still checked by `decode` and the complete Rust validator.
pub fn output_schema(choice_count: usize) -> Result<serde_json::Value, FailureKind> {
    if choice_count > 64 {
        return Err(FailureKind::InputLimit);
    }
    Ok(serde_json::json!({
        "type":"object",
        "properties":{
            "decisions":{"type":"array","items":{"type":"boolean"},"minItems":choice_count,"maxItems":choice_count},
            "punctuation":{"type":"array","maxItems":0,"items":false}
        },
        "required":["decisions","punctuation"],
        "additionalProperties":false
    }))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    decisions: Vec<bool>,
    punctuation: Vec<serde_json::Value>,
}

pub fn decode(
    output: &str,
    original: &OriginalTranscript,
    choices: &[ProposedEdit],
    language: Language,
) -> Result<Vec<ProposedEdit>, FailureKind> {
    if output.len() > 32 * 1024 {
        return Err(FailureKind::InputLimit);
    }
    let selection: Selection =
        serde_json::from_str(output).map_err(|_| FailureKind::InvalidCandidate)?;
    if !selection.punctuation.is_empty()
        || selection.decisions.len() != choices.len()
        || selection
            .decisions
            .iter()
            .filter(|selected| **selected)
            .count()
            .saturating_add(selection.punctuation.len())
            > 32
    {
        return Err(FailureKind::InvalidCandidate);
    }
    let mut edits = Vec::new();
    for (index, selected) in selection.decisions.into_iter().enumerate() {
        if selected {
            edits.push(choices[index].clone());
        }
    }
    if edits
        .iter()
        .any(|edit| edit.kind == EditKind::PunctuationCase)
    {
        return Err(FailureKind::InvalidCandidate);
    }
    validate(original, &edits, language).map_err(|_| FailureKind::InvalidCandidate)?;
    Ok(edits)
}

/// Show only the edited span to the model. Validation anchors can include other
/// edit locations and appear to describe overlapping spans. Keep them out of
/// the semantic choice representation.
/// Repeated source text is identified by its one-based occurrence instead.
pub fn prompt_options(
    original: &OriginalTranscript,
    choices: &[ProposedEdit],
) -> Result<Vec<serde_json::Value>, FailureKind> {
    if choices.len() > 64 {
        return Err(FailureKind::InputLimit);
    }
    let source = original.as_str();
    choices.iter().enumerate().map(|(id, edit)| {
        if edit.before.is_empty() { return Err(FailureKind::InvalidCandidate); }
        let mut selected = None;
        let mut count = 0;
        let mut cursor = 0;
        while let Some(relative) = source[cursor..].find(&edit.before) {
            let start = cursor + relative;
            cursor = start + source[start..].chars().next().ok_or(FailureKind::InvalidCandidate)?.len_utf8();
            count += 1;
            let end = start + edit.before.len();
            if edit.left_context.as_ref().is_none_or(|s| source[..start].ends_with(s))
                && edit.right_context.as_ref().is_none_or(|s| source[end..].starts_with(s)) {
                if selected.is_some() { return Err(FailureKind::InvalidCandidate); }
                selected = Some(count);
            }
        }
        let occurrence = selected.ok_or(FailureKind::InvalidCandidate)?;
        let mut value = serde_json::json!({"id":id,"kind":edit.kind,"before":edit.before,"after":edit.after});
        if count > 1 { value["occurrence"] = serde_json::json!(occurrence); }
        Ok(value)
    }).collect()
}
