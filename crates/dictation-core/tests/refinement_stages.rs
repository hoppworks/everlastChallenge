use dictation_core::refinement::{
    EditKind, Language, OriginalTranscript, ProposedEdit, StagedEdits, ValidationError, validate,
    validate_staged,
};
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
fn punctuation_of_a_corrected_value_keeps_authority_bound_to_raw_dictation() -> TestResult {
    let original = OriginalTranscript::new("Thursday, no Tuesday".into())?;
    let structural = edit(EditKind::SelfCorrection, "Thursday, no Tuesday", "Tuesday");
    let punctuation = edit(EditKind::PunctuationCase, "Tuesday", "Tuesday.");
    assert!(
        validate(
            &original,
            &[structural.clone(), punctuation.clone()],
            Language::English
        )
        .is_err()
    );
    let accepted = validate_staged(
        &original,
        &StagedEdits {
            structural: vec![structural],
            punctuation: vec![punctuation],
        },
        Language::English,
    )?;
    assert_eq!(accepted.as_str(), "Tuesday.");
    assert!(accepted.original() == &original);
    assert_eq!(accepted.language(), Language::English);
    assert_eq!(accepted.edit_count(), 2);
    Ok(())
}
#[test]
fn later_stage_failure_never_returns_a_partially_cleaned_transcript() -> TestResult {
    let original = OriginalTranscript::new("Um, send 15 euros.".into())?;
    let result = validate_staged(
        &original,
        &StagedEdits {
            structural: vec![edit(EditKind::Filler, "Um,", "")],
            punctuation: vec![edit(EditKind::PunctuationCase, "15", "50")],
        },
        Language::English,
    );
    assert!(matches!(result, Err(ValidationError::InvalidEdit)));
    assert_eq!(original.as_str(), "Um, send 15 euros.");
    Ok(())
}
#[test]
fn stage_roles_and_total_budget_cannot_be_bypassed() -> TestResult {
    let original = OriginalTranscript::new("Um, hello.".into())?;
    for proposal in [
        StagedEdits {
            structural: vec![edit(EditKind::PunctuationCase, "hello", "Hello")],
            punctuation: vec![],
        },
        StagedEdits {
            structural: vec![],
            punctuation: vec![edit(EditKind::Filler, "Um,", "")],
        },
    ] {
        assert!(matches!(
            validate_staged(&original, &proposal, Language::English),
            Err(ValidationError::InvalidEdit)
        ));
    }
    let over_budget = StagedEdits {
        structural: vec![edit(EditKind::Filler, "Um,", ""); 32],
        punctuation: vec![edit(EditKind::PunctuationCase, "hello", "Hello")],
    };
    assert!(matches!(
        validate_staged(&original, &over_budget, Language::English),
        Err(ValidationError::Limit)
    ));
    Ok(())
}
#[test]
fn punctuation_is_anchored_to_the_exact_intermediate_text() -> TestResult {
    let original = OriginalTranscript::new("Um, hello.".into())?;
    let accepted = validate_staged(
        &original,
        &StagedEdits {
            structural: vec![edit(EditKind::Filler, "Um,", "")],
            punctuation: vec![edit(EditKind::PunctuationCase, "hello", "Hello")],
        },
        Language::English,
    )?;
    assert_eq!(accepted.as_str(), "Hello.");
    let stale = StagedEdits {
        structural: vec![edit(EditKind::Filler, "Um,", "")],
        punctuation: vec![edit(EditKind::PunctuationCase, "Um, hello.", "Um, Hello.")],
    };
    assert!(validate_staged(&original, &stale, Language::English).is_err());
    Ok(())
}

#[test]
fn protected_values_and_negation_remain_guarded_after_a_valid_correction() -> TestResult {
    let original = OriginalTranscript::new("It is 50, sorry 15 euros, not 5.".into())?;
    for (before, after) in [("15", "16"), ("not 5", "5"), ("5", "05")] {
        let proposal = StagedEdits {
            structural: vec![edit(EditKind::SelfCorrection, "50, sorry 15", "15")],
            punctuation: vec![edit(EditKind::PunctuationCase, before, after)],
        };
        assert!(validate_staged(&original, &proposal, Language::English).is_err());
    }
    let unchanged = validate_staged(&original, &StagedEdits::default(), Language::English)?;
    assert_eq!(unchanged.as_str(), original.as_str());
    assert_eq!(unchanged.edit_count(), 0);
    Ok(())
}

#[test]
fn staged_wire_shape_rejects_unknown_and_missing_fields() {
    for json in [
        r#"{"structural":[],"punctuation":[],"text":"trusted"}"#,
        r#"{"structural":[]}"#,
        r#"{"punctuation":[]}"#,
    ] {
        assert!(serde_json::from_str::<StagedEdits>(json).is_err());
    }
}
