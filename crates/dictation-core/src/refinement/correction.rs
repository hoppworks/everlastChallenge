use super::{Language, text};

fn markers(language: Language) -> &'static [&'static str] {
    match language {
        Language::German => &["nein", "entschuldigung", "ich meine", "korrektur"],
        Language::English => &["no", "sorry", "i mean", "correction"],
    }
}

/// Locate an explicit, word-bounded correction marker. The replacement is
/// always an exact suffix already present in the original, never generated prose.
struct Correction<'a> {
    old: &'a str,
    replacement: &'a str,
    weak_marker: bool,
    comma: bool,
}

fn parts(before: &str, language: Language) -> Option<Correction<'_>> {
    let mut found = None;
    for range in text::word_ranges(before) {
        let index = range.start;
        if index > 0 && !before[..index].ends_with(|c: char| c.is_whitespace() || c == ',') {
            continue;
        }
        let rest = &before[index..];
        for marker in markers(language) {
            let Some(prefix) = rest.get(..marker.len()) else {
                continue;
            };
            if !prefix.eq_ignore_ascii_case(marker) {
                continue;
            }
            let tail = &rest[marker.len()..];
            if !tail.starts_with(|c: char| c.is_whitespace() || matches!(c, ',' | ':')) {
                continue;
            }
            let replacement =
                tail.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ':'));
            let old = before[..index].trim_end_matches(|c: char| c.is_whitespace() || c == ',');
            if old.is_empty() {
                continue;
            }
            if replacement.is_empty() || found.is_some() {
                return None;
            }
            found = Some(Correction {
                old,
                replacement,
                weak_marker: matches!(*marker, "no" | "nein"),
                comma: before[..index].trim_end().ends_with(','),
            });
        }
    }
    found
}

fn frame(word: &str, language: Language) -> bool {
    match language {
        Language::German => matches!(
            word,
            "der"
                | "die"
                | "das"
                | "den"
                | "dem"
                | "des"
                | "ein"
                | "eine"
                | "einen"
                | "ist"
                | "sind"
                | "war"
                | "waren"
                | "hat"
                | "haben"
                | "wird"
                | "werden"
                | "ich"
                | "du"
                | "er"
                | "sie"
                | "es"
                | "wir"
                | "ihr"
                | "am"
                | "um"
                | "im"
                | "in"
                | "mit"
                | "von"
                | "zu"
                | "für"
                | "an"
                | "auf"
                | "nicht"
                | "nein"
                | "kein"
                | "keine"
                | "keinen"
                | "nur"
                | "wenn"
                | "falls"
                | "bitte"
                | "nie"
                | "niemals"
        ),
        Language::English => matches!(
            word,
            "the"
                | "a"
                | "an"
                | "is"
                | "are"
                | "was"
                | "were"
                | "has"
                | "have"
                | "will"
                | "i"
                | "you"
                | "he"
                | "she"
                | "it"
                | "we"
                | "they"
                | "at"
                | "on"
                | "in"
                | "with"
                | "from"
                | "to"
                | "for"
                | "not"
                | "no"
                | "never"
                | "only"
                | "if"
                | "unless"
                | "please"
        ),
    }
}

pub(super) fn allowed(before: &str, after: &str, language: Language) -> bool {
    let Some(Correction {
        old,
        replacement,
        weak_marker,
        comma,
    }) = parts(before, language)
    else {
        return false;
    };
    if after != replacement {
        return false;
    }
    let old_words = text::words(old);
    let new_words = text::words(replacement);
    if old_words.is_empty() || old_words.len() > 6 || old_words.len() > new_words.len() {
        return false;
    }
    if weak_marker && !comparable(&old_words, &new_words, language, comma) {
        return false;
    }
    // A correction is local, not permission to discard a previous sentence.
    if old.char_indices().any(|(index, c)| {
        matches!(c, '?' | '!' | ';' | '\n' | '\r')
            || c == '.'
                && !(old[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_digit())
                    && old[index + 1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit()))
    }) {
        return false;
    }
    let old_frame = old_words
        .iter()
        .map(|w| w.to_lowercase())
        .filter(|w| frame(w, language))
        .collect::<Vec<_>>();
    let new_frame = new_words
        .iter()
        .map(|w| w.to_lowercase())
        .filter(|w| frame(w, language))
        .collect::<Vec<_>>();
    // Keep the surrounding grammatical frame and negation. Additional trailing
    // context (e.g. "um 9 Uhr") may already be part of the corrected source span.
    new_frame.starts_with(&old_frame)
}

fn comparable(old: &[&str], new: &[&str], language: Language, comma: bool) -> bool {
    let shared = old
        .iter()
        .zip(new.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let Some(left) = old.get(shared) else {
        return false;
    };
    let Some(right) = new.get(shared) else {
        return false;
    };
    if left.chars().all(|c| c.is_ascii_digit()) && right.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let days = match language {
        Language::German => &[
            "montag",
            "dienstag",
            "mittwoch",
            "donnerstag",
            "freitag",
            "samstag",
            "sonntag",
        ][..],
        Language::English => &[
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ][..],
    };
    if days.contains(&left.to_lowercase().as_str()) && days.contains(&right.to_lowercase().as_str())
    {
        return true;
    }
    // A bare "no" is commonly ordinary negation. Apart from numeric/day values,
    // require a comma and parallel short proper-name phrases, not arbitrary text.
    let proper_name = |words: &[&str]| {
        words.len() <= 2
            && words.iter().all(|word| {
                let mut chars = word.chars();
                chars.next().is_some_and(char::is_uppercase)
                    && chars.clone().next().is_some()
                    && chars.all(|c| c.is_lowercase() || c == '-')
            })
    };
    comma && proper_name(&old[shared..]) && proper_name(&new[shared..])
}
