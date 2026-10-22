use dictation_core::refinement::{Language, OriginalTranscript, proposal_candidates, validate};
use text_refinement_worker::selection::decode;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn complete_decisions_resolve_to_exact_original_spans() -> TestResult {
    let original = OriginalTranscript::new("Um, send, uh, the report.".into())?;
    let choices = proposal_candidates(&original, Language::English)?;
    let edits = decode(
        r#"{"decisions":[true,true,false],"punctuation":[]}"#,
        &original,
        &choices,
        Language::English,
    )
    .map_err(|_| "selection failed")?;
    assert_eq!(
        validate(&original, &edits, Language::English)?.as_str(),
        "send the report."
    );
    Ok(())
}

#[test]
fn invalid_selection_or_punctuation_rejects_the_whole_result() -> TestResult {
    let original = OriginalTranscript::new("Um, send, uh, the report.".into())?;
    let choices = proposal_candidates(&original, Language::English)?;
    for output in [
        r#"{"decisions":[true],"punctuation":[]}"#,
        r#"{"decisions":[true,true,false,false],"punctuation":[]}"#,
        r#"{"decisions":[true,true,true],"punctuation":[]}"#,
        r#"{"decisions":[true,false,false],"punctuation":[{"before":"report","after":"secret"}]}"#,
        r#"{"decisions":[true,false,false],"decisions":[],"punctuation":[]}"#,
        r#"{"decisions":[],"punctuation":[],"extra":true}"#,
    ] {
        assert!(
            decode(output, &original, &choices, Language::English).is_err(),
            "{output}"
        );
    }
    assert_eq!(original.as_str(), "Um, send, uh, the report.");
    Ok(())
}

#[test]
fn prompt_options_keep_anchors_private_and_identify_repeated_source_occurrences() -> TestResult {
    let original = OriginalTranscript::new("We, uh, begin and, uh, finish.".into())?;
    let choices = proposal_candidates(&original, Language::English)?;
    let options = text_refinement_worker::selection::prompt_options(&original, &choices)
        .map_err(|_| "options failed")?;
    assert_eq!(options.len(), 4);
    assert_eq!(options[0]["occurrence"], 1);
    assert_eq!(options[2]["occurrence"], 2);
    for option in options {
        assert!(option.get("leftContext").is_none());
        assert!(option.get("rightContext").is_none());
    }
    Ok(())
}

#[test]
fn occurrence_numbers_include_overlapping_source_matches() -> TestResult {
    use dictation_core::refinement::{EditKind, ProposedEdit};
    let original = OriginalTranscript::new("la la la".into())?;
    let choices = [
        ProposedEdit {
            kind: EditKind::PunctuationCase,
            before: "la la".into(),
            after: "la la.".into(),
            left_context: Some("".into()),
            right_context: Some(" la".into()),
        },
        ProposedEdit {
            kind: EditKind::PunctuationCase,
            before: "la la".into(),
            after: "la la.".into(),
            left_context: Some("la ".into()),
            right_context: Some("".into()),
        },
    ];
    for choice in &choices {
        validate(&original, std::slice::from_ref(choice), Language::English)?;
    }
    let options = text_refinement_worker::selection::prompt_options(&original, &choices)
        .map_err(|_| "options failed")?;
    assert_eq!(options[0]["occurrence"], 1);
    assert_eq!(options[1]["occurrence"], 2);
    Ok(())
}
