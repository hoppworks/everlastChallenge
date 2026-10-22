//! Finite word alternatives for a single text generation; not acceptance proof.
use dictation_core::refinement::{OriginalTranscript, ProposedEdit};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};
use text_refinement_protocol::FailureKind;

const MAX_VARIANTS: usize = 128;
const MAX_SEARCH: usize = 4096;
const MAX_PATTERN: usize = 512 * 1024;

fn word_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || matches!(c as u32,
        0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}
fn ranges(text: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if word_char(c) {
            start.get_or_insert(i);
        } else if let Some(start) = start.take() {
            result.push(start..i);
        }
    }
    result
}
fn words(text: &str) -> Vec<String> {
    ranges(text)
        .iter()
        .map(|range| text[range.clone()].to_owned())
        .collect()
}
fn span(source: &str, edit: &ProposedEdit) -> Result<Range<usize>, FailureKind> {
    let mut found = None;
    for (start, _) in source.char_indices() {
        if source[start..].starts_with(&edit.before) {
            let end = start + edit.before.len();
            if edit
                .left_context
                .as_ref()
                .is_none_or(|x| source[..start].ends_with(x))
                && edit
                    .right_context
                    .as_ref()
                    .is_none_or(|x| source[end..].starts_with(x))
            {
                if found.is_some() {
                    return Err(FailureKind::InvalidCandidate);
                }
                found = Some(start..end);
            }
        }
    }
    found.ok_or(FailureKind::InvalidCandidate)
}
fn word_pattern(word: &str, gap: &str, sentence_gap: Option<&str>, initial: bool) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let title = first.to_uppercase().collect::<String>() + chars.as_str();
    if first.is_lowercase() && !chars.as_str().chars().any(char::is_uppercase) && word != title {
        if initial {
            format!("(?:{gap})?(?:{word}|{title})")
        } else if let Some(sentence_gap) = sentence_gap {
            format!("(?:{gap}{word}|{sentence_gap}{title})")
        } else {
            format!("{gap}{word}")
        }
    } else {
        if initial {
            format!("(?:{gap})?{word}")
        } else {
            format!("{gap}{word}")
        }
    }
}
struct Part {
    normal: String,
    initial: String,
    nullable: bool,
}
impl Part {
    fn word(word: &str, gap: &str, sentence_gap: Option<&str>) -> Self {
        Self {
            normal: word_pattern(word, gap, sentence_gap, false),
            initial: word_pattern(word, gap, sentence_gap, true),
            nullable: false,
        }
    }
    fn choices(
        variants: &BTreeSet<Vec<String>>,
        gap: &str,
        sentence_gap: Option<&str>,
        prefix_gap: Option<&str>,
        fixed_pairs: &BTreeMap<(String, String), String>,
    ) -> Self {
        let render = |words: &[String], initial: bool| {
            words
                .iter()
                .enumerate()
                .map(|(index, word)| {
                    let fixed = if index == 0 {
                        prefix_gap
                    } else {
                        fixed_pairs
                            .get(&(words[index - 1].clone(), word.clone()))
                            .map(String::as_str)
                            .or_else(|| {
                                words[index - 1]
                                    .chars()
                                    .all(char::is_numeric)
                                    .then_some(" ")
                            })
                    };
                    word_pattern(
                        word,
                        fixed.unwrap_or(gap),
                        if fixed.is_some() { None } else { sentence_gap },
                        initial && index == 0,
                    )
                })
                .collect::<String>()
        };
        let normal = variants
            .iter()
            .map(|words| render(words, false))
            .collect::<Vec<_>>();
        let initial = variants
            .iter()
            .filter(|words| !words.is_empty())
            .map(|words| render(words, true))
            .collect::<Vec<_>>();
        Self {
            normal: format!("(?:{})", normal.join("|")),
            initial: format!("(?:{})", initial.join("|")),
            nullable: variants.contains(&Vec::new()),
        }
    }
}

