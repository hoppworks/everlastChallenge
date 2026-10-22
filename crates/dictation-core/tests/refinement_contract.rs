//! Public text-acceptance contracts. These do not qualify a language model.
use dictation_core::refinement::{EditKind, Language, OriginalTranscript, ProposedEdit, validate};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn edit(kind: EditKind, before: &str, after: &str) -> ProposedEdit {
    ProposedEdit {
        kind,
        before: before.into(),
        after: after.into(),
        left_context: None,
        right_context: None,
    }
}

#[test]
fn filler_deletion_keeps_words_separate_without_leading_or_double_spaces() -> TestResult {
    let original = OriginalTranscript::new("Ähm, wir schicken, äh, den Bericht morgen.".into())?;
    let accepted = validate(
        &original,
        &[
            edit(EditKind::Filler, "Ähm,", ""),
            edit(EditKind::Filler, ", äh,", ""),
        ],
        Language::German,
    )?;
    assert_eq!(accepted.as_str(), "wir schicken den Bericht morgen.");
    assert_eq!(accepted.edit_count(), 2);
    assert_eq!(
        original.as_str(),
        "Ähm, wir schicken, äh, den Bericht morgen."
    );
    Ok(())
}

#[test]
fn a_quoted_filler_cannot_be_removed_even_alongside_a_valid_change() -> TestResult {
    let original = OriginalTranscript::new("Ähm, sie sagte: „äh“ und ging.".into())?;
    let result = validate(
        &original,
        &[
            edit(EditKind::Filler, "Ähm,", ""),
            edit(EditKind::Filler, "äh", ""),
        ],
        Language::German,
    );
    assert!(result.is_err());
    assert_eq!(original.as_str(), "Ähm, sie sagte: „äh“ und ging.");
    Ok(())
}

#[test]
fn an_explicit_weekday_correction_removes_the_retracted_day_and_marker() -> TestResult {
    let original =
        OriginalTranscript::new("Der Termin ist am Dienstag, nein am Donnerstag um 9 Uhr.".into())?;
    let accepted = validate(
        &original,
        &[edit(
            EditKind::SelfCorrection,
            "am Dienstag, nein am Donnerstag",
            "am Donnerstag",
        )],
        Language::German,
    )?;
    assert_eq!(accepted.as_str(), "Der Termin ist am Donnerstag um 9 Uhr.");
    Ok(())
}

#[test]
fn sentence_case_and_punctuation_can_follow_a_valid_filler_removal() -> TestResult {
    let original = OriginalTranscript::new("Ähm, wir schicken, äh, den Bericht morgen".into())?;
    let accepted = validate(
        &original,
        &[
            edit(EditKind::Filler, "Ähm,", ""),
            edit(EditKind::PunctuationCase, "wir", "Wir"),
            edit(EditKind::Filler, ", äh,", ""),
            edit(EditKind::PunctuationCase, "morgen", "morgen."),
        ],
        Language::German,
    )?;
    assert_eq!(accepted.as_str(), "Wir schicken den Bericht morgen.");
    Ok(())
}

#[test]
fn repeated_unicode_sources_require_an_exact_adjacent_context() -> TestResult {
    let original = OriginalTranscript::new("Müller sagt äh und später äh zum Termin.".into())?;
    assert!(
        validate(
            &original,
            &[edit(EditKind::Filler, "äh", "")],
            Language::German
        )
        .is_err()
    );
    let mut anchored = edit(EditKind::Filler, "äh", "");
    anchored.left_context = Some("später ".into());
    anchored.right_context = Some(" zum Termin".into());
    assert_eq!(
        validate(&original, &[anchored], Language::German)?.as_str(),
        "Müller sagt äh und später zum Termin."
    );
    Ok(())
}

