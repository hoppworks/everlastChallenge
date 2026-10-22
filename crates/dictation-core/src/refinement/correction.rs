use super::{Language, text};

pub(super) fn markers(language: Language) -> &'static [&'static str] {
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
    comma_after: bool,
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
                comma_after: tail.trim_start().starts_with(','),
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

// Compare an explicitly restated clause after removing only the polarity
// construction. English "do not" needs its support verb removed as a unit;
// unrelated verbs, names, quantities and grammatical words remain significant.
fn without_polarity<'a>(words: &[&'a str], language: Language) -> Vec<&'a str> {
    words
        .iter()
        .enumerate()
        .filter_map(|(index, &word)| {
            let negative = match language {
                Language::German => word.eq_ignore_ascii_case("nicht"),
                Language::English => word.eq_ignore_ascii_case("not"),
            };
            let support = language == Language::English
                && word.eq_ignore_ascii_case("do")
                && words
                    .get(index + 1)
                    .is_some_and(|next| next.eq_ignore_ascii_case("not"));
            (!negative && !support).then_some(word)
        })
        .collect()
}

// Recognize cardinal words only to protect a correction's surrounding text.
// This does not convert values or authorize changing any original number.
fn numeric_word(word: &str, language: Language) -> bool {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if word.len() > 128 {
        return false;
    }
    let lower = word.to_lowercase();
    match language {
        Language::German => german_cardinal(&lower),
        Language::English => matches!(
            lower.as_str(),
            "zero"
                | "one"
                | "two"
                | "three"
                | "four"
                | "five"
                | "six"
                | "seven"
                | "eight"
                | "nine"
                | "ten"
                | "eleven"
                | "twelve"
                | "thirteen"
                | "fourteen"
                | "fifteen"
                | "sixteen"
                | "seventeen"
                | "eighteen"
                | "nineteen"
                | "twenty"
                | "thirty"
                | "forty"
                | "fifty"
                | "sixty"
                | "seventy"
                | "eighty"
                | "ninety"
                | "hundred"
                | "thousand"
                | "million"
                | "billion"
                | "trillion"
        ),
    }
}

fn german_under_hundred(word: &str) -> bool {
    const UNITS: &[&str] = &[
        "ein", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
    ];
    const TENS: &[&str] = &[
        "zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig",
    ];
    UNITS.contains(&word)
        || TENS.contains(&word)
        || matches!(
            word,
            "null"
                | "zehn"
                | "elf"
                | "zwölf"
                | "dreizehn"
                | "vierzehn"
                | "fünfzehn"
                | "sechzehn"
                | "siebzehn"
                | "achtzehn"
                | "neunzehn"
        )
        || word.split_once("und").is_some_and(|(unit, tens)| {
            unit != "eins" && UNITS.contains(&unit) && TENS.contains(&tens)
        })
}

fn german_under_thousand(word: &str) -> bool {
    german_under_hundred(word)
        || word.split_once("hundert").is_some_and(|(hundreds, rest)| {
            (hundreds.is_empty()
                || matches!(
                    hundreds,
                    "ein"
                        | "zwei"
                        | "drei"
                        | "vier"
                        | "fünf"
                        | "sechs"
                        | "sieben"
                        | "acht"
                        | "neun"
                ))
                && (rest.is_empty() || german_under_hundred(rest))
        })
}

fn german_cardinal(word: &str) -> bool {
    german_under_thousand(word)
        || matches!(word, "million" | "millionen" | "milliarde" | "milliarden")
        || word.split_once("tausend").is_some_and(|(thousands, rest)| {
            (thousands.is_empty()
                || !matches!(thousands, "null" | "eins") && german_under_thousand(thousands))
                && (rest.is_empty() || german_under_thousand(rest))
        })
}

