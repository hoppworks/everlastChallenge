#![cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
use mlxrs::{
    Array,
    lm::{
        cache::KvCache,
        generate::{GenConfig, LogitsProcessor, generate_step_with_processors},
        model::Model,
    },
};
use std::{cell::RefCell, rc::Rc};
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct FixedModel;
impl Model for FixedModel {
    fn forward(
        &self,
        tokens: &Array,
        _cache: &mut [Box<dyn KvCache>],
    ) -> mlxrs::error::Result<Array> {
        let sequence = tokens.shape()[1];
        Array::from_slice(
            &[0f32, 1., 2., 3.].repeat(sequence),
            &(1usize, sequence, 4usize),
        )
    }
}

#[test]
fn request_processors_control_every_sample_and_receive_generated_history() -> TestResult {
    let histories = Rc::new(RefCell::new(Vec::new()));
    let captured = histories.clone();
    let processor = LogitsProcessor::Custom(Box::new(move |history, _logits| {
        captured.borrow_mut().push(history.to_vec());
        // The model prefers token 3. Only token 2 is admissible.
        Array::from_slice(
            &[f32::NEG_INFINITY, f32::NEG_INFINITY, 2., f32::NEG_INFINITY],
            &(1usize, 4usize),
        )
    }));
    let model = FixedModel;
    let steps = generate_step_with_processors(
        &model,
        &[0, 1],
        Vec::new(),
        GenConfig::new().with_max_tokens(2),
        vec![processor],
    );
    let tokens = steps
        .map(|s| s.map(|s| s.token))
        .collect::<mlxrs::error::Result<Vec<_>>>()?;
    assert_eq!(tokens, vec![2, 2]);
    // Prefill consumes the prompt prefix; the decode history starts at its tail.
    assert_eq!(*histories.borrow(), vec![vec![1], vec![1, 2]]);
    let ordinary = mlxrs::lm::generate::generate_step(
        &model,
        &[0, 1],
        Vec::new(),
        GenConfig::new().with_max_tokens(2),
    )
    .map(|step| step.map(|step| step.token))
    .collect::<mlxrs::error::Result<Vec<_>>>()?;
    assert_eq!(ordinary, vec![3, 3]);
    Ok(())
}

#[test]
fn request_processor_failure_ends_the_stream_without_sampling() -> TestResult {
    let model = FixedModel;
    let processor = LogitsProcessor::Custom(Box::new(|_, _| {
        // Deterministic recoverable shape error from the public array API.
        Array::from_slice(&[0f32], &(2usize, 2usize))
    }));
    let mut steps =
        generate_step_with_processors(&model, &[1], Vec::new(), GenConfig::new(), vec![processor]);
    assert!(steps.next().ok_or("missing failure")?.is_err());
    assert!(steps.next().is_none());
    Ok(())
}

