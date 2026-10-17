//! Numeric SDK diagnosis only; never a transcription/quality or real-time gate.
#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn singleton_row_softmax_is_stable_and_excludes_other_rows() -> Result<(), Error> {
        let observed = probability(&[1000.0, 1001.0, 999.0], 1)?;
        assert!((observed - 0.6652409558).abs() < 1e-9);
        assert_eq!(probability(&[0.0, 0.0], 0)?, 0.5);
        Ok(())
    }
    #[test]
    fn malformed_or_nonfinite_rows_are_rejected() {
        for row in [
            &[][..],
            &[f32::NAN][..],
            &[f32::INFINITY][..],
            &[f32::NEG_INFINITY][..],
        ] {
            assert!(probability(row, 0).is_err());
        }
        assert!(probability(&[0.0], 1).is_err());
    }
    #[test]
    fn tiny_probabilities_still_distinguish_sot_from_uniform_or_last_rows() {
        let expected = 5.0854216642746395e-11;
        assert!(matches_oracle(&[5.085398768756022e-11], expected));
        assert!(!matches_oracle(&[0.00001928], expected));
        assert!(!matches_oracle(&[4.1993266413770185e-8], expected));
    }
}

#[allow(
    dead_code,
    reason = "shared input module also exports the corpus-only paired report writer"
)]
#[path = "corpus_probe/input.rs"]
mod input;
use dictation_core::audio::{EnergyProfile, speech_range};
use everlast_dictation::model::{REFERENCE, SMALL, verify_file};
use serde::Serialize;
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};
type Error = Box<dyn std::error::Error>;
const IDS: [&str; 2] = ["fleurs-de-dev-1510", "calibration-de-room-5s"];
const ABSOLUTE_TOLERANCE: f64 = 1e-12;
const RELATIVE_TOLERANCE: f64 = 0.001;

fn probability(row: &[f32], token: usize) -> Result<f64, Error> {
    if row.is_empty() || token >= row.len() || row.iter().any(|v| !v.is_finite()) {
        return Err("invalid finite singleton logit row".into());
    }
    let maximum = row.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64;
    let denominator: f64 = row.iter().map(|v| (f64::from(*v) - maximum).exp()).sum();
    Ok((f64::from(row[token]) - maximum).exp() / denominator)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    id: String,
    variant: &'static str,
    state_run: &'static str,
    source_audio_sha256: String,
    source_frames: usize,
    selected_start: usize,
    selected_end: usize,
    backend_frames: usize,
    explicit_prefix_tokens: Vec<i32>,
    oracle_sot: f64,
    oracle_last_initial_row: f64,
    actual_segments: Vec<f64>,
    max_absolute_error: Option<f64>,
    allowed_error: f64,
    distinguishes_sot_from_last: bool,
    passed: bool,
}

