//! Public decoder completion contract: reaching a token limit is not EOS.
use super::*;
use mlxrs::error::Error;

type Result<T> = std::result::Result<T, String>;

fn run_fixture(emit_eot: bool) -> Result<std::result::Result<decoding::DecodingResult, Error>> {
    let tokenizer = tests::tokenizer_fixture().map_err(|e| e.to_string())?;
    let wrapper = HFTokenizerWrapper::new(&tokenizer, false, 1, Some("en"), Task::Transcribe)
        .map_err(|e| e.to_string())?;
    let dims =
        ModelDimensions::new(4, 1500, 4, 2, 1, 128, 6, 4, 2, 1).map_err(|e| e.to_string())?;
    let mut weights = coefficient_tests::weights(&[1500, 4])?;
    let mut embedding = vec![0.0_f32; 128 * 4];
    if emit_eot {
        // EOS is ID1. A constant final LN bias makes its tied projection the
        // unique highest score. Initial suppress_blank still emits ordinary ID0
        // first, then the next step is EOS. No fitted/random model is used.
        embedding[4] = 1.0;
        weights.insert(
            "decoder.ln.bias".into(),
            Array::from_slice(&[1.0_f32, 0.0, 0.0, 0.0], &[4]).map_err(|e| e.to_string())?,
        );
    }
    weights.insert(
        "decoder.token_embedding.weight".into(),
        Array::from_slice(&embedding, &[128, 4]).map_err(|e| e.to_string())?,
    );
    let model = WhisperModel::from_weights(dims, weights, Dtype::F32).map_err(|e| e.to_string())?;
    let backend = model.backend(false, None);
    let options = DecodingOptions {
        without_timestamps: true,
        sample_len: Some(3),
        suppress_tokens: SuppressSpec::Ids(Vec::new()),
        language: Some("en".into()),
        ..DecodingOptions::default()
    };
    let task =
        decoding::DecodingTask::new(&backend, &wrapper, options).map_err(|e| e.to_string())?;
    let features = Array::zeros::<f32>(&(1500_usize, 4_usize)).map_err(|e| e.to_string())?;
    Ok(task.run(&features, "en"))
}

#[test]
fn text_only_decode_rejects_token_budget_without_eot() -> Result<()> {
    std::thread::spawn(|| {
        let result = (|| match run_fixture(false)? {
            Err(Error::CapExceeded(detail))
                if detail.context() == "Whisper text-only completion" =>
            {
                Ok(())
            }
            Err(error) => Err(format!(
                "wrong failure for exhausted text-only decode: {error}"
            )),
            Ok(result) => Err(format!(
                "incomplete text-only decode succeeded: {:?}",
                result.tokens
            )),
        })();
        result.and(cleanup_streams().map_err(|e| e.to_string()))
    })
    .join()
    .map_err(|_| "text-only completion worker panicked".to_owned())?
}

#[test]
fn text_only_decode_accepts_a_real_eot_before_the_budget() -> Result<()> {
    std::thread::spawn(|| {
        let result = (|| {
            let result = run_fixture(true)?.map_err(|e| e.to_string())?;
            if result.tokens != [0] || result.text != "ordinary" {
                return Err(format!(
                    "complete EOS fixture decoded wrong tokens: {:?}",
                    result.tokens
                ));
            }
            Ok(())
        })();
        result.and(cleanup_streams().map_err(|e| e.to_string()))
    })
    .join()
    .map_err(|_| "complete EOS worker panicked".to_owned())?
}

#[test]
fn incomplete_text_only_attempt_retries_the_whole_input_with_native_timestamps() -> Result<()> {
    std::thread::spawn(|| {
        let result = (|| {
            let raw = tests::tokenizer_fixture().map_err(|e| e.to_string())?;
            let tokenizer = HFTokenizerWrapper::new(&raw, false, 1, Some("en"), Task::Transcribe)
                .map_err(|e| e.to_string())?;
            let dims = ModelDimensions::new(4, 1500, 4, 2, 1, 128, 6, 4, 2, 1)
                .map_err(|e| e.to_string())?;
            let mut weights = coefficient_tests::weights(&[1500, 4])?;
            let mut embedding = vec![0.0_f32; 128 * 4];
            // Ordinary0 and no-timestamps8 form an unending text-only path.
            // Native SOT2→timestamp9→EOS1 completes with no text. All graph
            // weights are zero except these tied embedding rows and LN scales.
            for (id, value) in [(0, 10.0), (8, 10.0), (1, -10.0), (2, -10.0), (9, -10.0)] {
                embedding[id * 4] = value;
            }
            weights.insert(
                "decoder.token_embedding.weight".into(),
                Array::from_slice(&embedding, &[128, 4]).map_err(|e| e.to_string())?,
            );
            let model =
                WhisperModel::from_weights(dims, weights, Dtype::F32).map_err(|e| e.to_string())?;
            let backend = model.backend(false, None);
            let mel = Array::zeros::<f32>(&(3000_usize, 4_usize)).map_err(|e| e.to_string())?;
            let options = TranscribeOptions {
                decode: DecodingOptions {
                    without_timestamps: true,
                    sample_len: Some(3),
                    suppress_tokens: SuppressSpec::Ids(Vec::new()),
                    language: Some("en".into()),
                    ..DecodingOptions::default()
                },
                temperatures: vec![0.0],
                condition_on_previous_text: false,
                ..TranscribeOptions::default()
            };
            let complete = complete_decode::transcribe(&backend, &tokenizer, &mel, 100, &options)
                .map_err(|e| e.to_string())?;
            if !complete.native_timestamp_retry
                || complete.without_timestamps
                || !complete.result.text.trim().is_empty()
            {
                return Err(format!(
                    "incomplete initial text was not replaced by a full native retry: {:?}",
                    complete.result.text
                ));
            }
            Ok(())
        })();
        result.and(cleanup_streams().map_err(|e| e.to_string()))
    })
    .join()
    .map_err(|_| "whole-input retry worker panicked".to_owned())?
}
