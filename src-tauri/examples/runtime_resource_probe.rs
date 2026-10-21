//! Explicit developer-only lifecycle/resource workload; no resource qualification.
#![forbid(unsafe_code)]
#[allow(dead_code, reason = "reuse bounded corpus input helpers")]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "runtime_resource_probe/metrics.rs"]
mod metrics;
#[path = "runtime_resource_probe/parameters.rs"]
mod parameters;
#[path = "runtime_resource_probe/runner.rs"]
mod runner;
#[allow(
    dead_code,
    reason = "reuse bounded readiness and cleanup without another join implementation"
)]
#[path = "paced_runtime_probe/runner.rs"]
mod shared;

use dictation_core::audio::AudioFormat;
use everlast_dictation::{
    model::{REFERENCE, SMALL},
    runtime::measurement::MeasuredRuntime,
    transcription::Engine,
};
use std::{
    io::{Cursor, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    time::Instant,
};
type Error = Box<dyn std::error::Error>;

/// Same-directory private tempfile, fsync, atomic no-replace publication.
fn write_report(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path)?;
    Ok(())
}
fn main() -> Result<(), Error> {
    let args = std::env::args_os().skip(1).take(7).collect::<Vec<_>>();
    if args.len() == 1 && args[0] == "--print-parameters" {
        println!(
            "{}",
            serde_json::to_string_pretty(&parameters::template()?)?
        );
        return Ok(());
    }
    #[cfg(feature = "mlx")]
    if args.len() == 1 && args[0] == "--print-mlx-reference-encoder-parameters" {
        println!(
            "{}",
            serde_json::to_string_pretty(&parameters::mlx_template()?)?
        );
        return Ok(());
    }
    if args.len() != 6
        || args
            .iter()
            .any(|arg| arg.is_empty() || arg.as_encoded_bytes().len() > 4096)
    {
        return Err("usage: runtime_resource_probe MODEL reference|small|mlx-reference-encoder WAV REFERENCE_UTF8 PARAMETERS_JSON NEW_OUTPUT_JSON".into());
    }
    let definition = match args[1].to_str() {
        Some("reference") => REFERENCE,
        Some("small") => SMALL,
        #[cfg(feature = "mlx")]
        Some("mlx-reference-encoder") => everlast_dictation::model::MLX_REFERENCE_COEFFICIENTS,
        _ => {
            return Err(
                "model selection must be reference, small or feature-enabled mlx-reference-encoder"
                    .into(),
            );
        }
    };
    let output = Path::new(&args[5]);
    if output.try_exists()? {
        return Err("output must be a new path".into());
    }
    let parameter_bytes = input::bounded_read(Path::new(&args[4]), 1024 * 1024)?;
    let parameter_document = parameters::select(&parameter_bytes, definition)?;
    let wav_bytes = input::bounded_read(Path::new(&args[2]), input::MAX_AUDIO)?;
    let wav = hound::WavReader::new(Cursor::new(&wav_bytes))?;
    let audio = input::Audio {
        path: "explicit-local-fixture.wav".into(),
        sha256: input::digest(&wav_bytes),
        frames: wav.duration(),
        sample_rate: wav.spec().sample_rate,
    };
    let samples: std::sync::Arc<[f32]> = input::decode_audio(&wav_bytes, &audio)?.into();
    if samples.len() < 320 {
        return Err("resource fixture must contain at least one full20ms chunk".into());
    }
    let reference_bytes = input::bounded_read(Path::new(&args[3]), 256 * 1024)?;
    let reference = std::str::from_utf8(&reference_bytes)?;
    if reference.trim().is_empty() {
        return Err("reference must contain nonempty UTF8 text".into());
    }
    let config = runner::Config::production();
    let load_started = Instant::now();
    let initialized = (|| -> Result<_, String> {
        let model = std::path::PathBuf::from(&args[0]);
        let (service, control) = {
            #[cfg(feature = "mlx")]
            if definition.is_mlx() {
                runner::controlled_service(
                    move || {
                        everlast_dictation::transcription::mlx::MlxEngine::load(&model, definition)
                    },
                    std::sync::Arc::clone(&samples),
                    runner::WAIT,
                )
            } else {
                runner::controlled_service(
                    move || Engine::load(&model, definition),
                    std::sync::Arc::clone(&samples),
                    runner::WAIT,
                )
            }
            #[cfg(not(feature = "mlx"))]
            runner::controlled_service(
                move || Engine::load(&model, definition),
                std::sync::Arc::clone(&samples),
                runner::WAIT,
            )
        }
        .map_err(|e| e.to_string())?;
        let runtime = MeasuredRuntime::spawn(
            service,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )
        .map_err(|e| e.to_string())?;
        Ok((runtime, control))
    })();
    let load_micros = load_started.elapsed().as_micros();
    let mut checkpoints = Vec::new();
    let mut baseline_rss = None;
    let mut metric_errors = 0;
    let (workload, cleanup, load_error, decoder_after_cleanup) = match initialized {
        Ok((runtime, control)) => {
            let ready = shared::wait_ready(&runtime);
            let workload = match ready {
                Ok(()) => {
                    runner::workload(&runtime, &control, &samples, reference, config, |point| {
                        let observation = metrics::observe(std::process::id(), point.include_vmmap);
                        if observation.rss_error.is_some() || observation.vmmap_error.is_some() {
                            metric_errors += 1;
                        }
                        if checkpoints.is_empty() {
                            baseline_rss = observation.rss_bytes;
                        }
                        let delta = observation
                            .rss_bytes
                            .zip(baseline_rss)
                            .map(|(now, first)| i128::from(now) - i128::from(first));
                        eprintln!(
                            "checkpoint {} status={} rssBytes={:?} rssError={:?} vmmapError={:?}",
                            point.after_id,
                            point.status,
                            observation.rss_bytes,
                            observation.rss_error,
                            observation.vmmap_error
                        );
                        checkpoints.push(serde_json::json!({"checkpoint":point,"metrics":observation,"rssDeltaFromWarmupBytes":delta}));
                    })
                }
                Err(error) => runner::unavailable(error, config),
            };
            let cleanup = shared::cleanup(runtime);
            let diagnostics = control.diagnostics();
            (workload, Some(cleanup), None, Some(diagnostics))
        }
        Err(error) => (
            runner::unavailable(error.clone(), config),
            None,
            Some(error),
            None,
        ),
    };
    let passed = workload.behavior_passed
        && cleanup.as_ref().is_some_and(|c| c.joined)
        && decoder_after_cleanup.as_ref().is_some_and(|d| {
            d.as_ref().is_ok_and(|d| {
                !d.active
                    && !d.armed
                    && d.started == d.completed
                    && d.mismatches == 0
                    && d.last_error.is_none()
            })
        });
    let metadata = serde_json::json!({
        "schemaVersion":1,"kind":"runtime-lifecycle-resource-workload","behaviorPassed":passed,"resourceQualified":false,"setupTimingPassed":workload.setup_timing_passed,
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "parameters":parameter_document,"parameterDocumentSha256":input::digest(&parameter_bytes),
        "model":{"id":definition.id(),"sha256":definition.sha256(),"bytes":definition.bytes()},
        "fixture":{"sha256":audio.sha256,"frames":samples.len(),"sampleRate":16000,"channels":1},
        "retainedInputPayloadBytes":{"encodedWav":wav_bytes.len(),"normalizedPcm":samples.len()*std::mem::size_of::<f32>(),"referenceUtf8":reference_bytes.len(),"parameterUtf8":parameter_bytes.len()},
        "referenceSha256":input::digest(&reference_bytes),"referenceComparison":"exact UTF8, no trimming; prior baseline repeatability only",
        "host":{"architecture":std::env::consts::ARCH,"os":std::env::consts::OS,"processId":std::process::id(),"debugAssertions":cfg!(debug_assertions),"availableParallelism":std::thread::available_parallelism().ok().map(|n|n.get())},
        "decoderAfterCleanup":decoder_after_cleanup.as_ref().and_then(|d|d.as_ref().ok()),
        "decoderDiagnosticsError":decoder_after_cleanup.as_ref().and_then(|d|d.as_ref().err()),
        "setupTimingScope":"feed publication summaries only, no Stop ingress budget or L qualification",
        "loadMicros":load_micros,"loadError":load_error,"workload":workload,"checkpoints":checkpoints,"metricsErrorCount":metric_errors,"cleanup":cleanup,
        "limits":["developer replay only: no microphone, GUI, paste or bundle rebuild",
            "wrapper entry barrier precedes native decode; no native-kernel interruption or strict cancellation latency claim",
            "cancelled native decode may finish; next actor StartACK confirms previous audio/inference ownership is free",
            "checkpoints follow ControllerIdle and wrapper idle; job-local allocations may survive to the worker loop boundary",
            "last measured case is StopComplete; adopted completion and explicit join provide final owner evidence",
            "RSS observations and diagnostic deltas are not live-heap proof, leak qualification or an arbitrary threshold pass",
            "pacing evidence retained as summaries, not L raw data; setupTimingPassed is independent of behaviorPassed",
            "one shared original fixture Arc and encoded input/reference/parameter bytes stay resident; payload sizes exclude allocator/header overhead; no per-cycle expected-audio copies",
            "exact full-fixture decoder input is checked for StopComplete and CancelDecoderEntry; CancelCapture has no decoder input/sample-equality claim",
            "synchronous Engine initialization and OS calls have no hard real-time preemption"]
    });
    write_report(output, &serde_json::to_vec_pretty(&metadata)?)?;
    println!("runtime resource report written; behaviorPassed={passed}; resourceQualified=false");
    if !passed {
        return Err("runtime resource workload failed; inspect private text-free report".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_publication_is_private_and_cannot_replace_a_frozen_report() -> Result<(), Error> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.json");
        write_report(&path, b"{\"behaviorPassed\":false}")?;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        assert!(write_report(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&path)?, b"{\"behaviorPassed\":false}");
        Ok(())
    }
}