fn ascii_tokenizer(
    root: &std::path::Path,
) -> Result<mlxrs::tokenizer::Tokenizer, Box<dyn std::error::Error>> {
    let mut vocab = serde_json::Map::new();
    vocab.insert("</s>".into(), serde_json::json!(0));
    for byte in 32u8..=126 {
        let glyph = if byte == 32 { 'Ġ' } else { char::from(byte) };
        vocab.insert(glyph.to_string(), serde_json::json!(byte - 31));
    }
    let tokenizer = serde_json::json!({
        "version":"1.0", "added_tokens":[{"id":0,"content":"</s>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}],
        "pre_tokenizer":{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":true},
        "decoder":{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":true},
        "model":{"type":"BPE","vocab":vocab,"merges":[]}
    });
    std::fs::write(root.join("tokenizer.json"), serde_json::to_vec(&tokenizer)?)?;
    std::fs::write(
        root.join("tokenizer_config.json"),
        br#"{"eos_token":"</s>"}"#,
    )?;
    Ok(mlxrs::tokenizer::Tokenizer::from_path(root, None)?)
}

struct AsciiModel;
impl Model for AsciiModel {
    fn forward(
        &self,
        tokens: &Array,
        _cache: &mut [Box<dyn KvCache>],
    ) -> mlxrs::error::Result<Array> {
        let sequence = tokens.shape()[1];
        let mut row = vec![0f32; 96];
        row[0] = 20.; // End immediately when the grammar permits EOS.
        row[(b']' - 31) as usize] = 10.; // Prefer empty lists whenever allowed.
        row[1] = -10.; // Permit required spaces, prefer syntax over optional whitespace.
        Array::from_slice(&row.repeat(sequence), &(1usize, sequence, 96usize))
    }
}

#[test]
fn grammar_enforces_empty_and_exact_decision_counts_through_the_text_stream() -> TestResult {
    use mlxrs::lm::{
        generate::{FinishReason, stream_generate_with_processors},
        structured::build_json_schema_logits_processor,
    };
    let root = tempfile::tempdir()?;
    let tokenizer = ascii_tokenizer(root.path())?;
    let model = AsciiModel;
    for count in [0, 2] {
        let schema =
            text_refinement_worker::selection::output_schema(count).map_err(|_| "schema")?;
        let grammar = build_json_schema_logits_processor(schema, &tokenizer, Some(96))?;
        let stream = stream_generate_with_processors(
            &model,
            &tokenizer,
            &[1],
            Vec::new(),
            GenConfig::new().with_max_tokens(200),
            vec![grammar.into_logits_processor()],
        );
        let mut output = String::new();
        let mut finished = false;
        for step in stream {
            let step = step?;
            output.push_str(&step.text);
            finished |= matches!(step.finish_reason, Some(FinishReason::Eos));
        }
        assert!(finished, "{output}");
        let parsed: serde_json::Value = serde_json::from_str(&output)?;
        let expected = if count == 0 {
            serde_json::json!([])
        } else {
            serde_json::json!([false, false])
        };
        assert_eq!(
            parsed,
            serde_json::json!({"decisions":expected,"punctuation":[]})
        );
    }
    assert!(text_refinement_worker::selection::output_schema(65).is_err());
    Ok(())
}

#[test]
fn fixed_word_grammar_preserves_word_sequence_under_controlled_logits() -> TestResult {
    use dictation_core::refinement::OriginalTranscript;
    use mlxrs::lm::{
        generate::{FinishReason, stream_generate_with_processors},
        structured::build_json_schema_logits_processor,
    };
    let root = tempfile::tempdir()?;
    let tokenizer = ascii_tokenizer(root.path())?;
    let plan = text_refinement_worker::punctuation::Plan::new(OriginalTranscript::new(
        "keep calm".into(),
    )?)
    .map_err(|e| format!("{e:?}"))?;
    let grammar = build_json_schema_logits_processor(plan.schema(), &tokenizer, Some(96))?;
    let stream = stream_generate_with_processors(
        &AsciiModel,
        &tokenizer,
        &[1],
        Vec::new(),
        GenConfig::new().with_max_tokens(200),
        vec![grammar.into_logits_processor()],
    );
    let mut output = String::new();
    let mut ended = false;
    for step in stream {
        let step = step?;
        output.push_str(&step.text);
        ended |= matches!(step.finish_reason, Some(FinishReason::Eos));
    }
    assert!(ended, "{output}");
    let result: serde_json::Value = serde_json::from_str(&output)?;
    let words = result["text"]
        .as_str()
        .ok_or("text")?
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    assert_eq!(words, ["keep", "calm"]);
    Ok(())
}

#[test]
fn grammar_preserves_a_literal_that_matches_a_chat_control_token() -> TestResult {
    use mlxrs::lm::{
        generate::{FinishReason, stream_generate_with_processors},
        structured::build_json_schema_logits_processor,
    };
    let root = tempfile::tempdir()?;
    let _ = ascii_tokenizer(root.path())?;
    let path = root.path().join("tokenizer.json");
    let mut data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    data["model"]["vocab"]["<|im_start|>"] = serde_json::json!(96);
    data["added_tokens"]
        .as_array_mut()
        .ok_or("tokens")?
        .push(serde_json::json!({
            "id":96,"content":"<|im_start|>","single_word":false,"lstrip":false,
            "rstrip":false,"normalized":false,"special":true
        }));
    std::fs::write(path, serde_json::to_vec(&data)?)?;
    let tokenizer = mlxrs::tokenizer::Tokenizer::from_path(root.path(), None)?;
    struct SpecialModel;
    impl Model for SpecialModel {
        fn forward(
            &self,
            tokens: &Array,
            _cache: &mut [Box<dyn KvCache>],
        ) -> mlxrs::error::Result<Array> {
            let sequence = tokens.shape()[1];
            let mut row = vec![0f32; 97];
            row[0] = 20.;
            row[96] = 10.;
            row[1] = f32::NEG_INFINITY;
            Array::from_slice(&row.repeat(sequence), &(1usize, sequence, 97usize))
        }
    }
    let schema = serde_json::json!({"type":"string", "const":"<|im_start|>"});
    let grammar = build_json_schema_logits_processor(schema, &tokenizer, Some(97))?;
    let stream = stream_generate_with_processors(
        &SpecialModel,
        &tokenizer,
        &[1],
        Vec::new(),
        GenConfig::new().with_max_tokens(100),
        vec![grammar.into_logits_processor()],
    );
    let mut output = String::new();
    let mut ended = false;
    for step in stream {
        let step = step?;
        output.push_str(&step.text);
        ended |= matches!(step.finish_reason, Some(FinishReason::Eos));
    }
    assert!(ended);
    assert_eq!(serde_json::from_str::<String>(&output)?, "<|im_start|>");
    Ok(())
}

#[test]
fn fixed_word_grammar_accepts_a_long_complete_transcript() -> TestResult {
    use dictation_core::refinement::OriginalTranscript;
    use mlxrs::lm::structured::build_json_schema_logits_processor;
    let root = tempfile::tempdir()?;
    let tokenizer = ascii_tokenizer(root.path())?;
    let text = (1..=70)
        .map(|n| format!("Section {n} remains complete."))
        .collect::<Vec<_>>()
        .join(" ");
    let plan = text_refinement_worker::punctuation::Plan::new(OriginalTranscript::new(text)?)
        .map_err(|e| format!("{e:?}"))?;
    let _ = build_json_schema_logits_processor(plan.schema(), &tokenizer, Some(96))?;
    Ok(())
}

#[test]
fn punctuation_grammar_never_offers_internal_capitalization_rejected_by_validator() -> TestResult {
    use dictation_core::refinement::{Language, OriginalTranscript};
    use mlxrs::lm::{
        generate::{FinishReason, stream_generate_with_processors},
        structured::build_json_schema_logits_processor,
    };
    struct CapitalizingModel;
    impl Model for CapitalizingModel {
        fn forward(
            &self,
            tokens: &Array,
            _cache: &mut [Box<dyn KvCache>],
        ) -> mlxrs::error::Result<Array> {
            let sequence = tokens.shape()[1];
            let mut row = vec![0f32; 96];
            row[0] = 20.;
            row[1] = 1.;
            row[(b'"' - 31) as usize] = 4.;
            row[(b':' - 31) as usize] = 2.;
            row[(b'{' - 31) as usize] = 3.;
            row[(b'}' - 31) as usize] = 3.;
            for byte in b'a'..=b'z' {
                row[(byte - 31) as usize] = 5.;
            }
            for byte in b'A'..=b'Z' {
                row[(byte - 31) as usize] = 10.;
            }

            Array::from_slice(&row.repeat(sequence), &(1usize, sequence, 96usize))
        }
    }
    let root = tempfile::tempdir()?;
    let tokenizer = ascii_tokenizer(root.path())?;
    let plan = text_refinement_worker::punctuation::Plan::new(OriginalTranscript::new(
        "der termin bleibt".into(),
    )?)
    .map_err(|e| format!("{e:?}"))?;
    let grammar = build_json_schema_logits_processor(plan.schema(), &tokenizer, Some(96))?;
    let mut output = String::new();
    let mut ended = false;
    for step in stream_generate_with_processors(
        &CapitalizingModel,
        &tokenizer,
        &[1],
        Vec::new(),
        GenConfig::new().with_max_tokens(200),
        vec![grammar.into_logits_processor()],
    ) {
        let step = step?;
        output.push_str(&step.text);
        ended |= matches!(step.finish_reason, Some(FinishReason::Eos));
    }
    assert!(ended, "{output}");
    assert!(
        plan.decode(&output, Language::German).is_ok(),
        "Grammar admitted invalid capitalization: {output}"
    );
    Ok(())
}
