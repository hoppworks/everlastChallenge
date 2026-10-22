use dictation_core::refinement::{Language, OriginalTranscript, propose_punctuation, validate};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn punctuation_proposals_preserve_unicode_words_and_exact_output() -> TestResult {
    let source = OriginalTranscript::new("wir grüßen Müller vielen Dank".into())?;
    let target = "Wir grüßen Müller. Vielen Dank.";
    let edits = propose_punctuation(&source, target, Language::German)?;
    assert_eq!(
        validate(&source, &edits, Language::German)?.as_str(),
        target
    );
    Ok(())
}
#[test]
fn changed_words_numbers_quotes_and_internal_case_are_rejected() -> TestResult {
    for (source, target) in [
        ("send 15 euros", "Send 50 euros."),
        ("do not send", "Do send."),
        ("send report", "Send secret report."),
        ("read https://example.org", "Read https://Example.org."),
        ("he said \"stay calm\"", "He said \"Stay calm\"."),
        ("visit iPhone today", "Visit IPhone today."),
    ] {
        assert!(
            propose_punctuation(
                &OriginalTranscript::new(source.into())?,
                target,
                Language::English
            )
            .is_err(),
            "{source}"
        );
    }
    Ok(())
}
#[test]
fn unchanged_inputs_and_repeated_words_do_not_need_unanchored_replacements() -> TestResult {
    let source = OriginalTranscript::new("no no no".into())?;
    let edits = propose_punctuation(&source, "No no no.", Language::English)?;
    assert_eq!(
        validate(&source, &edits, Language::English)?.as_str(),
        "No no no."
    );
    for text in ["...", "", "Müller", "  keep\n\nthis  "] {
        assert!(
            propose_punctuation(
                &OriginalTranscript::new(text.into())?,
                text,
                Language::English
            )?
            .is_empty()
        );
    }
    assert!(
        propose_punctuation(
            &OriginalTranscript::new("...".into())?,
            "!",
            Language::English
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn unchanged_quotes_and_dashes_do_not_enter_adjacent_case_edits() -> TestResult {
    for (source, target) in [
        ("hello \"stay calm\"", "Hello \"stay calm\""),
        ("hallo – Welt", "Hallo – Welt"),
        ("  hello \"stay calm\"", "  Hello \"stay calm\""),
    ] {
        let original = OriginalTranscript::new(source.into())?;
        let edits = propose_punctuation(&original, target, Language::English)?;
        assert_eq!(
            validate(&original, &edits, Language::English)?.as_str(),
            target
        );
    }
    Ok(())
}
