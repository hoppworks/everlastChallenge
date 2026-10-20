use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    samples: Vec<f32>,
    mels: usize,
    padding: usize,
    shape: Vec<usize>,
    expected: Vec<f32>,
    maximum_absolute_error: f32,
}

#[test]
fn whisper_mel_matches_independent_periodic_dft_goldens() -> Result<(), String> {
    std::thread::spawn(|| {
        let result = (|| {
            let oracle: Oracle = serde_json::from_str(include_str!(
                "../../../../tests/fixtures/mlx/whisper-mel-oracle-v1.json"
            ))
            .map_err(|e| e.to_string())?;
            for case in oracle.cases {
                let length = i32::try_from(case.samples.len()).map_err(|e| e.to_string())?;
                let wave =
                    Array::from_slice(&case.samples, &[length]).map_err(|e| e.to_string())?;
                let mut mel = log_mel_spectrogram_whisper(&wave, case.mels, case.padding)
                    .map_err(|e| e.to_string())?;
                if mel.shape() != case.shape {
                    return Err(format!("{}: wrong mel shape", case.name));
                }
                let actual = mel.to_vec::<f32>().map_err(|e| e.to_string())?;
                if actual.len() != case.expected.len() {
                    return Err(format!("{}: wrong mel element count", case.name));
                }
                let error = actual
                    .iter()
                    .zip(&case.expected)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f32, f32::max);
                if actual.iter().any(|value| !value.is_finite())
                    || error > case.maximum_absolute_error
                {
                    return Err(format!(
                        "{}: independent periodic DFT max error {error} exceeds {}",
                        case.name, case.maximum_absolute_error
                    ));
                }
            }
            // Public generic DSP retains symmetric Hann and its own inverse.
            let mut window = mlxrs::audio::dsp::hann_window(4).map_err(|e| e.to_string())?;
            let values = window.to_vec::<f32>().map_err(|e| e.to_string())?;
            if values
                .iter()
                .zip([0.0_f32, 0.75, 0.75, 0.0])
                .any(|(a, b)| (a - b).abs() > 0.000001)
            {
                return Err("generic symmetric Hann contract changed".into());
            }
            let original = [
                0.2_f32, -0.4, 0.7, 0.1, -0.3, 0.9, 0.4, -0.8, 0.6, 0.5, -0.1, 0.3, -0.7, 0.8, 0.0,
                -0.2,
            ];
            let wave = Array::from_slice(&original, &[16]).map_err(|e| e.to_string())?;
            let spectrum =
                mlxrs::audio::dsp::stft(&wave, 8, 2, None, mlxrs::audio::dsp::WindowPad::Right)
                    .map_err(|e| e.to_string())?;
            let mut restored =
                mlxrs::audio::dsp::istft(&spectrum, Some(16)).map_err(|e| e.to_string())?;
            let values = restored.to_vec::<f32>().map_err(|e| e.to_string())?;
            if values.len() != original.len()
                || values
                    .iter()
                    .zip(original)
                    .any(|(a, b)| (a - b).abs() > 0.00001)
            {
                return Err("generic symmetric STFT/ISTFT roundtrip changed".into());
            }
            Ok(())
        })();
        // All arrays leave scope before the thread's final MLX operation.
        let cleanup = cleanup_streams().map_err(|e| e.to_string());
        result.and(cleanup)
    })
    .join()
    .map_err(|_| "Whisper mel oracle worker panicked".to_owned())?
}

pub(super) fn tokenizer_fixture() -> Result<Tokenizer, Box<dyn std::error::Error>> {
    use mlxrs::audio::stt::models::whisper::tokenizer::LANGUAGES;
    use serde_json::json;
    let directory = tempfile::tempdir()?;
    let mut tokens = vec![
        "ordinary".to_owned(),
        "<|endoftext|>".to_owned(),
        "<|startoftranscript|>".to_owned(),
        "<|translate|>".to_owned(),
        "<|transcribe|>".to_owned(),
        "<|startoflm|>".to_owned(),
        "<|startofprev|>".to_owned(),
        "<|nospeech|>".to_owned(),
        "<|notimestamps|>".to_owned(),
        "<|0.00|>".to_owned(),
    ];
    tokens.extend(LANGUAGES.iter().map(|(code, _)| format!("<|{code}|>")));
    let vocab: serde_json::Map<String, serde_json::Value> = tokens
        .iter()
        .enumerate()
        .map(|(id, token)| (token.clone(), json!(id)))
        .collect();
    let additions: Vec<_> = tokens
        .iter()
        .enumerate()
        .map(|(id, token)| {
            json!({
                "id":id,"content":token,"single_word":false,"lstrip":false,
                "rstrip":false,"normalized":false,"special":id != 0
            })
        })
        .collect();
    std::fs::write(
        directory.path().join("tokenizer.json"),
        json!({
            "version":"1.0","added_tokens":additions,"normalizer":null,
            "pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,
            "model":{"type":"WordLevel","vocab":vocab,"unk_token":"<|endoftext|>"}
        })
        .to_string(),
    )?;
    std::fs::write(
        directory.path().join("tokenizer_config.json"),
        json!({
            "eos_token":"<|endoftext|>","unk_token":"<|endoftext|>"
        })
        .to_string(),
    )?;
    Tokenizer::from_path(directory.path(), None).map_err(Into::into)
}

#[test]
fn fixed_language_decode_suppresses_language_switches_but_preserves_text_and_prompt()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = tokenizer_fixture()?;
    for language in [Language::German, Language::English] {
        let tokenizer =
            HFTokenizerWrapper::new(&raw, true, 100, Some(language.code()), Task::Transcribe)?;
        let options = decode_options(&tokenizer, language, None, false);
        let suppressed = decoding::get_suppress_tokens(&tokenizer, &options.suppress_tokens)?;
        // The fixture places the complete language vocabulary at IDs10..110.
        // Pinned whisper.cpp unconditionally excludes all language tokens.
        assert!(
            (10..110).all(|id| suppressed.contains(&id)),
            "fixed-language output may still sample language-switch specials"
        );
        assert!(
            !suppressed.contains(&0),
            "ordinary speech must remain generatable"
        );
        assert!(
            !suppressed.contains(&1),
            "end-of-text remains available after the first step"
        );
        assert_eq!(
            tokenizer.sot_sequence(),
            vec![2, if language == Language::German { 12 } else { 10 }, 4]
        );
        assert_eq!(options.language.as_deref(), Some(language.code()));
    }
    Ok(())
}

#[cfg(feature = "measurement")]
#[test]
fn f16_measurement_loader_rejects_missing_reference_assets()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let result = MlxEngine::load_reference_encoder_f16_for_measurement(
        &directory
            .path()
            .join(crate::model::MLX_REFERENCE_COEFFICIENTS.file_name()),
    );
    assert!(
        result.is_err(),
        "diagnosis must not load without pinned source assets"
    );
    Ok(())
}
