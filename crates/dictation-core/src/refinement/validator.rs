use std::ops::Range;

use super::{
    AcceptedTranscript, EditKind, Language, MAX_ORIGINAL_BYTES, OriginalTranscript, ProposedEdit,
    ValidationError, text,
};

const MAX_EDITS: usize = 32;
const MAX_EDIT_BYTES: usize = 2048;
const MAX_CONTEXT_BYTES: usize = 128;

struct ResolvedEdit {
    range: Range<usize>,
    replacement: String,
    needs_sentence_start: bool,
}

pub(super) fn validate(
    original: &OriginalTranscript,
    edits: &[ProposedEdit],
    language: Language,
) -> Result<AcceptedTranscript, ValidationError> {
    if edits.len() > MAX_EDITS {
        return Err(ValidationError::Limit);
    }
    let source = original.as_str();
    let quoted = text::quoted_ranges(source);
    let literals = super::literals::ranges(source);
    let mut resolved = Vec::with_capacity(edits.len());
    for edit in edits {
        let range = locate(source, edit)?;
        if !text::whole_words(source, &range)
            || quoted
                .iter()
                .any(|quote| range.start < quote.end && range.end > quote.start)
        {
            return Err(ValidationError::InvalidEdit);
        }
        if literals.iter().any(|literal| {
            range.start < literal.end
                && range.end > literal.start
                && (edit.kind != EditKind::SelfCorrection
                    || range.start > literal.start
                    || range.end < literal.end)
        }) {
            return Err(ValidationError::InvalidEdit);
        }
        resolved.push(match edit.kind {
            EditKind::Filler => deletion(source, range, edit, language)?,
            EditKind::SelfCorrection
                if super::correction::allowed(&edit.before, &edit.after, language) =>
            {
                ResolvedEdit {
                    range,
                    replacement: edit.after.clone(),
                    needs_sentence_start: false,
                }
            }
            EditKind::PunctuationCase => {
                let needs_sentence_start = super::punctuation::analyze(&edit.before, &edit.after)
                    .ok_or(ValidationError::InvalidEdit)?;
                ResolvedEdit {
                    range,
                    replacement: edit.after.clone(),
                    needs_sentence_start,
                }
            }
            _ => return Err(ValidationError::InvalidEdit),
        });
    }
    resolved.sort_by_key(|edit| edit.range.start);
    if resolved
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return Err(ValidationError::InvalidEdit);
    }
    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in &resolved {
        result.push_str(&source[cursor..edit.range.start]);
        if edit.needs_sentence_start && !super::punctuation::sentence_start(&result) {
            return Err(ValidationError::InvalidEdit);
        }
        result.push_str(&edit.replacement);
        cursor = edit.range.end;
        if result.len() > MAX_ORIGINAL_BYTES {
            return Err(ValidationError::Limit);
        }
    }
    result.push_str(&source[cursor..]);
    if result.len() > MAX_ORIGINAL_BYTES {
        return Err(ValidationError::Limit);
    }
    Ok(AcceptedTranscript {
        text: result,
        edit_count: resolved.len(),
    })
}

fn locate(source: &str, edit: &ProposedEdit) -> Result<Range<usize>, ValidationError> {
    if edit.before.is_empty() || edit.before == edit.after {
        return Err(ValidationError::InvalidEdit);
    }
    if edit.before.len() > MAX_EDIT_BYTES
        || edit.after.len() > MAX_EDIT_BYTES
        || edit
            .left_context
            .as_ref()
            .is_some_and(|s| s.len() > MAX_CONTEXT_BYTES)
        || edit
            .right_context
            .as_ref()
            .is_some_and(|s| s.len() > MAX_CONTEXT_BYTES)
    {
        return Err(ValidationError::Limit);
    }
    // Advance one Unicode scalar after each match, including overlapping matches.
    // The returned boundaries always originate from the original UTF-8 string.
    let mut cursor = 0;
    let mut found = None;
    while let Some(relative) = source[cursor..].find(&edit.before) {
        let start = cursor + relative;
        let end = start + edit.before.len();
        if edit
            .left_context
            .as_ref()
            .is_none_or(|s| source[..start].ends_with(s))
            && edit
                .right_context
                .as_ref()
                .is_none_or(|s| source[end..].starts_with(s))
        {
            if found.is_some() {
                return Err(ValidationError::InvalidEdit);
            }
            found = Some(start..end);
        }
        let step = source[start..]
            .chars()
            .next()
            .ok_or(ValidationError::InvalidEdit)?
            .len_utf8();
        cursor = start + step;
    }
    found.ok_or(ValidationError::InvalidEdit)
}

fn deletion(
    source: &str,
    mut range: Range<usize>,
    edit: &ProposedEdit,
    language: Language,
) -> Result<ResolvedEdit, ValidationError> {
    let words = text::words(&edit.before);
    let retained_comma = edit.after == ","
        && edit.before.trim().starts_with(',')
        && edit.before.trim().ends_with(',')
        && edit.before.matches(',').count() >= 2;
    if (!edit.after.is_empty() && !retained_comma)
        || words.is_empty()
        || !words.iter().all(|word| text::filler(word, language))
        || edit
            .before
            .chars()
            .any(|c| !text::word_character(c) && !matches!(c, ' ' | '\t' | ','))
    {
        return Err(ValidationError::InvalidEdit);
    }
    // A proposal must own both sides of a comma-delimited hesitation. Leaving
    // an outside comma is underspecified: it might be a list separator or an
    // orphan. The proposer must explicitly remove or retain that comma.
    let owns_left_comma = edit.before.trim_start().starts_with(',');
    let owns_right_comma = edit.before.trim_end().ends_with(',');
    let has_left_comma = owns_left_comma
        || source[..range.start]
            .trim_end_matches([' ', '\t'])
            .ends_with(',');
    let has_right_comma = owns_right_comma
        || source[range.end..]
            .trim_start_matches([' ', '\t'])
            .starts_with(',');
    if has_left_comma && has_right_comma && !(owns_left_comma && owns_right_comma) {
        return Err(ValidationError::InvalidEdit);
    }
    while range.start > 0 && matches!(source.as_bytes()[range.start - 1], b' ' | b'\t') {
        range.start -= 1;
    }
    while range.end < source.len() && matches!(source.as_bytes()[range.end], b' ' | b'\t') {
        range.end += 1;
    }
    let left = source[..range.start].chars().next_back();
    let right = source[range.end..].chars().next();
    if retained_comma && (left.is_none() || right.is_none()) {
        return Err(ValidationError::InvalidEdit);
    }
    let needs_separator = left.is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | '[' | '{'))
        && right.is_some_and(|c| {
            !c.is_whitespace() && !matches!(c, '.' | ',' | '?' | '!' | ';' | ':' | ')' | ']' | '}')
        });
    Ok(ResolvedEdit {
        range,
        replacement: if needs_separator {
            format!("{} ", edit.after)
        } else {
            edit.after.clone()
        },
        needs_sentence_start: false,
    })
}
