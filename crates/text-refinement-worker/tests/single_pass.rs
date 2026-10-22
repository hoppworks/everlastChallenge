use dictation_core::refinement::validate_staged;
use text_refinement_protocol::Language;
use text_refinement_worker::single_pass::Plan;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn one_model_response_combines_independent_edits_and_punctuation() -> TestResult {
    let source = "Ähm, wir kommen am Montag, nein, am Dienstag";
    let plan = Plan::new(source, Language::German).map_err(|e| format!("{e:?}"))?;
    let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
    let edits = plan
        .generate(|_, _| Ok(serde_json::json!({"text":"Wir kommen am Dienstag."}).to_string()))
        .map_err(|e| format!("{e:?}"))?;
    let accepted = validate_staged(
        &original,
        &edits,
        dictation_core::refinement::Language::German,
    )?;
    assert_eq!(accepted.as_str(), "Wir kommen am Dienstag.");
    Ok(())
}

#[test]
fn unrelated_final_text_is_never_accepted() -> TestResult {
    let source = "Der Termin ist am Montag, nein, am Dienstag.";
    for text in [
        "Der Termin ist am Freitag.",
        "Der wichtige Termin ist am Dienstag.",
    ] {
        let plan = Plan::new(source, Language::German).map_err(|e| format!("{e:?}"))?;
        let result = plan.generate(|_, _| Ok(serde_json::json!({"text":text}).to_string()));
        assert!(result.is_err(), "Unexpectedly accepted {text}");
    }
    Ok(())
}

#[test]
fn invalid_response_never_triggers_another_model_call() -> TestResult {
    use std::cell::Cell;
    use text_refinement_protocol::FailureKind;
    for response in [
        r#"{"text":"Hallo.","extra":true}"#,
        r#"{"text":"Hallo.","text":"Hallo."}"#,
        "incomplete",
    ] {
        let calls = Cell::new(0);
        let result = Plan::new("Hallo", Language::German)
            .map_err(|e| format!("{e:?}"))?
            .generate(|_, _| {
                calls.set(calls.get() + 1);
                Ok(response.into())
            });
        assert_eq!(calls.get(), 1);
        assert!(matches!(result, Err(FailureKind::InvalidCandidate)));
    }
    Ok(())
}

#[test]
fn unusual_punctuation_cannot_create_an_unbounded_grammar() {
    let source: String = (0x2190..0x2300).filter_map(char::from_u32).collect();
    assert!(matches!(
        Plan::new(&source, Language::German),
        Err(text_refinement_protocol::FailureKind::InputLimit)
    ));
}

#[test]
fn one_response_finishes_explicit_numeric_correction_without_model_retry() -> TestResult {
    let source = "Die Menge beträgt 60, Entschuldigung, 16 Kartons.";
    let calls = std::cell::Cell::new(0);
    let edits = Plan::new(source, Language::German)
        .map_err(|e| format!("{e:?}"))?
        .generate(|_, _| {
            calls.set(calls.get() + 1);
            Ok(serde_json::json!({"text":source}).to_string())
        })
        .map_err(|e| format!("{e:?}"))?;
    let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
    assert_eq!(
        validate_staged(
            &original,
            &edits,
            dictation_core::refinement::Language::German
        )?
        .as_str(),
        "Die Menge beträgt 16 Kartons."
    );
    assert_eq!(calls.get(), 1);
    Ok(())
}

#[test]
fn one_response_finishes_sentence_case_but_preserves_quoted_words() -> TestResult {
    let source = "we received your report. thank you for the update";
    let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
    let edits = Plan::new(source, Language::English)
        .map_err(|e| format!("{e:?}"))?
        .generate(|_, _| Ok(serde_json::json!({"text":source}).to_string()))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        validate_staged(
            &original,
            &edits,
            dictation_core::refinement::Language::English
        )?
        .as_str(),
        "We received your report. Thank you for the update."
    );
    Ok(())
}

