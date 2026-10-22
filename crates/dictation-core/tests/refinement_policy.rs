//! Routing decisions preserve original bytes and do not perform corrections.
use dictation_core::refinement::{
    Language, OriginalTranscript, ProcessingDecision, RefinementSignal, decide,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn short_corrections_are_not_skipped_because_of_their_length() -> TestResult {
    let original = OriginalTranscript::new("Dienstag, nein Donnerstag".into())?;
    assert!(
        matches!(decide(&original, Language::German),ProcessingDecision::Refine(signals) if signals.contains(&RefinementSignal::ExplicitCorrection))
    );
    assert_eq!(original.as_str(), "Dienstag, nein Donnerstag");
    Ok(())
}

#[test]
fn exact_values_and_clean_text_skip_without_losing_any_bytes() -> TestResult {
    use dictation_core::refinement::SkipReason;
    for (input, reason) in [
        ("", SkipReason::Empty),
        (" \n ", SkipReason::Empty),
        ("001204", SkipReason::ExactValue),
        ("-12,5 %", SkipReason::ExactValue),
        ("https://example.org/um?x=1", SkipReason::ExactValue),
        ("aeh@example.org", SkipReason::ExactValue),
        ("AB_19", SkipReason::ExactValue),
        ("Guten Tag!", SkipReason::NoCorrectionSignal),
        (
            "Der Bericht wird morgen an das Team geschickt.",
            SkipReason::NoCorrectionSignal,
        ),
        ("Sie sagt „äh“.", SkipReason::NoCorrectionSignal),
        ("iPhone bleibt iPhone.", SkipReason::NoCorrectionSignal),
    ] {
        let original = OriginalTranscript::new(input.into())?;
        assert_eq!(
            decide(&original, Language::German),
            ProcessingDecision::Skip(reason),
            "{input}"
        );
        assert_eq!(original.as_str(), input);
    }
    Ok(())
}

#[test]
fn numbers_or_quotes_inside_a_sentence_do_not_hide_external_corrections() -> TestResult {
    for input in [
        "Ähm, der Betrag ist 12,50 Euro.",
        "Sie sagt „äh“, äh, und geht.",
        "Wir, äh, öffnen https://example.org.",
    ] {
        let original = OriginalTranscript::new(input.into())?;
        assert!(
            matches!(decide(&original,Language::German),ProcessingDecision::Refine(signals) if signals.contains(&RefinementSignal::Filler)),
            "{input}"
        );
    }
    Ok(())
}

#[test]
fn english_filler_and_correction_signals_are_language_bound() -> TestResult {
    for (input, signal) in [
        ("Um, send this tomorrow.", RefinementSignal::Filler),
        ("Tuesday, no Thursday", RefinementSignal::ExplicitCorrection),
    ] {
        assert!(
            matches!(decide(&OriginalTranscript::new(input.into())?,Language::English),ProcessingDecision::Refine(signals) if signals.contains(&signal))
        );
    }
    let german = decide(
        &OriginalTranscript::new("Um 9 Uhr ist der Termin.".into())?,
        Language::German,
    );
    assert!(
        !matches!(german,ProcessingDecision::Refine(signals) if signals.contains(&RefinementSignal::Filler))
    );
    Ok(())
}

#[test]
fn missing_punctuation_and_repetition_request_review_without_editing() -> TestResult {
    for (input, signal) in [
        (
            "Wir senden morgen diesen Bericht",
            RefinementSignal::Punctuation,
        ),
        (
            "Wir senden senden den Bericht.",
            RefinementSignal::Repetition,
        ),
    ] {
        let original = OriginalTranscript::new(input.into())?;
        assert!(
            matches!(decide(&original,Language::German),ProcessingDecision::Refine(signals) if signals.contains(&signal))
        );
        assert_eq!(original.as_str(), input);
    }
    Ok(())
}

#[test]
fn numeric_corrections_still_trigger_when_numbers_are_protected() -> TestResult {
    for input in ["50, Entschuldigung, 15 Euro.", "50 nein 15"] {
        assert!(
            matches!(decide(&OriginalTranscript::new(input.into())?,Language::German),ProcessingDecision::Refine(signals) if signals.contains(&RefinementSignal::ExplicitCorrection)),
            "{input}"
        );
    }
    Ok(())
}
