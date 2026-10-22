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

pub(super) fn sentence_start(prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches([' ', '\t']);
    prefix.is_empty() || prefix.ends_with(['.', '!', '?', '\n', '\r'])
}
