use super::text;

/// Return whether acceptance additionally requires a sentence-start position.
/// Internal/name/identifier case and protected literal punctuation cannot change.
pub(super) fn analyze(before: &str, after: &str) -> Option<bool> {
    if before.chars().chain(after.chars()).any(|c| {
        !text::word_character(c)
            && !matches!(
                c,
                ' ' | '\t' | '\n' | '\r' | '.' | ',' | '!' | '?' | ':' | ';'
            )
    }) {
        return None;
    }
    let old_words = text::words(before);
    let new_words = text::words(after);
    if old_words.is_empty() || old_words.len() != new_words.len() {
        return None;
    }
    let mut needs_sentence_start = false;
    for (index, (old, new)) in old_words.iter().zip(new_words.iter()).enumerate() {
        if old == new {
            continue;
        }
        let first = old.chars().next()?;
        let title = first.to_uppercase().collect::<String>() + &old[first.len_utf8()..];
        if index != 0 || !first.is_lowercase() || &title != new {
            return None;
        }
        needs_sentence_start = true;
    }
    Some(needs_sentence_start)
}

/// Source positions of unchanged punctuation gaps. Word equality/case has
/// already been checked by `analyze`. A changed gap has no source authority.
pub(super) fn source_positions(before: &str, after: &str, offset: usize) -> Vec<Option<usize>> {
    let mut positions = vec![None; after.len()];
    let old_words = text::word_ranges(before);
    let new_words = text::word_ranges(after);
    let (mut old_end, mut new_end) = (0, 0);
    for (old, new) in old_words
        .into_iter()
        .chain(std::iter::once(before.len()..before.len()))
        .zip(
            new_words
                .into_iter()
                .chain(std::iter::once(after.len()..after.len())),
        )
    {
        let old_gap = &before[old_end..old.start];
        let new_gap = &after[new_end..new.start];
        if old_gap
            .chars()
            .filter(|c| !c.is_whitespace())
            .eq(new_gap.chars().filter(|c| !c.is_whitespace()))
        {
            for ((old_index, _), (new_index, _)) in old_gap
                .char_indices()
                .filter(|(_, c)| !c.is_whitespace())
                .zip(new_gap.char_indices().filter(|(_, c)| !c.is_whitespace()))
            {
                positions[new_end + new_index] = Some(offset + old_end + old_index);
            }
        }
        old_end = old.end;
        new_end = new.end;
    }
    positions
}

/// Observe the final composition, including later deletions. Repeated separators
/// are allowed only when both characters already neighbored in the source.
pub(super) struct Composition<'a> {
    source: &'a str,
    previous: Option<(char, Option<usize>)>,
}

impl<'a> Composition<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self {
            source,
            previous: None,
        }
    }

    pub(super) fn append(
        &mut self,
        value: &str,
        position: impl Fn(usize) -> Option<usize>,
    ) -> Result<(), super::ValidationError> {
        for (index, c) in value.char_indices() {
            if c.is_whitespace() {
                continue;
            }
            let origin = position(index);
            if let Some((previous, previous_origin)) = self.previous
                && previous == c
                && matches!(c, '.' | ',' | ':' | ';')
                && !previous_origin.zip(origin).is_some_and(|(left, right)| {
                    self.source
                        .get(left + 1..right)
                        .is_some_and(|gap| gap.chars().all(char::is_whitespace))
                })
            {
                return Err(super::ValidationError::InvalidEdit);
            }
            self.previous = Some((c, origin));
        }
        Ok(())
    }
}

pub(super) fn sentence_start(prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches([' ', '\t']);
    prefix.is_empty() || prefix.ends_with(['.', '!', '?', '\n', '\r'])
}

pub(super) fn propose(
    original: &super::OriginalTranscript,
    candidate: &str,
    language: super::Language,
) -> Result<Vec<super::ProposedEdit>, super::ValidationError> {
    use super::{EditKind, MAX_ORIGINAL_BYTES, ProposedEdit, ValidationError};
    let source = original.as_str();
    if candidate.len() > MAX_ORIGINAL_BYTES {
        return Err(ValidationError::Limit);
    }
    if source == candidate {
        return Ok(Vec::new());
    }
    let old = text::word_ranges(source);
    let new = text::word_ranges(candidate);
    if old.is_empty() || old.len() != new.len() {
        return Err(ValidationError::InvalidEdit);
    }
    let mut edits = Vec::new();
    for (index, (before_word, after_word)) in old.iter().zip(&new).enumerate() {
        if source[before_word.clone()].to_lowercase()
            != candidate[after_word.clone()].to_lowercase()
        {
            return Err(ValidationError::InvalidEdit);
        }
        let start = if index == 0 { 0 } else { before_word.start };
        let after_start = if index == 0 { 0 } else { after_word.start };
        let end = old.get(index + 1).map_or(source.len(), |word| word.start);
        let after_end = new
            .get(index + 1)
            .map_or(candidate.len(), |word| word.start);
        let before = &source[start..end];
        let after = &candidate[after_start..after_end];
        if before == after {
            continue;
        }
        // Keep unchanged punctuation outside the edit. In particular an
        // adjacent opening quote or dash does not belong to a case change.
        // Only trim the gaps: every edited word retains its whole-word anchor.
        let prefix = source[start..before_word.start]
            .chars()
            .zip(candidate[after_start..after_word.start].chars())
            .take_while(|(left, right)| left == right)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let suffix = source[before_word.end..end]
            .chars()
            .rev()
            .zip(candidate[after_word.end..after_end].chars().rev())
            .take_while(|(left, right)| left == right)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let start = start + prefix;
        let end = end - suffix;
        let before = &source[start..end];
        let after = &candidate[after_start + prefix..after_end - suffix];
        if edits.len() == super::validator::MAX_EDITS {
            return Err(ValidationError::Limit);
        }
        let (left_context, right_context) = super::candidates::context(source, &(start..end));
        edits.push(ProposedEdit {
            kind: EditKind::PunctuationCase,
            before: before.into(),
            after: after.into(),
            left_context: Some(left_context),
            right_context: Some(right_context),
        });
    }
    let accepted = super::validate(original, &edits, language)?;
    if accepted.as_str() != candidate {
        return Err(ValidationError::InvalidEdit);
    }
    Ok(edits)
}
