use dictation_core::refinement::{Language, OriginalTranscript, validate};
use text_refinement_worker::punctuation::Plan;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn decoded_punctuation_remains_bound_to_words_and_original_anchors() -> TestResult {
    let source = OriginalTranscript::new("wir danken dir vielen Dank".into())?;
    let plan = Plan::new(source.clone()).map_err(|e| format!("{e:?}"))?;
    let edits = plan
        .decode(
            r#"{"text":"Wir danken dir. Vielen Dank."}"#,
            Language::German,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        validate(&source, &edits, Language::German)?.as_str(),
        "Wir danken dir. Vielen Dank."
    );
    for output in [
        r#"{"text":"Wir danken euch."}"#,
        r#"{"text":"Wir danken dir.","extra":true}"#,
        r#"{"text":"x","text":"y"}"#,
    ] {
        assert!(plan.decode(output, Language::German).is_err());
    }
    Ok(())
}
#[test]
fn pattern_preserves_existing_lines_and_escapes_literal_syntax() -> TestResult {
    let plan = Plan::new(OriginalTranscript::new("iPhone a+b\n\nnext.".into())?)
        .map_err(|e| format!("{e:?}"))?;
    let schema = plan.schema();
    let pattern = schema["properties"]["text"]["pattern"]
        .as_str()
        .ok_or("pattern")?;
    assert!(pattern.contains("iPhone"));
    assert!(!pattern.contains("IPhone"));
    assert!(pattern.contains("a\\+b"));
    assert!(pattern.contains("\n\n"));
    assert!(!pattern.contains("*"));
    Ok(())
}

#[test]
fn grammar_and_decode_reject_oversized_inputs_before_processing() -> TestResult {
    use text_refinement_protocol::FailureKind;
    let original = OriginalTranscript::new("a ".repeat(8193))?;
    assert!(matches!(Plan::new(original), Err(FailureKind::InputLimit)));
    let plan =
        Plan::new(OriginalTranscript::new("Hello.".into())?).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        plan.decode(&" ".repeat(65537), Language::English),
        Err(FailureKind::InputLimit)
    );
    Ok(())
}

#[test]
fn existing_comma_does_not_authorize_a_new_sentence_capital() -> TestResult {
    let plan =
        Plan::new(OriginalTranscript::new("hello, world".into())?).map_err(|e| format!("{e:?}"))?;
    let schema = plan.schema();
    let pattern = schema["properties"]["text"]["pattern"]
        .as_str()
        .ok_or("pattern")?;
    assert!(pattern.contains(" world"));
    assert!(!pattern.contains("World"));
    assert!(
        plan.decode(r#"{"text":"Hello, World."}"#, Language::English)
            .is_err()
    );
    Ok(())
}