#[test]
fn invalid_edits_never_return_a_partial_result() -> TestResult {
    let original = OriginalTranscript::new("Ähm, der Betrag ist 12,50 Euro, nicht 1250.".into())?;
    for invalid in [
        edit(EditKind::PunctuationCase, "12,50", "1250"),
        edit(EditKind::PunctuationCase, ",", ""),
        edit(EditKind::PunctuationCase, "nicht", ""),
        edit(EditKind::SelfCorrection, "nicht 1250", "1250"),
        edit(EditKind::Filler, "Betrag", ""),
    ] {
        assert!(
            validate(
                &original,
                &[edit(EditKind::Filler, "Ähm,", ""), invalid],
                Language::German
            )
            .is_err()
        );
    }
    assert_eq!(
        original.as_str(),
        "Ähm, der Betrag ist 12,50 Euro, nicht 1250."
    );
    Ok(())
}

#[test]
fn corrections_keep_the_frame_and_reject_partial_markers_or_multiple_corrections() -> TestResult {
    for (source, before, after) in [
        (
            "Der Termin ist am Dienstag, nein am Donnerstag.",
            "nein am Donnerstag",
            "am Donnerstag",
        ),
        (
            "Nicht am Montag, nein am Dienstag.",
            "Nicht am Montag, nein am Dienstag",
            "am Dienstag",
        ),
        (
            "Ich komme am Montag, nein Dienstag, nein Mittwoch.",
            "Montag, nein Dienstag, nein Mittwoch",
            "Mittwoch",
        ),
        (
            "Sie prüft alles. Dienstag, nein Donnerstag.",
            "Sie prüft alles. Dienstag, nein Donnerstag",
            "Donnerstag",
        ),
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(source.into())?,
                &[edit(EditKind::SelfCorrection, before, after)],
                Language::German
            )
            .is_err()
        );
    }
    let amount = OriginalTranscript::new("Der Betrag ist 50, Entschuldigung, 15 Euro.".into())?;
    assert_eq!(
        validate(
            &amount,
            &[edit(
                EditKind::SelfCorrection,
                "50, Entschuldigung, 15 Euro",
                "15 Euro"
            )],
            Language::German
        )?
        .as_str(),
        "Der Betrag ist 15 Euro."
    );
    Ok(())
}

#[test]
fn punctuation_cannot_recase_names_change_literals_or_modify_abbreviations() -> TestResult {
    for (source, before, after) in [
        ("Peter sieht Anna.", "Anna", "anna"),
        ("Das ist iPhone.", "iPhone", "Iphone"),
        ("Öffne https://example.org/a?x=1.", "org", "Org"),
        ("Kontaktiere a@b.de.", "de", "de!"),
        ("Version v1.2.3 bleibt.", ".", ","),
        ("Bitte Dr. Maier fragen.", "Dr.", "Dr"),
        ("Die ID ist AB_19.", "AB_19", "AB19"),
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(source.into())?,
                &[edit(EditKind::PunctuationCase, before, after)],
                Language::German
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn quoted_and_unclosed_quoted_data_remain_immutable() -> TestResult {
    for source in [
        "Er sagt \"äh\".",
        "Sie sagt 'äh'.",
        "Sie sagt «äh».",
        "Sie sagt »äh«.",
        "Sie sagt „äh“, dann geht sie.",
        "Sie sagt „äh",
        "Code: `äh`",
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(source.into())?,
                &[edit(EditKind::Filler, "äh", "")],
                Language::German
            )
            .is_err()
        );
    }
    let english = OriginalTranscript::new("Don't, um, change Anna's name.".into())?;
    assert_eq!(
        validate(
            &english,
            &[edit(EditKind::Filler, ", um,", "")],
            Language::English
        )?
        .as_str(),
        "Don't change Anna's name."
    );
    Ok(())
}

