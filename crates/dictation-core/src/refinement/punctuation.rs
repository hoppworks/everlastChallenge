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