// C++ initial prompt: optional [PREV, explicit tokens], then SOT, language,
// TRANSCRIBE, NOTIMESTAMPS. Single-token decodes expose exactly one n_vocab row
// through safe get_logits(), independently of full()'s batch-row bookkeeping.
fn oracle(context: &WhisperContext, audio: &[f32], prefix: &[i32]) -> Result<(f64, f64), Error> {
    let mut state = context.create_state().map_err(|_| "oracle state failed")?;
    state
        .pcm_to_mel(audio, 4)
        .map_err(|_| "oracle mel failed")?;
    state.encode(0, 4).map_err(|_| "oracle encode failed")?;
    let mut past = 0;
    if !prefix.is_empty() {
        let mut history = vec![context.token_prev()];
        history.extend_from_slice(prefix);
        state
            .decode(&history, 0, 4)
            .map_err(|_| "oracle prefix failed")?;
        past = history.len();
    }
    state
        .decode(&[context.token_sot()], past, 4)
        .map_err(|_| "oracle SOT failed")?;
    let token = usize::try_from(context.token_nosp())?;
    let sot = probability(
        state.get_logits().map_err(|_| "oracle logits failed")?,
        token,
    )?;
    past += 1;
    for token in [
        context.token_lang(whisper_rs::get_lang_id("de").ok_or("language unavailable")?),
        context.token_transcribe(),
        context.token_not(),
    ] {
        state
            .decode(&[token], past, 4)
            .map_err(|_| "oracle final prompt failed")?;
        past += 1;
    }
    let last = probability(
        state
            .get_logits()
            .map_err(|_| "oracle final logits failed")?,
        token,
    )?;
    Ok((sot, last))
}
fn run_full(state: &mut WhisperState, audio: &[f32], prefix: &[i32]) -> Result<Vec<f64>, Error> {
    let mut parameters = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    parameters.set_n_threads(4);
    parameters.set_language(Some("de"));
    parameters.set_translate(false);
    parameters.set_detect_language(false);
    // C++ clears rolling history BEFORE inserting explicit prompt_tokens.
    parameters.set_no_context(true);
    parameters.set_tokens(prefix);
    parameters.set_no_timestamps(true);
    parameters.set_print_special(false);
    parameters.set_print_progress(false);
    parameters.set_print_realtime(false);
    parameters.set_print_timestamps(false);
    parameters.set_suppress_nst(true);
    parameters.set_no_speech_thold(1.0);
    parameters.set_logprob_thold(-1.0);
    parameters.set_entropy_thold(2.4);
    parameters.set_temperature(0.0);
    parameters.set_temperature_inc(0.0);
    state
        .full(parameters, audio)
        .map_err(|_| "full decode failed")?;
    Ok(state
        .as_iter()
        .map(|segment| f64::from(segment.no_speech_probability()))
        .collect())
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().skip(1).take(6).collect();
    if !(4..=5).contains(&args.len())
        || args
            .iter()
            .any(|a| a.is_empty() || a.as_encoded_bytes().len() > 4096)
    {
        return Err(
            "usage: no_speech_regression MODEL_PATH reference|small MANIFEST_JSON OUTPUT_JSON [cpu]"
                .into(),
        );
    }
    let use_gpu = match args.get(4).map(|value| value.to_str()) {
        None => true,
        Some(Some("cpu")) => false,
        _ => return Err("optional backend must be cpu".into()),
    };
    let definition = match args[1].to_str() {
        Some("reference") => REFERENCE,
        Some("small") => SMALL,
        _ => return Err("invalid pinned model selection".into()),
    };
    let bytes = input::bounded_read(Path::new(&args[2]), input::MAX_MANIFEST)?;
    let manifest = input::parse_manifest(&bytes)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository unavailable")?;
    // Validate both fixed inputs before any model work; never silently substitute.
    let mut cases = Vec::new();
    for id in IDS {
        let case = manifest
            .cases
            .iter()
            .find(|case| case.id == id)
            .ok_or("required fixed case missing")?;
        if case.language != "de" || case.audio.frames > 30 * 16000 {
            return Err("fixed case language/duration invalid".into());
        }
        let samples = input::load_audio(root, &case.audio)?;
        let range = speech_range(&samples, EnergyProfile::default())
            .ok_or("fixed case has no energy range")?;
        let mut selected = samples[range.clone()].to_vec();
        selected.resize(selected.len().max(16000), 0.0);
        cases.push((case, samples.len(), range, selected));
    }
    verify_file(Path::new(&args[0]), definition).map_err(|_| "pinned model verification failed")?;
    whisper_rs::install_logging_hooks();
    let context = WhisperContext::new_with_params(
        Path::new(&args[0]).to_str().ok_or("model path invalid")?,
        WhisperContextParameters {
            use_gpu,
            flash_attn: false,
            ..Default::default()
        },
    )
    .map_err(|_| "model initialization failed")?;
    if !context.is_multilingual() {
        return Err("multilingual model required".into());
    }
    // Deliberately contains an earlier SOT, proving the active row cannot be
    // selected by searching for the first SOT token in the complete prompt.
    let mut prefix = context
        .tokenize("Früherer Kontext.", 64)
        .map_err(|_| "prefix tokenization failed")?;
    prefix.insert(0, context.token_sot());
    if prefix.len() + 1 >= (context.n_text_ctx() / 2) as usize {
        return Err("prefix exceeds unchanged context budget".into());
    }
    let mut rows = Vec::new();
    for (case, source_frames, range, audio) in cases {
        for (variant, explicit) in [
            ("without-prefix", &[][..]),
            ("earlier-sot-prefix", prefix.as_slice()),
        ] {
            let mut state = context.create_state().map_err(|_| "full state failed")?;
            for state_run in ["fresh", "reused"] {
                let (sot, last) = oracle(&context, &audio, explicit)?;
                let actual = run_full(&mut state, &audio, explicit)?;
                let error = maximum_error(&actual, sot);
                let matched = matches_oracle(&actual, sot);
                rows.push(Observation {
                    id: case.id.clone(),
                    variant,
                    state_run,
                    source_audio_sha256: case.audio.sha256.clone(),
                    source_frames,
                    selected_start: range.start,
                    selected_end: range.end,
                    backend_frames: audio.len(),
                    explicit_prefix_tokens: explicit.to_vec(),
                    oracle_sot: sot,
                    oracle_last_initial_row: last,
                    actual_segments: actual,
                    max_absolute_error: error,
                    allowed_error: allowed_error(sot),
                    distinguishes_sot_from_last: (sot - last).abs() > 10.0 * allowed_error(sot),
                    passed: matched,
                });
            }
        }
    }
    let passed = rows.iter().all(|row| row.passed)
        && ["without-prefix", "earlier-sot-prefix"]
            .iter()
            .all(|variant| {
                rows.iter()
                    .any(|row| row.variant == *variant && row.distinguishes_sot_from_last)
            });
    let report = serde_json::json!({
        "schemaVersion": 1, "diagnostic": "singleton-SOT-versus-full-segment-NSP", "manifestSha256":input::digest(&bytes),
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "model":{"id":definition.id(),"sha256":definition.sha256()},
        "absoluteTolerance":ABSOLUTE_TOLERANCE,"relativeTolerance":RELATIVE_TOLERANCE,"passed":passed,"observations":rows,
        "parameters":{"threads":4,"greedyBestOf":1,"useGpu":use_gpu,"flashAttention":false,"language":"de","translate":false,"detectLanguage":false,"noContext":true,"noTimestamps":true,"suppressNonSpeechTokens":true,"noSpeechThreshold":1.0,"logprobThreshold":-1.0,"entropyThreshold":2.4,"temperature":0.0,"temperatureIncrement":0.0,"energyProfile":"production-default","minimumBackendSamples":16000},
        "scope":{"automaticRollingHistoryTested":false,"oracleAlwaysFresh":true,"sameFullStateRuns":2,"explicitPrefixReinsertedAfterHistoryClear":true,"emptySegmentListFails":true,"diagnosticOverrides":["noSpeechThreshold=1","temperatureIncrement=0"],"rawTranscriptExported":false}
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(Path::new(&args[3]))?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    if !passed {
        return Err(
            "SDK regression mismatch or no observable segment; inspect numeric report".into(),
        );
    }
    Ok(())
}
fn allowed_error(expected: f64) -> f64 {
    ABSOLUTE_TOLERANCE + RELATIVE_TOLERANCE * expected.abs()
}
fn matches_oracle(actual: &[f64], expected: f64) -> bool {
    maximum_error(actual, expected).is_some_and(|error| error <= allowed_error(expected))
}
fn maximum_error(actual: &[f64], expected: f64) -> Option<f64> {
    if actual.is_empty()
        || !expected.is_finite()
        || actual
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return None;
    }
    Some(
        actual
            .iter()
            .map(|v| (v - expected).abs())
            .fold(0.0, f64::max),
    )
}
