use dictation_core::refinement::{Language, OriginalTranscript, proposal_candidates, validate};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn filler_options_own_exact_source_spans_and_leave_quotes_and_links_alone() -> TestResult {
    let original = OriginalTranscript::new(
        "Ähm, wir sagen, äh, Hallo. Im Zitat steht „äh“. https://example.org/äh".into(),
    )?;
    let choices = proposal_candidates(&original, Language::German)?;
    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0].before, "Ähm,");
    assert_eq!(choices[1].before, ", äh,");
    assert_eq!(
        validate(
            &original,
            &choices
                .iter()
                .filter(|c| c.after.is_empty())
                .cloned()
                .collect::<Vec<_>>(),
            Language::German
        )?
        .as_str(),
        "wir sagen Hallo. Im Zitat steht „äh“. https://example.org/äh"
    );
    Ok(())
}

#[test]
fn correction_options_keep_complete_values_and_do_not_turn_negation_into_correction() -> TestResult
{
    let original = OriginalTranscript::new("We meet on Monday, no Tuesday at 14:30.".into())?;
    let choices = proposal_candidates(&original, Language::English)?;
    let correction = choices
        .iter()
        .find(|c| c.before == "Monday, no Tuesday")
        .ok_or("missing short correction")?;
    assert_eq!(correction.after, "Tuesday");
    assert_eq!(
        validate(
            &original,
            std::slice::from_ref(correction),
            Language::English
        )?
        .as_str(),
        "We meet on Tuesday at 14:30."
    );
    for text in [
        "Nein.",
        "Fear no evil.",
        "Her name is Sara or Sarah; I am unsure.",
        "She said \"Monday, no Tuesday\".",
    ] {
        assert!(
            proposal_candidates(&OriginalTranscript::new(text.into())?, Language::English)?
                .is_empty()
        );
    }
    Ok(())
}

#[test]
fn comma_delimited_hesitation_offers_retaining_the_grammatical_comma() -> TestResult {
    let original = OriginalTranscript::new("Ich denke, äh, dass es klappt.".into())?;
    let choices = proposal_candidates(&original, Language::German)?;
    let retained = choices
        .iter()
        .find(|c| c.before == ", äh," && c.after == ",")
        .ok_or("missing comma-preserving alternative")?;
    assert_eq!(
        validate(&original, std::slice::from_ref(retained), Language::German)?.as_str(),
        "Ich denke, dass es klappt."
    );
    Ok(())
}

#[test]
fn adjacent_hesitations_form_one_option_without_overlapping_comma_ownership() -> TestResult {
    let original = OriginalTranscript::new("Ähm, äh, wir kommen.".into())?;
    let choices = proposal_candidates(&original, Language::German)?;
    let deletions = choices
        .iter()
        .filter(|c| c.after.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(deletions.len(), 1);
    assert_eq!(deletions[0].before, "Ähm, äh,");
    assert_eq!(
        validate(&original, &deletions, Language::German)?.as_str(),
        "wir kommen."
    );
    Ok(())
}

#[test]
fn numeric_options_never_replace_only_part_of_a_decimal() -> TestResult {
    let original =
        OriginalTranscript::new("Der Preis ist 12,50, Entschuldigung, 15,25 Euro.".into())?;
    let choices = proposal_candidates(&original, Language::German)?;
    assert!(!choices.is_empty());
    assert!(choices.iter().all(|c| c.before.starts_with("12,50")));
    let short = choices
        .iter()
        .find(|c| c.after == "15,25")
        .ok_or("missing decimal option")?;
    assert_eq!(
        validate(&original, std::slice::from_ref(short), Language::German)?.as_str(),
        "Der Preis ist 15,25 Euro."
    );
    Ok(())
}

#[test]
fn too_many_options_fail_instead_of_silently_omitting_later_changes() -> TestResult {
    let text = (0..65).map(|i| format!("äh Wort{i}. ")).collect::<String>();
    let original = OriginalTranscript::new(text)?;
    assert!(proposal_candidates(&original, Language::German).is_err());
    Ok(())
}

#[test]
fn missing_sentence_case_and_terminal_punctuation_are_available_as_local_options() -> TestResult {
    let original = OriginalTranscript::new("wir senden den Bericht".into())?;
    let choices = proposal_candidates(&original, Language::German)?;
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].before, "wir");
    assert_eq!(choices[0].after, "Wir");
    assert_eq!(choices[1].before, "Bericht");
    assert_eq!(choices[1].after, "Bericht.");
    assert_eq!(
        validate(&original, &choices, Language::German)?.as_str(),
        "Wir senden den Bericht."
    );
    Ok(())
}

