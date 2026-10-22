//! Conservative zero-model completion using the same source-bound validators.
use super::{
    AcceptedTranscript, EditKind, Language, OriginalTranscript, ProcessingDecision,
    RefinementSignal, ValidationError, correction, decide, postprocess, proposal_candidates,
    reconcile, validate, validate_staged,
};

/// Return a complete validated result only when all remaining work is
/// deterministic. `None` requires the caller to send the untouched original to
/// the model. This does not grant general semantic authority to edit candidates.
pub fn try_direct(
    original: &OriginalTranscript,
    language: Language,
) -> Result<Option<AcceptedTranscript>, ValidationError> {
    match decide(original, language) {
        ProcessingDecision::Skip(_) => return validate(original, &[], language).map(Some),
        ProcessingDecision::Refine(signals)
            if signals.iter().any(|signal| {
                !matches!(
                    signal,
                    RefinementSignal::Filler | RefinementSignal::ExplicitCorrection
                )
            }) =>
        {
            return Ok(None);
        }
        ProcessingDecision::Refine(_) => {}
    }
    let mut choices = proposal_candidates(original, language)?
        .into_iter()
        .filter(|edit| edit.kind == EditKind::SelfCorrection)
        .map(|edit| super::reconcile::locate(original.as_str(), &edit).map(|range| (range, edit)))
        .collect::<Result<Vec<_>, _>>()?;
    choices.sort_by_key(|(range, _)| range.start);
    let mut selected = Vec::new();
    let mut cursor = 0;
    while cursor < choices.len() {
        let mut end = choices[cursor].0.end;
        let mut next = cursor + 1;
        while next < choices.len() && choices[next].0.start < end {
            end = end.max(choices[next].0.end);
            next += 1;
        }
        let group = &choices[cursor..next];
        // A weak marker without punctuation can be an ordinary negation.
        // Every overlapping interpretation must have the explicit delimiters
        // and yield the same complete text before one representative is used.
        let mut agreed: Option<AcceptedTranscript> = None;
        for (range, edit) in group {
            if !correction::explicitly_delimited(&edit.before, language)
                || correction::requires_terminal_context(&edit.before, language)
                    && !terminal_tail(&original.as_str()[range.end..], language)
                    && !(correction::single_cardinal_repair(&edit.before, language)
                        && quantity_tail(&original.as_str()[range.end..], language))
            {
                return Ok(None);
            }
            let candidate = validate(original, std::slice::from_ref(edit), language)?;
            if agreed
                .as_ref()
                .is_some_and(|prior| prior.as_str() != candidate.as_str())
            {
                return Ok(None);
            }
            agreed = Some(candidate);
        }
        selected.push(group[0].1.clone());
        cursor = next;
    }
    let intermediate = validate(original, &selected, language)?;
    let intermediate = OriginalTranscript::new(intermediate.as_str().into())?;
    let finished = validate_staged(
        &intermediate,
        &postprocess(&intermediate, language)?,
        language,
    )?;
    let candidate = OriginalTranscript::new(finished.as_str().into())?;
    if !matches!(decide(&candidate, language), ProcessingDecision::Skip(_)) {
        return Ok(None);
    }
    let proposal = reconcile(original, candidate.as_str(), language)?;
    validate_staged(original, &proposal, language).map(Some)
}

// Even a comma-delimited correction can introduce a separate assertion.
// Except for explicit numeric repair, require a sentence boundary or a literal
// clock adjunct; arbitrary continuation still requires the model.
fn terminal_tail(tail: &str, language: Language) -> bool {
    let clause = first_clause(tail);
    if clause.is_empty() {
        return true;
    }
    let time = match language {
        Language::German => clause
            .strip_prefix("um ")
            .and_then(|s| s.strip_suffix(" Uhr")),
        Language::English => clause.strip_prefix("at "),
    };
    time.is_some_and(|time| {
        !time.is_empty()
            && time.len() <= 5
            && time.chars().any(|c| c.is_ascii_digit())
            && time.chars().all(|c| c.is_ascii_digit() || c == ':')
    })
}

fn first_clause(tail: &str) -> &str {
    let boundary = tail
        .char_indices()
        .find(|(index, c)| {
            matches!(c, '\n' | '\r')
                || matches!(c, '.' | '!' | '?')
                    && tail[index + c.len_utf8()..]
                        .chars()
                        .next()
                        .is_none_or(char::is_whitespace)
        })
        .map_or(tail.len(), |(index, _)| index);
    tail[..boundary].trim()
}

// A closed vocabulary of complete quantity units. No arbitrary noun, extra
// adjective or trailing predicate is treated as a deterministic quantity.
fn quantity_tail(tail: &str, language: Language) -> bool {
    let unit = first_clause(tail).to_lowercase();
    match language {
        Language::German => matches!(
            unit.as_str(),
            "person"
                | "personen"
                | "stück"
                | "stücke"
                | "karton"
                | "kartons"
                | "gerät"
                | "geräte"
                | "euro"
                | "cent"
                | "dollar"
                | "sekunde"
                | "sekunden"
                | "minute"
                | "minuten"
                | "stunde"
                | "stunden"
                | "gramm"
                | "kilogramm"
                | "meter"
                | "kilometer"
                | "liter"
                | "milliliter"
                | "prozent"
        ),
        Language::English => matches!(
            unit.as_str(),
            "person"
                | "people"
                | "item"
                | "items"
                | "box"
                | "boxes"
                | "device"
                | "devices"
                | "euro"
                | "euros"
                | "cent"
                | "cents"
                | "dollar"
                | "dollars"
                | "second"
                | "seconds"
                | "minute"
                | "minutes"
                | "hour"
                | "hours"
                | "gram"
                | "grams"
                | "kilogram"
                | "kilograms"
                | "meter"
                | "meters"
                | "kilometer"
                | "kilometers"
                | "liter"
                | "liters"
                | "milliliter"
                | "milliliters"
                | "percent"
        ),
    }
}