#[test]
fn bounded_acceptance_rejects_overlap_and_preserves_long_unchanged_input() -> TestResult {
    let original = OriginalTranscript::new("äh ähm".into())?;
    assert!(
        validate(
            &original,
            &[
                edit(EditKind::Filler, "äh", ""),
                edit(EditKind::Filler, "äh ähm", "")
            ],
            Language::German
        )
        .is_err()
    );
    assert!(
        validate(
            &original,
            &vec![edit(EditKind::Filler, "äh", ""); 33],
            Language::German
        )
        .is_err()
    );
    let long = OriginalTranscript::new("Abschnitt bleibt vollständig erhalten. ".repeat(70))?;
    assert_eq!(
        validate(&long, &[], Language::German)?.as_str(),
        long.as_str()
    );
    assert!(OriginalTranscript::new("x".repeat(32 * 1024 + 1)).is_err());
    Ok(())
}

#[test]
fn filler_words_inside_urls_and_addresses_are_immutable() -> TestResult {
    for (input, word, language) in [
        (
            "Open https://example.org/um/manual now.",
            "um",
            Language::English,
        ),
        ("Schreibe hm@example.org morgen.", "hm", Language::German),
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(input.into())?,
                &[edit(EditKind::Filler, word, "")],
                language
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn a_correction_cannot_start_inside_a_decimal_or_signed_number() -> TestResult {
    for (input, before) in [
        ("Der Betrag ist 12,50, nein 15 Euro.", "50, nein 15 Euro"),
        ("Der Betrag ist -50, nein 15 Euro.", "50, nein 15 Euro"),
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(input.into())?,
                &[edit(EditKind::SelfCorrection, before, "15 Euro")],
                Language::German
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn paired_comma_filler_deletion_requires_explicit_punctuation_ownership() -> TestResult {
    let original = OriginalTranscript::new("Wir schicken, äh, den Bericht morgen.".into())?;
    for before in ["äh", "äh,", ", äh"] {
        assert!(
            validate(
                &original,
                &[edit(EditKind::Filler, before, "")],
                Language::German
            )
            .is_err()
        );
    }
    assert_eq!(
        validate(
            &original,
            &[edit(EditKind::Filler, ", äh,", "")],
            Language::German
        )?
        .as_str(),
        "Wir schicken den Bericht morgen."
    );
    let subordinate = OriginalTranscript::new("Ich denke, äh, dass es klappt.".into())?;
    assert_eq!(
        validate(
            &subordinate,
            &[edit(EditKind::Filler, ", äh,", ",")],
            Language::German
        )?
        .as_str(),
        "Ich denke, dass es klappt."
    );
    Ok(())
}

#[test]
fn explicit_correction_does_not_require_already_punctuated_asr() -> TestResult {
    for (input, before, after, expected, language) in [
        (
            "Der Termin ist Dienstag nein Donnerstag.",
            "Dienstag nein Donnerstag",
            "Donnerstag",
            "Der Termin ist Donnerstag.",
            Language::German,
        ),
        (
            "We meet Tuesday no Thursday.",
            "Tuesday no Thursday",
            "Thursday",
            "We meet Thursday.",
            Language::English,
        ),
    ] {
        assert_eq!(
            validate(
                &OriginalTranscript::new(input.into())?,
                &[edit(EditKind::SelfCorrection, before, after)],
                language
            )?
            .as_str(),
            expected
        );
    }
    Ok(())
}

#[test]
fn ordinary_negation_and_path_components_do_not_authorize_replacement() -> TestResult {
    for (source, before, after, language) in [
        (
            "Keep foo/no/bar intact.",
            "foo/no/bar",
            "/bar",
            Language::English,
        ),
        ("Fear no evil.", "Fear no evil", "evil", Language::English),
        (
            "Peter sagt nein zum Vertrag.",
            "Peter sagt nein zum Vertrag",
            "zum Vertrag",
            Language::German,
        ),
    ] {
        assert!(
            validate(
                &OriginalTranscript::new(source.into())?,
                &[edit(EditKind::SelfCorrection, before, after)],
                language
            )
            .is_err()
        );
    }
    Ok(())
}