#[test]
fn numeric_corrections_own_signs_and_terminal_literal_punctuation() -> TestResult {
    for (source, expected) in [
        (
            "Der Betrag ist 50, Entschuldigung, 15.",
            "Der Betrag ist 15.",
        ),
        (
            "Der Betrag ist -50, Entschuldigung, 15 Euro.",
            "Der Betrag ist 15 Euro.",
        ),
        ("Uhrzeit 14:30, Korrektur 15:45.", "Uhrzeit 15:45."),
    ] {
        let original = OriginalTranscript::new(source.into())?;
        let choices = proposal_candidates(&original, Language::German)?;
        assert!(
            choices.iter().any(|edit| validate(
                &original,
                std::slice::from_ref(edit),
                Language::German
            )
            .is_ok_and(|accepted| accepted.as_str() == expected)),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn invalid_comma_retention_does_not_discard_a_valid_trailing_filler_removal() -> TestResult {
    let original = OriginalTranscript::new("Wir sind fertig, äh,".into())?;
    let choices = proposal_candidates(&original, Language::German)?;
    let deletion = choices
        .iter()
        .find(|c| c.after.is_empty())
        .ok_or("missing trailing deletion")?;
    assert_eq!(
        validate(&original, std::slice::from_ref(deletion), Language::German)?.as_str(),
        "Wir sind fertig"
    );
    Ok(())
}

#[test]
fn duplicate_numeric_windows_do_not_consume_an_additional_candidate_slot() -> TestResult {
    let mut text = (0..63).map(|i| format!("Äh Wort{i}. ")).collect::<String>();
    text.push_str("ist 12,50, Entschuldigung, 15,25.");
    let choices = proposal_candidates(&OriginalTranscript::new(text)?, Language::German)?;
    assert_eq!(choices.len(), 64);
    Ok(())
}

#[test]
fn statement_correction_options_include_the_complete_revised_clause() -> TestResult {
    for (text, expected, language) in [
        (
            "Die Option ist aktiviert, Korrektur: Die Option ist nicht aktiviert.",
            "Die Option ist nicht aktiviert.",
            Language::German,
        ),
        (
            "I approve the draft, correction: I do not approve the draft.",
            "I do not approve the draft.",
            Language::English,
        ),
    ] {
        let source = OriginalTranscript::new(text.into())?;
        let choices = proposal_candidates(&source, language)?;
        assert!(
            choices.iter().any(
                |choice| validate(&source, std::slice::from_ref(choice), language)
                    .is_ok_and(|accepted| accepted.as_str() == expected)
            ),
            "{text}"
        );
        assert!(
            !choices
                .iter()
                .any(|choice| choice.after == "Die" || choice.after == "I")
        );
    }
    Ok(())
}

#[test]
fn numeric_replacements_cannot_consume_a_preceding_predicate() -> TestResult {
    for (text, language, prefix, expected) in [
        (
            "Der Termin dauert 40, Entschuldigung, 14 Minuten.",
            Language::German,
            "Der Termin dauert ",
            "Der Termin dauert 14 Minuten.",
        ),
        (
            "Die Menge beträgt sechzig, Entschuldigung, sechzehn Kartons.",
            Language::German,
            "Die Menge beträgt ",
            "Die Menge beträgt sechzehn Kartons.",
        ),
        (
            "Der Einkauf kostet fünfundzwanzig, Entschuldigung, siebenundzwanzig Euro.",
            Language::German,
            "Der Einkauf kostet ",
            "Der Einkauf kostet siebenundzwanzig Euro.",
        ),
        (
            "The order costs sixty, sorry, sixteen euros.",
            Language::English,
            "The order costs ",
            "The order costs sixteen euros.",
        ),
    ] {
        let source = OriginalTranscript::new(text.into())?;
        let choices = proposal_candidates(&source, language)?;
        let mut correct = false;
        for choice in choices {
            let accepted = validate(&source, std::slice::from_ref(&choice), language)?;
            correct |= accepted.as_str() == expected;
            assert!(
                accepted.as_str().starts_with(prefix),
                "{}",
                accepted.as_str()
            );
        }
        assert!(correct, "required correction absent: {text}");
    }
    Ok(())
}