fn literal_atom(atom: &str) -> bool {
    atom.chars()
        .any(|c| c.is_numeric() || "@/\\=_<>`#+-%\"„“”‘’".contains(c))
}
fn escape_literal(value: &str) -> String {
    let mut result = String::new();
    for c in value.chars() {
        if ".+*?^$()[]{}|\\".contains(c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}
fn append_plain(
    parts: &mut Vec<Part>,
    words: &[String],
    range: Range<usize>,
    locks: &BTreeMap<usize, (usize, String)>,
    gap: &str,
    fixed_gaps: &BTreeMap<usize, String>,
    sentence_gap: &str,
) {
    let mut cursor = range.start;
    while cursor < range.end {
        let fixed_gap = fixed_gaps.get(&cursor);
        let sentence_gap = fixed_gap.is_none().then_some(sentence_gap);
        let gap = fixed_gap.map_or(gap, String::as_str);
        if let Some((next, atom)) = locks.get(&cursor).filter(|(next, _)| *next <= range.end) {
            let exact = format!("{gap}{}", escape_literal(atom));
            parts.push(Part {
                normal: exact.clone(),
                initial: format!("(?:{gap})?{}", escape_literal(atom)),
                nullable: false,
            });
            cursor = *next;
        } else {
            parts.push(Part::word(&words[cursor], gap, sentence_gap));
            cursor += 1;
        }
    }
}

struct Alternative {
    start: usize,
    end: usize,
    after: Vec<String>,
}

fn variants(
    start: usize,
    end: usize,
    words: &[String],
    alternatives: &[Alternative],
) -> Result<BTreeSet<Vec<String>>, FailureKind> {
    let mut pending = vec![(start, Vec::new())];
    let mut results = BTreeSet::new();
    let mut visited = 0;
    while let Some((position, prefix)) = pending.pop() {
        visited += 1;
        if visited > MAX_SEARCH || pending.len() > MAX_SEARCH {
            return Err(FailureKind::InputLimit);
        }
        if position == end {
            results.insert(prefix);
            if results.len() > MAX_VARIANTS {
                return Err(FailureKind::InputLimit);
            }
            continue;
        }
        let mut unchanged = prefix.clone();
        unchanged.push(words[position].clone());
        pending.push((position + 1, unchanged));
        for alternative in alternatives
            .iter()
            .filter(|a| a.start == position && a.end <= end)
        {
            let mut changed = prefix.clone();
            changed.extend(alternative.after.iter().cloned());
            pending.push((alternative.end, changed));
        }
    }
    Ok(results)
}

pub fn pattern(
    original: &OriginalTranscript,
    edits: &[ProposedEdit],
) -> Result<String, FailureKind> {
    let source = original.as_str();
    let source_ranges = ranges(source);
    let source_words = words(source);
    if source_words.len() > 8192 {
        return Err(FailureKind::InputLimit);
    }
    let mut alternatives = Vec::new();
    for edit in edits {
        let span = span(source, edit)?;
        let start = source_ranges.partition_point(|word| word.end <= span.start);
        let end = source_ranges.partition_point(|word| word.start < span.end);
        if start >= end {
            return Err(FailureKind::InvalidCandidate);
        }
        alternatives.push(Alternative {
            start,
            end,
            after: words(&edit.after),
        });
    }
    alternatives.sort_by_key(|a| (a.start, a.end));
    // Lock unchanged technical/quoted atoms as whole literal fragments. Edits
    // that actually cover an atom retain their explicitly authorized alternatives.
    let mut locks = BTreeMap::new();
    let mut atom_start = None;
    for (offset, c) in source
        .char_indices()
        .chain(std::iter::once((source.len(), ' ')))
    {
        if !c.is_whitespace() {
            atom_start.get_or_insert(offset);
        } else if let Some(start) = atom_start.take() {
            let atom = &source[start..offset];
            let first = source_ranges.partition_point(|word| word.end <= start);
            let last = source_ranges.partition_point(|word| word.start < offset);
            if first < last
                && literal_atom(atom)
                && !alternatives.iter().any(|a| a.start < last && a.end > first)
            {
                locks.insert(first, (last, atom.to_owned()));
            }
        }
    }
    let mut gaps: BTreeSet<String> = [" ", ", ", ". ", "! ", "? ", ": ", "; ", ".", "!", "?"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut previous = 0;
    for range in &source_ranges {
        if previous < range.start {
            let source_gap = &source[previous..range.start];
            if !source_gap.contains(['\n', '\r']) {
                gaps.insert(source_gap.to_owned());
            }
        }
        previous = range.end;
    }
    if previous < source.len() {
        let source_gap = &source[previous..];
        if !source_gap.contains(['\n', '\r']) {
            gaps.insert(source_gap.to_owned());
        }
    }
    // Case changes must use the same permitted separators as unchanged words.
    // In particular, a titlecase alternative must not bypass a numeric gap lock.
    let sentence_gap = format!(
        "(?:{})",
        gaps.iter()
            .filter(|gap| {
                gap.contains('\n')
                    || gap
                        .strip_prefix(['.', '!', '?'])
                        .is_some_and(|rest| rest.chars().all(char::is_whitespace))
            })
            .map(|gap| escape_literal(gap))
            .collect::<Vec<_>>()
            .join("|")
    );
    let gap = format!(
        "(?:{})",
        gaps.iter()
            .map(|gap| escape_literal(gap))
            .collect::<Vec<_>>()
            .join("|")
    );
    if gap.len() > 1024
        || gap
            .len()
            .saturating_mul(source_words.len())
            .saturating_mul(4)
            > MAX_PATTERN
    {
        return Err(FailureKind::InputLimit);
    }
    // A bare numeric value followed by a unit/name must retain its source
    // separator. Inserting a period here changes the value into an ordinal.
    let mut fixed_gaps = BTreeMap::new();
    let mut fixed_separator_sets: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for (index, pair) in source_ranges.windows(2).enumerate() {
        let previous = &source[pair[0].clone()];
        let separator = &source[pair[0].end..pair[1].start];
        if !separator.is_empty()
            && (previous.chars().all(char::is_numeric) || separator.contains(['\n', '\r']))
        {
            // A locked atom may already own trailing punctuation (e.g. 0017;).
            // The next word consumes only the remaining separator.
            let owned_suffix = locks
                .values()
                .find(|(end, _)| *end == index + 1)
                .and_then(|(_, atom)| ranges(atom).last().map(|last| &atom[last.end..]));
            let remaining = owned_suffix
                .and_then(|suffix| separator.strip_prefix(suffix))
                .unwrap_or(separator);
            fixed_gaps.insert(index + 1, escape_literal(remaining));
            fixed_separator_sets
                .entry((previous.to_owned(), source[pair[1].clone()].to_owned()))
                .or_default()
                .insert(escape_literal(separator));
        }
    }
    let fixed_pairs = fixed_separator_sets
        .into_iter()
        .map(|(key, values)| {
            (
                key,
                format!("(?:{})", values.into_iter().collect::<Vec<_>>().join("|")),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut parts = Vec::new();
    let mut cursor = 0;
    let mut index = 0;
    while index < alternatives.len() {
        let start = alternatives[index].start;
        append_plain(
            &mut parts,
            &source_words,
            cursor..start,
            &locks,
            &gap,
            &fixed_gaps,
            &sentence_gap,
        );
        let mut end = alternatives[index].end;
        let mut next = index + 1;
        while next < alternatives.len() && alternatives[next].start < end {
            end = end.max(alternatives[next].end);
            next += 1;
        }
        let variants = variants(start, end, &source_words, &alternatives[index..next])?;
        parts.push(Part::choices(
            &variants,
            &gap,
            Some(&sentence_gap),
            fixed_gaps.get(&start).map(String::as_str),
            &fixed_pairs,
        ));
        cursor = end;
        index = next;
    }
    append_plain(
        &mut parts,
        &source_words,
        cursor..source_words.len(),
        &locks,
        &gap,
        &fixed_gaps,
        &sentence_gap,
    );
    let mut branches = Vec::new();
    let mut total = 0usize;
    let mut all_nullable = true;
    for (index, part) in parts.iter().enumerate() {
        let length = part.initial.len()
            + parts[index + 1..]
                .iter()
                .map(|part| part.normal.len())
                .sum::<usize>();
        total = total.saturating_add(length);
        if total > MAX_PATTERN {
            return Err(FailureKind::InputLimit);
        }
        let mut branch = part.initial.clone();
        for remaining in &parts[index + 1..] {
            branch.push_str(&remaining.normal);
        }
        branches.push(branch);
        if !part.nullable {
            all_nullable = false;
            break;
        }
    }
    if all_nullable {
        branches.push(String::new());
    }
    let source_prefix = &source[..source_ranges
        .first()
        .map_or(source.len(), |word| word.start)];
    let prefix = if source_prefix.contains(['\n', '\r']) {
        let owned = locks
            .get(&0)
            .and_then(|(_, atom)| ranges(atom).first().map(|first| &atom[..first.start]));
        escape_literal(
            owned
                .and_then(|part| source_prefix.strip_suffix(part))
                .unwrap_or(source_prefix),
        )
    } else {
        String::new()
    };
    let source_suffix = &source[source_ranges.last().map_or(source.len(), |word| word.end)..];
    let suffix = if source_suffix.contains(['\n', '\r']) {
        let owned = locks
            .values()
            .find(|(end, _)| *end == source_words.len())
            .and_then(|(_, atom)| ranges(atom).last().map(|last| &atom[last.end..]));
        escape_literal(
            owned
                .and_then(|part| source_suffix.strip_prefix(part))
                .unwrap_or(source_suffix),
        )
    } else {
        format!("(?:{gap})?")
    };
    let pattern = format!("^{prefix}(?:{}){suffix}$", branches.join("|"));
    if pattern.len() > MAX_PATTERN {
        return Err(FailureKind::InputLimit);
    }
    Ok(pattern)
}
