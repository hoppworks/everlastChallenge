use dictation_core::refinement::{Language, OriginalTranscript, reconcile, validate_staged};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn derives_only_source_bound_corrections_from_one_final_text() -> TestResult {
    let original = OriginalTranscript::new("Ähm, wir kommen am Montag, nein, am Dienstag".into())?;
    let proposal = reconcile(&original, "Wir kommen am Dienstag.", Language::German)?;
    assert_eq!(
        validate_staged(&original, &proposal, Language::German)?.as_str(),
        "Wir kommen am Dienstag."
    );
    assert_eq!(proposal.structural.len(), 2);
    Ok(())
}

#[test]
fn preserves_authority_over_names_negation_quotes_and_literals() -> TestResult {
    for (source, output) in [
        ("Mara kommt morgen.", "Maria kommt morgen."),
        ("Bitte nicht löschen.", "Bitte löschen."),
        (
            "Er sagte: „ähm, nichts ändern“.",
            "Er sagte: „nichts ändern“.",
        ),
        (
            "Öffne https://example.org/a-b?q=12.",
            "Öffne https://example.org/a-b?q=13.",
        ),
        ("Ähm, der Code ist AB-204.", "Der Code ist AB-205."),
        (
            "Im Büro bleibt die Nummer 42.",
            "Büro bleibt die Nummer 42.",
        ),
    ] {
        let original = OriginalTranscript::new(source.into())?;
        assert!(
            reconcile(&original, output, Language::German).is_err(),
            "{source} -> {output}"
        );
    }
    Ok(())
}

#[test]
fn repeated_spans_and_multiple_corrections_keep_the_right_occurrences() -> TestResult {
    let source = "Ähm, am Montag, nein, am Dienstag. Am Montag bleibt der Laden zu. Die Menge ist zwölf, nein, sechzehn.";
    let output = "Am Dienstag. Am Montag bleibt der Laden zu. Die Menge ist sechzehn.";
    let original = OriginalTranscript::new(source.into())?;
    let edits = reconcile(&original, output, Language::German)?;
    assert_eq!(
        validate_staged(&original, &edits, Language::German)?.as_str(),
        output
    );
    Ok(())
}

#[test]
fn keeps_alternative_punctuation_paths_until_exact_validation() -> TestResult {
    let source = "„Hallo“, äh, sagte er.";
    let output = "„Hallo“, sagte er.";
    let original = OriginalTranscript::new(source.into())?;
    let edits = reconcile(&original, output, Language::German)?;
    assert_eq!(
        validate_staged(&original, &edits, Language::German)?.as_str(),
        output
    );
    Ok(())
}
