//! Exact source-bound options for the model to select, never automatic edits.
use super::{
    EditKind, Language, OriginalTranscript, ProposedEdit, ValidationError, text, validate,
};
use std::ops::Range;

const MAX_CANDIDATES: usize = 64;

pub(super) fn context(source: &str, range: &Range<usize>) -> (String, String) {
    let mut left = range.start.saturating_sub(128);
    while !source.is_char_boundary(left) {
        left += 1;
    }
    let mut right = (range.end + 128).min(source.len());
    while !source.is_char_boundary(right) {
        right -= 1;
    }
    (
        source[left..range.start].into(),
        source[range.end..right].into(),
    )
}

pub(super) fn generate(
    original: &OriginalTranscript,
    language: Language,
) -> Result<Vec<ProposedEdit>, ValidationError> {
    let source = original.as_str();
    let literals = super::literals::ranges(source);
    let protected = text::quoted_ranges(source)
        .into_iter()
        .chain(literals.iter().cloned())
        .collect::<Vec<_>>();
    let mut choices = Vec::new();
    let mut groups: Vec<Range<usize>> = Vec::new();
    for word in text::word_ranges(source) {
        if !text::filler(&source[word.clone()], language)
            || protected
                .iter()
                .any(|p| word.start < p.end && word.end > p.start)
        {
            continue;
        }
        if let Some(previous) = groups.last_mut()
            && source[previous.end..word.start]
                .chars()
                .all(|c| matches!(c, ' ' | '\t' | ','))
        {
            previous.end = word.end;
        } else {
            groups.push(word);
        }
    }
    for mut range in groups {
        let left = source[..range.start].trim_end_matches([' ', '\t']);
        if left.ends_with(',') {
            range.start = left.len() - 1;
        }
        let right = &source[range.end..];
        let trimmed = right.trim_start_matches([' ', '\t']);
        if trimmed.starts_with(',') {
            range.end += right.len() - trimmed.len() + 1;
        }
        let (left, right) = context(source, &range);
        let proposal = ProposedEdit {
            kind: EditKind::Filler,
            before: source[range].into(),
            after: String::new(),
            left_context: Some(left),
            right_context: Some(right),
        };
        validate(original, std::slice::from_ref(&proposal), language)?;
        if choices.len() == MAX_CANDIDATES {
            return Err(ValidationError::Limit);
        }
        let can_retain = proposal.before.starts_with(',') && proposal.before.ends_with(',');
        choices.push(proposal.clone());
        if can_retain {
            let retained = ProposedEdit {
                after: ",".into(),
                ..proposal
            };
            if validate(original, std::slice::from_ref(&retained), language).is_ok() {
                if choices.len() == MAX_CANDIDATES {
                    return Err(ValidationError::Limit);
                }
                choices.push(retained);
            }
        }
    }
    let words = text::word_ranges(source);
    for (index, word) in words.iter().enumerate() {
        if protected
            .iter()
            .any(|p| word.start < p.end && word.end > p.start)
        {
            continue;
        }
        for marker in super::correction::markers(language) {
            let end = word.start + marker.len();
            if !source
                .get(word.start..end)
                .is_some_and(|s| s.eq_ignore_ascii_case(marker))
                || !source.get(end..).is_some_and(|s| {
                    s.starts_with(|c: char| c.is_whitespace() || matches!(c, ',' | ':'))
                })
            {
                continue;
            }
            let next = words.partition_point(|range| range.start < end);
            // Every left boundary has at most one shortest valid replacement.
            // The small window and candidate cap bound work and model input.
            for left in words[index.saturating_sub(6)..index].iter().rev() {
                for right in words[next..].iter().take(6) {
                    let start = literals
                        .iter()
                        .find(|literal| literal.start <= left.start && literal.end > left.start)
                        .map_or(left.start, |literal| literal.start);
                    let end_of_value = literals
                        .iter()
                        .find(|literal| literal.start < right.end && literal.end >= right.end)
                        .map_or(right.end, |literal| literal.end);
                    let range = start..end_of_value;
                    let before = &source[range.clone()];
                    let after = source[end..end_of_value]
                        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ':'));
                    if !super::correction::allowed(before, after, language) {
                        continue;
                    }
                    let (left_context, right_context) = context(source, &range);
                    let proposal = ProposedEdit {
                        kind: EditKind::SelfCorrection,
                        before: before.into(),
                        after: after.into(),
                        left_context: Some(left_context),
                        right_context: Some(right_context),
                    };
                    if validate(original, std::slice::from_ref(&proposal), language).is_err() {
                        continue;
                    }
                    if !choices.iter().any(|existing| {
                        existing.before == proposal.before
                            && existing.after == proposal.after
                            && existing.left_context == proposal.left_context
                            && existing.right_context == proposal.right_context
                    }) {
                        if choices.len() == MAX_CANDIDATES {
                            return Err(ValidationError::Limit);
                        }
                        choices.push(proposal);
                    }
                    break;
                }
            }
        }
    }
    if let (Some(first), Some(last)) = (words.first(), words.last()) {
        let first_text = &source[first.clone()];
        let first_character = first_text
            .chars()
            .next()
            .ok_or(ValidationError::InvalidEdit)?;
        let capital = first_character.is_lowercase()
            && super::punctuation::sentence_start(&source[..first.start]);
        let terminal = source[last.end..].trim().is_empty();
        let mut endpoints = Vec::new();
        if capital {
            let mut after = first_character.to_uppercase().collect::<String>()
                + &first_text[first_character.len_utf8()..];
            if first == last && terminal {
                after.push('.');
            }
            endpoints.push((first.clone(), after));
        }
        if terminal && !(capital && first == last) {
            endpoints.push((last.clone(), format!("{}.", &source[last.clone()])));
        }
        for (range, after) in endpoints {
            let (left_context, right_context) = context(source, &range);
            let proposal = ProposedEdit {
                kind: EditKind::PunctuationCase,
                before: source[range].into(),
                after,
                left_context: Some(left_context),
                right_context: Some(right_context),
            };
            if validate(original, std::slice::from_ref(&proposal), language).is_ok() {
                if choices.len() == MAX_CANDIDATES {
                    return Err(ValidationError::Limit);
                }
                choices.push(proposal);
            }
        }
    }
    Ok(choices)
}