pub(super) fn allowed(before: &str, after: &str, language: Language) -> bool {
    let Some(Correction {
        old,
        replacement,
        weak_marker,
        comma,
        comma_after,
    }) = parts(before, language)
    else {
        return false;
    };
    if after != replacement {
        return false;
    }
    let old_words = text::words(old);
    let new_words = text::words(replacement);
    if old_words.is_empty() || old_words.len() > 6 {
        return false;
    }
    if weak_marker && !comparable(&old_words, &new_words, language, comma, comma_after) {
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
    let shared = old_words
        .iter()
        .zip(&new_words)
        .take_while(|(old, new)| old.eq_ignore_ascii_case(new))
        .count();
    if !weak_marker && shared >= 1 {
        let old_plain = without_polarity(&old_words, language);
        let new_plain = without_polarity(&new_words, language);
        let negative = |word: &&str| match language {
            Language::German => word.eq_ignore_ascii_case("nicht"),
            Language::English => word.eq_ignore_ascii_case("not"),
        };
        let polarity_changed = old_words.iter().any(negative) != new_words.iter().any(negative);
        if polarity_changed {
            // Only a complete explicit restatement may revise polarity. A
            // prefix ending at "not" cannot stand in for the full clause.
            return old_plain.len() == new_plain.len()
                && old_plain
                    .iter()
                    .zip(&new_plain)
                    .all(|(old, new)| old.eq_ignore_ascii_case(new));
        }
    }
    if old_words.len() > new_words.len() {
        return false;
    }
    // Preserve lexical context before an existing numeric value, even when
    // the subject itself starts with a quantity ("two tickets cost sixty").
    // Contiguous number words may still change representation ("twenty one"
    // to "21"); a later number beyond lexical context cannot erase that text.
    let numeric = |word: &str| numeric_word(word, language);
    if shared == 0
        && numeric(new_words[0])
        && old_words
            .iter()
            .skip_while(|word| numeric(word))
            .any(|word| numeric(word))
    {
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

fn comparable(
    old: &[&str],
    new: &[&str],
    language: Language,
    comma: bool,
    comma_after: bool,
) -> bool {
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
    // Written digits are unambiguous values. Spoken numbers need an explicit
    // separator after a weak marker: "no one" can instead begin a negative
    // clause ("two, no one else applied").
    let digits = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit());
    if digits(left) && digits(right)
        || comma_after && numeric_word(left, language) && numeric_word(right, language)
    {
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

/// A bounded context check for a restated phrase around a correction marker.
/// Repeated words on both sides must belong to the selected correction;
/// accepting only a prefix of the replacement would leave the old phrase behind.
/// This is a structural guard, not a general natural-language parser.
pub(super) fn context_aligned(
    source: &str,
    range: &std::ops::Range<usize>,
    before: &str,
    after: &str,
    language: Language,
) -> bool {
    let Some(correction) = parts(before, language) else {
        return false;
    };
    let old_end = range.start + correction.old.len();
    let replacement_start = range.end - after.len();
    let boundary = |text: &str, index: usize, c: char| {
        matches!(c, '?' | '!' | ';' | '\n' | '\r')
            || c == '.'
                && !(text[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_digit())
                    && text[index + 1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit()))
    };
    let left = &source[..old_end];
    let left_start = left
        .char_indices()
        .rev()
        .find(|(i, c)| boundary(left, *i, *c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let right = &source[replacement_start..];
    let right_end = right
        .char_indices()
        .find(|(i, c)| boundary(right, *i, *c))
        .map_or(right.len(), |(i, _)| i);
    let old_words = text::word_ranges(&source[left_start..old_end]);
    let new_words = text::word_ranges(&right[..right_end]);
    // A complete short value is not a restated clause. Its spelling can
    // legitimately repeat an unrelated quantity or person in the preceding
    // sentence ("5 boxes for 10, correction: 5 dollars", "Paul calls Maria,
    // sorry Paul"). Partial prefixes of a longer clause never take this path.
    let full_value = right[..right_end].trim_end();
    let value_words = text::words(full_value);
    let name = !value_words.is_empty()
        && value_words.len() <= 3
        && value_words.iter().all(|word| {
            word.chars().next().is_some_and(char::is_uppercase)
                && !frame(&word.to_lowercase(), language)
        });
    let scalar = value_words.len() == 1 || name;
    if scalar && range.end >= replacement_start + full_value.len() {
        return true;
    }
    for old in &old_words[old_words.len().saturating_sub(6)..] {
        for new in &new_words[..new_words.len().min(6)] {
            let old_word = &source[left_start + old.start..left_start + old.end];
            // Repeated quantities are values, not evidence of a repeated
            // grammatical frame ("5 boxes for 10, correction: 5 dollars").
            let same = !old_word.chars().all(|c| c.is_ascii_digit())
                && old_word.eq_ignore_ascii_case(&right[new.clone()]);
            if same
                && (range.start > left_start + old.start || range.end < replacement_start + new.end)
            {
                return false;
            }
        }
    }
    true
}
