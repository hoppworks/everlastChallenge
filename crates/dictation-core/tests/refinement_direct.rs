use dictation_core::refinement::{Language, OriginalTranscript, try_direct};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn explicit_delimited_correction_completes_with_original_authority() -> TestResult {
    let source = OriginalTranscript::new(
        "Der Termin ist am Dienstag, nein, am Donnerstag um 10 Uhr.".into(),
    )?;
    let accepted = try_direct(&source, Language::German)?.ok_or("needs model")?;
    assert_eq!(accepted.as_str(), "Der Termin ist am Donnerstag um 10 Uhr.");
    assert!(accepted.original() == &source);
    assert_eq!(accepted.language(), Language::German);
    Ok(())
}

#[test]
fn unchanged_inputs_remain_byte_identical() -> TestResult {
    for text in [
        "hello",
        "  hello  ",
        "https://example.org/a",
        "0017",
        "\n",
        "Das bleibt so.",
    ] {
        let source = OriginalTranscript::new(text.into())?;
        assert_eq!(
            try_direct(&source, Language::German)?
                .ok_or("needs model")?
                .as_str(),
            text
        );
    }
    Ok(())
}

#[test]
fn ambiguous_or_incomplete_cleanup_leaves_the_original_for_the_model() -> TestResult {
    for (text, language) in [
        (
            "Book it for Thursday, no Saturday at 14:30.",
            Language::English,
        ),
        (
            "There is no reason to remove this sentence.",
            Language::English,
        ),
        (
            "Ähm, wir schicken, äh, den Bericht morgen.",
            Language::German,
        ),
        (
            "Nein, nein, nein! Das habe ich nicht zugesagt.",
            Language::German,
        ),
        (
            "wir sind angekommen jetzt beginnt die Besprechung",
            Language::German,
        ),
        (
            "Wir kommen am Montag, nein am Dienstag um acht, Entschuldigung, neun Uhr.",
            Language::German,
        ),
    ] {
        let source = OriginalTranscript::new(text.into())?;
        assert!(try_direct(&source, language)?.is_none(), "{text}");
        assert_eq!(source.as_str(), text);
    }
    Ok(())
}

#[test]
fn independent_corrections_preserve_paragraphs_literals_and_negation() -> TestResult {
    let source = OriginalTranscript::new(
        concat!(
            "Ähm, die Nachricht geht an Emil, nein, an David.\n\n",
            "Die Menge ist 60, Entschuldigung, 16 Kartons. ",
            "Ändere retryCount und AB-204 nicht."
        )
        .into(),
    )?;
    let accepted = try_direct(&source, Language::German)?.ok_or("needs model")?;
    assert_eq!(
        accepted.as_str(),
        concat!(
            "Die Nachricht geht an David.\n\n",
            "Die Menge ist 16 Kartons. Ändere retryCount und AB-204 nicht."
        )
    );
    Ok(())
}

#[test]
fn comma_delimited_negation_can_still_begin_a_new_assertion() -> TestResult {
    for text in [
        "We can meet Monday, no, Tuesday is unavailable.",
        "We can meet on Monday, no, Tuesday is unavailable.",
        "We can meet on Monday, sorry, Tuesday is unavailable.",
        "We can meet on Monday, no, Tuesday at 14.30 is unavailable.",
        "We can meet on Monday, no, Tuesday at 14.30.",
    ] {
        let original = OriginalTranscript::new(text.into())?;
        assert!(
            try_direct(&original, Language::English)?.is_none(),
            "{text}"
        );
    }
    Ok(())
}

#[test]
fn clock_adjunct_requires_a_complete_sentence_boundary() -> TestResult {
    let original = OriginalTranscript::new("We can meet on Monday, no, Tuesday at 14:30.".into())?;
    assert_eq!(
        try_direct(&original, Language::English)?
            .ok_or("needs model")?
            .as_str(),
        "We can meet on Tuesday at 14:30."
    );
    Ok(())
}

#[test]
fn complete_cardinal_quantity_repairs_need_no_model_generation() -> TestResult {
    for (input, expected, language) in [
        (
            "Für den Aufbau brauchen wir sechs, nein, acht Personen.",
            "Für den Aufbau brauchen wir acht Personen.",
            Language::German,
        ),
        (
            "The shipment contains six, no, eight boxes.",
            "The shipment contains eight boxes.",
            Language::English,
        ),
    ] {
        let original = OriginalTranscript::new(input.into())?;
        assert_eq!(
            try_direct(&original, language)?
                .ok_or("needs model")?
                .as_str(),
            expected
        );
    }
    Ok(())
}

#[test]
fn quantity_route_rejects_predicates_modifiers_and_number_fragments() -> TestResult {
    for (text, language) in [
        (
            "Wir erwarten sechs, nein, acht Personen fehlen noch.",
            Language::German,
        ),
        (
            "Wir erwarten sechs, nein, acht weitere Personen.",
            Language::German,
        ),
        (
            "We counted six, no, eight people were injured.",
            Language::English,
        ),
        (
            "We counted six, no, eight people at 14.30 were injured.",
            Language::English,
        ),
        (
            "We counted twenty one, no, thirty two boxes.",
            Language::English,
        ),
        (
            "We counted one hundred, no, two hundred boxes.",
            Language::English,
        ),
        ("We counted six, no, eight m.", Language::English),
        ("We counted hundred, no, eight boxes.", Language::English),
        (
            "Wir erwarten Millionen, nein, acht Personen.",
            Language::German,
        ),
    ] {
        let original = OriginalTranscript::new(text.into())?;
        assert!(try_direct(&original, language)?.is_none(), "{text}");
    }
    Ok(())
}
