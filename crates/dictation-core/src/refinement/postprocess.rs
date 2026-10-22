//! Deterministic finishing after a validated model result. No new word authority.
use super::{
    EditKind, Language, OriginalTranscript, ProposedEdit, StagedEdits, ValidationError, candidates,
    proposal_candidates, propose_punctuation, punctuation, text, validate, validate_staged,
};

/// Finish only explicit digit-to-digit repairs with a strong correction marker.
/// A comma-delimited initial hesitation can also be removed. Names, weak
/// negations, other fillers and free prose still require model judgment.
/// Overlapping repairs are ambiguous and fail closed; no partial correction
/// is returned. The caller must reconcile the result against the original raw transcript.
pub fn postprocess(
    original: &OriginalTranscript,
    language: Language,
) -> Result<StagedEdits, ValidationError> {
    let structural = proposal_candidates(original, language)?
        .into_iter()
        .filter(|edit| {
            if edit.kind == EditKind::Filler {
                let words = text::words(&edit.before);
                return edit.after.is_empty()
                    && edit.before.ends_with(',')
                    && super::reconcile::locate(original.as_str(), edit)
                        .is_ok_and(|span| original.as_str()[..span.start].trim().is_empty())
                    && !words.is_empty()
                    && words.iter().all(|word| text::filler(word, language));
            }
            if edit.kind != EditKind::SelfCorrection {
                return false;
            }
            let words = text::words(&edit.before);
            let [old, marker, new] = words.as_slice() else {
                return false;
            };
            let strong = match language {
                Language::German => matches!(
                    marker.to_lowercase().as_str(),
                    "entschuldigung" | "korrektur"
                ),
                Language::English => {
                    matches!(marker.to_lowercase().as_str(), "sorry" | "correction")
                }
            };
            strong
                && [old, new]
                    .iter()
                    .all(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()))
                && edit.after == *new
        })
        .collect::<Vec<_>>();
    let intermediate = validate(original, &structural, language)?;
    let intermediate = OriginalTranscript::new(intermediate.as_str().into())?;
    let mut capitalization = Vec::new();
    for range in text::word_ranges(intermediate.as_str()) {
        let word = &intermediate.as_str()[range.clone()];
        if !word.chars().all(char::is_lowercase)
            || !punctuation::sentence_start(&intermediate.as_str()[..range.start])
        {
            continue;
        }
        let Some(first) = word.chars().next() else {
            continue;
        };
        let after = first.to_uppercase().collect::<String>() + &word[first.len_utf8()..];
        let (left, right) = candidates::context(intermediate.as_str(), &range);
        let edit = ProposedEdit {
            kind: EditKind::PunctuationCase,
            before: word.into(),
            after,
            left_context: Some(left),
            right_context: Some(right),
        };
        // Protected quotes, names with internal case, and literal atoms retain
        // their exact source spelling; they never gain formatting authority.
        if validate(&intermediate, std::slice::from_ref(&edit), language).is_ok() {
            capitalization.push(edit);
        }
    }
    let capitalized = validate(&intermediate, &capitalization, language)?;
    let capitalized = OriginalTranscript::new(capitalized.as_str().into())?;
    let endpoints = proposal_candidates(&capitalized, language)?
        .into_iter()
        .filter(|edit| edit.kind == EditKind::PunctuationCase)
        .collect::<Vec<_>>();
    let finished = validate(&capitalized, &endpoints, language)?;
    let punctuation = propose_punctuation(&intermediate, finished.as_str(), language)?;
    let result = StagedEdits {
        structural,
        punctuation,
    };
    validate_staged(original, &result, language)?;
    Ok(result)
}
