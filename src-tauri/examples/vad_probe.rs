//! Numeric developer experiment only. Never classifies an input as safe silence.
#![forbid(unsafe_code)]

#[allow(
    dead_code,
    reason = "reuse the audited corpus input boundary, not its paired report writer"
)]
#[path = "corpus_probe/input.rs"]
mod input;

use serde::Serialize;
use std::{io::Write, path::Path, time::Instant};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams};

type Error = Box<dyn std::error::Error>;
const MODEL_BYTES: u64 = 885_098;
const MODEL_SHA256: &str = "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf";
const MANIFEST_SHA256: &str = "514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49";
const CASE_COUNT: usize = 42;
const WINDOW_SAMPLES: usize = 512;
const THRESHOLD: f32 = 0.5;
const THREADS: i32 = 1;
const PEAK_TARGET: f64 = 0.95;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Signal {
    frames: usize,
    duration_millis: f64,
    rms: f64,
    peak: f32,
    expected_windows: usize,
    last_window_real_samples: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Scores {
    max_probability: f32,
    positive_window_count: usize,
    probabilities: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    id: String,
    language: String,
    audio_sha256: String,
    variant: &'static str,
    status: &'static str,
    assessment: &'static str,
    gain: Option<f64>,
    input_signal: Option<Signal>,
    output_signal: Option<Signal>,
    scores: Option<Scores>,
    input_micros: u128,
    prepare_micros: Option<u128>,
    detect_micros: Option<u128>,
    error: Option<&'static str>,
}

struct Prepared {
    gain: f64,
    input_signal: Signal,
    output_signal: Signal,
}

fn prepare(samples: &mut [f32], normalized: bool) -> Result<Prepared, Error> {
    let input_signal = signal(samples)?;
    let gain = if normalized && input_signal.peak > 0.0 {
        PEAK_TARGET / f64::from(input_signal.peak)
    } else {
        1.0
    };
    if normalized && input_signal.peak > 0.0 {
        for sample in samples.iter_mut() {
            *sample = (f64::from(*sample) * gain) as f32;
        }
    }
    let output_signal = if normalized {
        signal(samples)?
    } else {
        input_signal.clone()
    };
    Ok(Prepared {
        gain,
        input_signal,
        output_signal,
    })
}

fn signal(samples: &[f32]) -> Result<Signal, Error> {
    if samples.is_empty()
        || samples.len() > 4_800_000
        || samples
            .iter()
            .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
    {
        return Err("invalid bounded normalized PCM".into());
    }
    let squared: f64 = samples.iter().map(|v| f64::from(*v).powi(2)).sum();
    Ok(Signal {
        frames: samples.len(),
        duration_millis: samples.len() as f64 / 16.0,
        rms: (squared / samples.len() as f64).sqrt(),
        peak: samples.iter().map(|v| v.abs()).fold(0.0, f32::max),
        expected_windows: samples.len().div_ceil(WINDOW_SAMPLES),
        last_window_real_samples: (samples.len() - 1) % WINDOW_SAMPLES + 1,
    })
}

fn scores(probabilities: &[f32], expected: usize) -> Result<Scores, Error> {
    if expected == 0
        || probabilities.len() != expected
        || probabilities
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        return Err("invalid VAD probability output".into());
    }
    Ok(Scores {
        max_probability: probabilities.iter().copied().fold(0.0, f32::max),
        positive_window_count: probabilities.iter().filter(|p| **p >= THRESHOLD).count(),
        probabilities: probabilities.to_vec(),
    })
}

fn calibration(bytes: &[u8]) -> Result<input::Manifest, Error> {
    if input::digest(bytes) != MANIFEST_SHA256 {
        return Err("expected the frozen 42-case calibration-v2 manifest".into());
    }
    let manifest = input::parse_manifest(bytes)?;
    let declarations: serde_json::Value = serde_json::from_slice(bytes)?;
    let cases = declarations["cases"].as_array().ok_or("missing cases")?;
    if manifest.cases.len() != CASE_COUNT || cases.iter().any(|case| case["split"] != "calibration")
    {
        return Err("only all 42 calibration cases are permitted".into());
    }
    Ok(manifest)
}

fn pinned_model(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() as u64 != MODEL_BYTES || input::digest(bytes) != MODEL_SHA256 {
        return Err("VAD model size or SHA256 mismatch".into());
    }
    Ok(())
}

