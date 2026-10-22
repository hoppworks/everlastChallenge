use super::{Language, OriginalTranscript, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    Empty,
    ExactValue,
    NoCorrectionSignal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefinementSignal {
    Filler,
    ExplicitCorrection,
    Repetition,
    Punctuation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessingDecision {
    Skip(SkipReason),
    Refine(Vec<RefinementSignal>),
}

/// A routing heuristic, never a guarantee of grammatical correctness.
pub fn decide(original: &OriginalTranscript, language: Language) -> ProcessingDecision {
    let source = original.as_str().trim();
    if source.is_empty() {
        return ProcessingDecision::Skip(SkipReason::Empty);
    }
    if exact_value(source) {
        return ProcessingDecision::Skip(SkipReason::ExactValue);
    }
    let mut protected = text::quoted_ranges(source);
    protected.extend(super::literals::ranges(source));
    let ranges = text::word_ranges(source)
        .into_iter()
        .filter(|word| {
            !protected
                .iter()
                .any(|quote| word.start < quote.end && word.end > quote.start)
        })
        .collect::<Vec<_>>();
    let words = ranges
        .iter()
        .map(|range| source[range.clone()].to_lowercase())
        .collect::<Vec<_>>();
    let mut signals = Vec::with_capacity(4);
    if words.iter().any(|word| text::filler(word, language)) {
        signals.push(RefinementSignal::Filler);
    }
    let correction = words.iter().enumerate().any(|(index, word)| {
        ranges[index].start > 0
            && match language {
                Language::German => {
                    matches!(word.as_str(), "nein" | "entschuldigung" | "korrektur")
                        || word == "ich" && words.get(index + 1).is_some_and(|w| w == "meine")
                }
                Language::English => {
                    matches!(word.as_str(), "no" | "sorry" | "correction")
                        || word == "i" && words.get(index + 1).is_some_and(|w| w == "mean")
                }
            }
    });
    if correction {
        signals.push(RefinementSignal::ExplicitCorrection);
    }
    if words
        .windows(2)
        .any(|pair| pair[0] == pair[1] && pair[0].chars().count() > 1)
    {
        signals.push(RefinementSignal::Repetition);
    }
    let first_atom = source.split_whitespace().next().unwrap_or_default();
    let lower_start = ranges.first().is_some_and(|range| {
        range.start == 0
            && source[range.clone()].chars().all(char::is_lowercase)
            && !super::literals::protected_atom(first_atom)
    });
    let ends_sentence = source
        .trim_end_matches(['"', '“', '”', '»', '«', '\''])
        .ends_with(['.', '?', '!']);
    if words.len() >= 4 && !ends_sentence || words.len() >= 2 && lower_start {
        signals.push(RefinementSignal::Punctuation);
    }
    if signals.is_empty() {
        ProcessingDecision::Skip(SkipReason::NoCorrectionSignal)
    } else {
        ProcessingDecision::Refine(signals)
    }
}

fn exact_value(source: &str) -> bool {
    let numeric = source
        .strip_suffix(" EUR")
        .or_else(|| source.strip_suffix(" Euro"))
        .or_else(|| source.strip_suffix(" USD"))
        .unwrap_or(source)
        .trim();
    if numeric.chars().any(|c| c.is_ascii_digit())
        && numeric.chars().all(|c| {
            c.is_ascii_digit()
                || matches!(c, ' ' | '+' | '-' | '.' | ',' | ':' | '/' | '%' | '€' | '$')
        })
    {
        return true;
    }
    if source.chars().any(char::is_whitespace) {
        return false;
    }
    if let Some(rest) = source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
    {
        return !rest.is_empty()
            && !rest.starts_with(['/', '?', '#'])
            && !rest.chars().any(char::is_control);
    }
    if let Some((local, domain)) = source.split_once('@') {
        return !local.is_empty()
            && domain.contains('.')
            && !domain.contains('@')
            && !domain.starts_with('.');
    }
    source.contains(['_', '=', '<', '>', '/', '\\']) && source.chars().any(text::word_character)
}