#[test]
fn deterministic_finishing_preserves_quotes_literals_and_weak_negation() -> TestResult {
    for (source, expected) in [
        (
            "He said “hello. thank you”. iPhone remains.",
            "He said “hello. thank you”. iPhone remains.",
        ),
        (
            "Die Wörter „60, Entschuldigung, 16“ bleiben unverändert.",
            "Die Wörter „60, Entschuldigung, 16“ bleiben unverändert.",
        ),
        (
            "Die Kennung ist AB-204. Bitte schreibe 60, nein, 16.",
            "Die Kennung ist AB-204. Bitte schreibe 60, nein, 16.",
        ),
    ] {
        let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
        let edits = Plan::new(source, Language::German)
            .map_err(|e| format!("{e:?}"))?
            .generate(|_, _| Ok(serde_json::json!({"text":source}).to_string()))
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            validate_staged(
                &original,
                &edits,
                dictation_core::refinement::Language::German
            )?
            .as_str(),
            expected
        );
    }
    Ok(())
}

#[test]
fn ambiguous_numeric_chain_is_not_partially_rewritten() -> TestResult {
    let source = "The count is 50, sorry, 15, correction, 12.";
    let calls = std::cell::Cell::new(0);
    let edits = Plan::new(source, Language::English)
        .map_err(|e| format!("{e:?}"))?
        .generate(|_, _| {
            calls.set(calls.get() + 1);
            Ok(serde_json::json!({"text":source}).to_string())
        })
        .map_err(|e| format!("{e:?}"))?;
    let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
    assert_eq!(
        validate_staged(
            &original,
            &edits,
            dictation_core::refinement::Language::English
        )?
        .as_str(),
        source
    );
    assert_eq!(calls.get(), 1);
    Ok(())
}

#[test]
fn initial_hesitation_does_not_shield_a_later_reported_quote() -> TestResult {
    let source = "Ähm, Nora sagte wörtlich, bitte nicht löschen. Die Nachricht geht an David.";
    let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
    let edits = Plan::new(source, Language::German)
        .map_err(|e| format!("{e:?}"))?
        .generate(|_, _| Ok(serde_json::json!({"text":source}).to_string()))
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        validate_staged(
            &original,
            &edits,
            dictation_core::refinement::Language::German
        )?
        .as_str(),
        "Nora sagte wörtlich, bitte nicht löschen. Die Nachricht geht an David."
    );
    Ok(())
}

#[test]
fn initial_hesitation_is_bound_to_full_source_not_truncated_context() -> TestResult {
    let internal = format!("Hallo.{}Ähm, weiter.", " ".repeat(200));
    for (source, expected) in [
        (internal.as_str(), internal.as_str()),
        ("  Ähm, weiter.", "Weiter."),
    ] {
        let original = dictation_core::refinement::OriginalTranscript::new(source.into())?;
        let edits = Plan::new(source, Language::German)
            .map_err(|e| format!("{e:?}"))?
            .generate(|_, _| Ok(serde_json::json!({"text":source}).to_string()))
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            validate_staged(
                &original,
                &edits,
                dictation_core::refinement::Language::German
            )?
            .as_str(),
            expected
        );
    }
    Ok(())
}

#[test]
fn model_cannot_remove_or_move_a_source_paragraph_break() -> TestResult {
    let source = "Der Bericht ist fertig.\n\nBitte lies ihn morgen.";
    for output in [
        "Der Bericht ist fertig. Bitte lies ihn morgen.",
        "Der Bericht\n\nist fertig. Bitte lies ihn morgen.",
    ] {
        let result = Plan::new(source, Language::German)
            .map_err(|e| format!("{e:?}"))?
            .generate(|_, _| Ok(serde_json::json!({"text":output}).to_string()));
        assert!(matches!(
            result,
            Err(text_refinement_protocol::FailureKind::InvalidCandidate)
        ));
    }
    Ok(())
}