fn write_private(output: &Path, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("probe output exceeds 32 MiB".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    std::fs::hard_link(temporary.path(), output)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let arguments: Vec<_> = std::env::args_os().skip(1).take(5).collect();
    if !(3..=4).contains(&arguments.len())
        || arguments
            .iter()
            .any(|v| v.is_empty() || v.as_encoded_bytes().len() > 4096)
    {
        return Err(
            "usage: vad_probe MODEL_PATH CALIBRATION_V2_JSON OUTPUT_JSON [peak-normalized]".into(),
        );
    }
    let normalized = match arguments.get(3).map(|arg| arg.to_str()) {
        None => false,
        Some(Some("peak-normalized")) => true,
        _ => return Err("the only optional variant is peak-normalized".into()),
    };
    let variant = if normalized {
        "whole-recording-peak-normalized-0.95"
    } else {
        "raw-pcm16-no-gain-no-trim"
    };
    let manifest_bytes = input::bounded_read(Path::new(&arguments[1]), input::MAX_MANIFEST)?;
    let manifest = calibration(&manifest_bytes)?;
    let model_bytes = input::bounded_read(Path::new(&arguments[0]), MODEL_BYTES)?;
    pinned_model(&model_bytes)?;
    if Path::new(&arguments[2]).exists() || Path::new(&arguments[2]).is_symlink() {
        return Err("output already exists".into());
    }
    // C++ accepts a filename. Load an owned private copy of exactly the verified
    // bytes, so replacement of the original path cannot bypass the model pin.
    let mut model_copy = tempfile::NamedTempFile::new()?;
    model_copy.write_all(&model_bytes)?;
    model_copy.flush()?;
    let mut params = WhisperVadContextParams::new();
    params.set_use_gpu(false);
    params.set_n_threads(THREADS);
    let started = Instant::now();
    let mut context = WhisperVadContext::new(
        model_copy
            .path()
            .to_str()
            .ok_or("temporary model path is not UTF-8")?,
        params,
    )
    .map_err(|_| "VAD initialization failed; no silence decision")?;
    let init_micros = started.elapsed().as_micros();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    let mut observations = Vec::with_capacity(CASE_COUNT);
    let batch_started = Instant::now();
    for case in manifest.cases {
        let tick = Instant::now();
        let loaded = input::load_audio(root, &case.audio);
        let mut row = Observation {
            id: case.id,
            language: case.language,
            audio_sha256: case.audio.sha256,
            variant,
            status: "failed",
            assessment: "undecided-experiment-only",
            gain: None,
            input_signal: None,
            output_signal: None,
            scores: None,
            input_micros: tick.elapsed().as_micros(),
            prepare_micros: None,
            detect_micros: None,
            error: None,
        };
        match loaded {
            Err(_) => row.error = Some("audio_validation_failed"),
            Ok(mut samples) => {
                let tick = Instant::now();
                let prepared = prepare(&mut samples, normalized);
                row.prepare_micros = Some(tick.elapsed().as_micros());
                match prepared {
                    Err(_) => row.error = Some("invalid_pcm"),
                    Ok(prepared) => {
                        let tick = Instant::now();
                        let detected = context.detect_speech(&samples);
                        row.detect_micros = Some(tick.elapsed().as_micros());
                        match detected {
                            Err(_) => row.error = Some("vad_api_failed"),
                            Ok(()) => match scores(
                                context.probabilities(),
                                prepared.output_signal.expected_windows,
                            ) {
                                Err(_) => row.error = Some("invalid_probabilities"),
                                Ok(scores) => {
                                    row.scores = Some(scores);
                                    row.status = "api_completed_untrusted";
                                }
                            },
                        }
                        row.gain = Some(prepared.gain);
                        row.input_signal = Some(prepared.input_signal);
                        row.output_signal = Some(prepared.output_signal);
                    }
                }
            }
        }
        observations.push(row);
    }
    let batch_micros = batch_started.elapsed().as_micros();
    let tick = Instant::now();
    drop(context);
    let cleanup_micros = tick.elapsed().as_micros();
    let failed = observations
        .iter()
        .filter(|row| row.error.is_some())
        .count();
    let report = serde_json::json!({
        "schemaVersion": 2, "kind": "developer-vad-calibration-only", "productionSilenceDecision": false,
        "manifestSha256": MANIFEST_SHA256, "caseCount": CASE_COUNT,
        "sourceRevision": std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "probeSourceHash": input::digest(include_bytes!("vad_probe.rs")),
        "model": {"id": "silero-v5.1.2", "sha256": MODEL_SHA256, "bytes": MODEL_BYTES},
        "parameters": {"backend": "cpu", "threads": THREADS, "sampleRate": 16000, "windowSamples": WINDOW_SAMPLES, "positiveThreshold": THRESHOLD, "variant": variant, "peakTarget": if normalized { Some(PEAK_TARGET) } else { None }, "zeroPeakGain": 1.0, "trim": false, "clip": false},
        "limits": {"upstreamComputeFailureCanReportSuccess": true, "completeExecutionGuaranteed": false, "cancelCallback": false, "realTimeMeasurement": false, "rssMeasured": false},
        "timing": {"initMicros": init_micros, "batchMicros": batch_micros, "cleanupMicros": cleanup_micros},
        "failedCases": failed, "observations": observations,
    });
    write_private(
        Path::new(&arguments[2]),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "vad-probe: 42 numeric observations; {failed} explicit failures; no silence decisions"
    );
    if failed > 0 {
        return Err("one or more calibration cases failed; no silence decision".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_variant_handles_zero_one_sample_and_quiet_pcm_without_frame_changes()
    -> Result<(), Error> {
        let mut zero = [0.0, -0.0];
        let observed = prepare(&mut zero, true)?;
        assert_eq!(observed.gain, 1.0);
        assert_eq!(
            zero.map(f32::to_bits),
            [0.0f32.to_bits(), (-0.0f32).to_bits()]
        );
        let mut single = [-0.25];
        let observed = prepare(&mut single, true)?;
        assert_eq!(single, [-0.95]);
        assert_eq!(observed.gain, 3.8);
        assert_eq!(
            (observed.input_signal.frames, observed.output_signal.frames),
            (1, 1)
        );
        let mut quiet = [-0.25, 0.125, 0.0, 0.0625];
        let observed = prepare(&mut quiet, true)?;
        assert_eq!(quiet, [-0.95, 0.475, 0.0, 0.2375]);
        assert_eq!(observed.gain, 3.8);
        assert_eq!(
            (observed.input_signal.peak, observed.output_signal.peak),
            (0.25, 0.95)
        );
        assert_eq!(
            (observed.input_signal.frames, observed.output_signal.frames),
            (4, 4)
        );
        Ok(())
    }

    #[test]
    fn normalization_is_finite_uniform_and_raw_is_bitwise_unchanged() -> Result<(), Error> {
        let original = [-1.0, 0.5, -0.0, f32::from_bits(1)];
        let mut raw = original;
        assert_eq!(prepare(&mut raw, false)?.gain, 1.0);
        assert_eq!(raw.map(f32::to_bits), original.map(f32::to_bits));
        let mut normalized = [1.0, -0.5, 0.0];
        assert_eq!(prepare(&mut normalized, true)?.gain, 0.95);
        assert_eq!(normalized, [0.95, -0.475, 0.0]);
        let mut tiny = [f32::from_bits(1), -f32::from_bits(1)];
        let observed = prepare(&mut tiny, true)?;
        assert!(observed.gain.is_finite());
        assert_eq!(tiny, [0.95, -0.95]);
        for invalid in [f32::NAN, f32::INFINITY, 1.1] {
            assert!(prepare(&mut [invalid], true).is_err());
        }
        Ok(())
    }

    #[test]
    fn numeric_boundary_preserves_short_audio_and_the_final_partial_window() -> Result<(), Error> {
        let measured = signal(&vec![0.5; 3200])?;
        assert_eq!(
            (
                measured.frames,
                measured.expected_windows,
                measured.last_window_real_samples
            ),
            (3200, 7, 128)
        );
        assert_eq!(
            (measured.duration_millis, measured.rms, measured.peak),
            (200.0, 0.5, 0.5)
        );
        let measured = signal(&vec![0.0; 513])?;
        assert_eq!(
            (measured.expected_windows, measured.last_window_real_samples),
            (2, 1)
        );
        let summary = scores(&[0.1, 0.5, 0.9], 3)?;
        assert_eq!(summary.positive_window_count, 2);
        assert_eq!(summary.max_probability, 0.9);
        Ok(())
    }

    #[test]
    fn malformed_signal_probabilities_and_pins_never_become_silence() {
        for values in [&[][..], &[f32::NAN][..], &[f32::INFINITY][..], &[1.1][..]] {
            assert!(signal(values).is_err());
        }
        for values in [
            &[][..],
            &[f32::NAN][..],
            &[f32::INFINITY][..],
            &[-0.1][..],
            &[1.1][..],
        ] {
            assert!(scores(values, 1).is_err());
        }
        assert!(scores(&[0.1], 2).is_err());
        assert!(calibration(b"{}").is_err());
        assert!(pinned_model(&vec![0; MODEL_BYTES as usize]).is_err());
    }

    #[test]
    fn private_numeric_report_is_atomic_and_never_replaces_a_frozen_file() -> Result<(), Error> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.json");
        write_private(&path, b"{}")?;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        assert!(write_private(&path, b"different").is_err());
        assert_eq!(std::fs::read(&path)?, b"{}");
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 1);
        Ok(())
    }
}
