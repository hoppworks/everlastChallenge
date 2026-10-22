//! Match final words through a bounded graph of source-authorized edits.
//! Graph matching discovers proposals; the existing validator grants authority.
use super::{
    EditKind, Language, OriginalTranscript, ProposedEdit, StagedEdits, ValidationError,
    proposal_candidates, propose_punctuation, text, validate, validate_staged,
};
use std::ops::Range;

const MAX_STATES: usize = 16_384;
const MAX_TRANSITIONS: usize = 65_536;

struct Edge {
    end: usize,
    replacement: Vec<String>,
    choice: usize,
}
struct State {
    source: usize,
    output: usize,
    previous: Option<(usize, Option<usize>)>,
}

pub(super) fn locate(source: &str, edit: &ProposedEdit) -> Result<Range<usize>, ValidationError> {
    let mut found = None;
    for (start, _) in source.char_indices() {
        if !source[start..].starts_with(&edit.before) {
            continue;
        }
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
    }
    found.ok_or(ValidationError::InvalidEdit)
}

fn words(value: &str) -> Vec<String> {
    text::words(value)
        .into_iter()
        .map(str::to_lowercase)
        .collect()
}

fn add_state(
    states: &mut Vec<State>,
    pending: &mut Vec<usize>,
    source: usize,
    output: usize,
    previous: (usize, Option<usize>),
) -> Result<(), ValidationError> {
    if states.len() == MAX_STATES {
        return Err(ValidationError::Limit);
    }
    pending.push(states.len());
    states.push(State {
        source,
        output,
        previous: Some(previous),
    });
    Ok(())
}

fn validate_path(
    original: &OriginalTranscript,
    output: &str,
    language: Language,
    states: &[State],
    choices: &[ProposedEdit],
    final_state: usize,
) -> Result<StagedEdits, ValidationError> {
    let mut structural = Vec::new();
    let mut current = final_state;
    while let Some((previous, choice)) = states[current].previous {
        if let Some(choice) = choice {
            structural.push(choices[choice].clone());
        }
        current = previous;
    }
    structural.reverse();
    let intermediate = validate(original, &structural, language)?;
    let intermediate = OriginalTranscript::new(intermediate.as_str().into())?;
    let punctuation = propose_punctuation(&intermediate, output, language)?;
    let proposal = StagedEdits {
        structural,
        punctuation,
    };
    if validate_staged(original, &proposal, language)?.as_str() != output {
        return Err(ValidationError::InvalidEdit);
    }
    Ok(proposal)
}

/// Derive edits from one complete model result without trusting model-provided
/// offsets or allowing free rewrites. Matching ignores case and punctuation
/// only for discovering a path; exact case, literals, whitespace, protected
/// spans and the combined edit limit are checked by the validators afterward.
pub fn reconcile(
    original: &OriginalTranscript,
    output: &str,
    language: Language,
) -> Result<StagedEdits, ValidationError> {
    if output.len() > super::MAX_ORIGINAL_BYTES {
        return Err(ValidationError::Limit);
    }
    let source = original.as_str();
    let source_ranges = text::word_ranges(source);
    let source_words = words(source);
    let output_words = words(output);
    let choices: Vec<_> = proposal_candidates(original, language)?
        .into_iter()
        .filter(|e| e.kind != EditKind::PunctuationCase)
        .collect();
    let mut edges: Vec<Vec<Edge>> = (0..=source_words.len()).map(|_| Vec::new()).collect();
    for (choice, edit) in choices.iter().enumerate() {
        let span = locate(source, edit)?;
        let start = source_ranges.partition_point(|word| word.end <= span.start);
        let end = source_ranges.partition_point(|word| word.start < span.end);
        if start >= end {
            return Err(ValidationError::InvalidEdit);
        }
        edges[start].push(Edge {
            end,
            replacement: words(&edit.after),
            choice,
        });
    }
    let mut states = vec![State {
        source: 0,
        output: 0,
        previous: None,
    }];
    // Identical word positions can have different punctuation provenance.
    // Preserve each path until exact validation; depth-first traversal reaches
    // complete candidates without expanding all independent alternatives first.
    let mut pending = vec![0];
    let mut transitions = 0usize;
    while let Some(cursor) = pending.pop() {
        let state = &states[cursor];
        let (i, j) = (state.source, state.output);
        if i == source_words.len() && j == output_words.len() {
            if let Ok(proposal) =
                validate_path(original, output, language, &states, &choices, cursor)
            {
                return Ok(proposal);
            }
            continue;
        }
        for edge in edges[i].iter().rev() {
            transitions += 1;
            if transitions > MAX_TRANSITIONS {
                return Err(ValidationError::Limit);
            }
            let end = j
                .checked_add(edge.replacement.len())
                .ok_or(ValidationError::Limit)?;
            if output_words.get(j..end) == Some(edge.replacement.as_slice()) {
                add_state(
                    &mut states,
                    &mut pending,
                    edge.end,
                    end,
                    (cursor, Some(edge.choice)),
                )?;
            }
        }
        if i < source_words.len() && j < output_words.len() && source_words[i] == output_words[j] {
            add_state(&mut states, &mut pending, i + 1, j + 1, (cursor, None))?;
        }
    }
    Err(ValidationError::InvalidEdit)
}
