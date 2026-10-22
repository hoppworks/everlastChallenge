use std::ops::Range;

pub(super) fn word_character(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || matches!(c as u32, 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}

pub(super) fn words(text: &str) -> Vec<&str> {
    text.split(|c| !word_character(c))
        .filter(|word| !word.is_empty())
        .collect()
}

pub(super) fn whole_words(text: &str, range: &Range<usize>) -> bool {
    let selected = &text[range.clone()];
    !(selected.chars().next().is_some_and(word_character)
        && text[..range.start]
            .chars()
            .next_back()
            .is_some_and(word_character)
        || selected.chars().next_back().is_some_and(word_character)
            && text[range.end..].chars().next().is_some_and(word_character))
}

pub(super) fn filler(word: &str, language: super::Language) -> bool {
    match language {
        super::Language::German => {
            matches!(word.to_lowercase().as_str(), "äh" | "ähm" | "hm" | "öhm")
        }
        super::Language::English => matches!(word.to_lowercase().as_str(), "uh" | "um" | "hmm"),
    }
}

/// Protect quoted data, including an unclosed quote through the end of input.
/// Apostrophes inside words are not opening delimiters ("don't", "Anna's").
pub(super) fn quoted_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut open = None;
    let mut escaped = false;
    for (index, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if let Some((start, close)) = open {
            if c == close {
                ranges.push(start..index + c.len_utf8());
                open = None;
            }
            continue;
        }
        let close = match c {
            '"' | '`' => Some(c),
            '„' => Some('“'),
            '“' => Some('”'),
            '»' => Some('«'),
            '«' => Some('»'),
            '‚' => Some('‘'),
            '‘' => Some('’'),
            '\'' if !text[..index]
                .chars()
                .next_back()
                .is_some_and(word_character) =>
            {
                Some('\'')
            }
            _ => None,
        };
        if let Some(close) = close {
            open = Some((index, close));
        }
    }
    if let Some((start, _)) = open {
        ranges.push(start..text.len());
    }
    ranges
}

pub(super) fn word_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if word_character(c) {
            start.get_or_insert(index);
        } else if let Some(start) = start.take() {
            ranges.push(start..index);
        }
    }
    ranges
}
