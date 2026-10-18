//! Explicit developer measurement of paced stop-to-text through the real runtime.
#![forbid(unsafe_code)]
#[allow(
    dead_code,
    reason = "reuse the bounded, hash-verified corpus input and private export seam"
)]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "paced_runtime_probe/options.rs"]
mod options;
#[path = "paced_runtime_probe/parameters.rs"]
mod parameters;
#[path = "paced_runtime_probe/runner.rs"]
mod runner;
#[path = "paced_runtime_probe/statistics.rs"]
mod statistics;

use dictation_core::audio::AudioFormat;
use everlast_dictation::runtime::measurement::MeasuredRuntime;
use std::{io::Cursor, path::Path, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).take(9).collect::<Vec<_>>();
    if arguments.len() == 1
        && matches!(
            arguments[0].to_str(),
            Some(
                "--print-parameters"
                    | "--print-scheduling-parameters"
                    | "--print-context-reference-parameters"
                    | "--print-context-candidate-parameters"
            )
        )
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&if arguments[0] == "--print-scheduling-parameters" {
                parameters::scheduling_template()?
            } else if arguments[0] == "--print-context-reference-parameters" {
                parameters::context_template(false)?
            } else if arguments[0] == "--print-context-candidate-parameters" {
                parameters::context_template(true)?
            } else {
                parameters::template()?
            })?
        );
        return Ok(());
    }
    let options = options::Options::parse(&arguments)?;
    let parameter_bytes = input::bounded_read(&options.parameters, 1024 * 1024)?;
    let (parameter_document, execution_mode) = parameters::select(&parameter_bytes)?;
    let bytes = input::bounded_read(&options.wav, input::MAX_AUDIO)?;
    let wav = hound::WavReader::new(Cursor::new(&bytes))?;
    let declared = input::Audio {
        path: "explicit-local-fixture.wav".into(),
        sha256: input::digest(&bytes),
        frames: wav.duration(),
        sample_rate: wav.spec().sample_rate,
    };
    let samples = Arc::new(input::decode_audio(&bytes, &declared)?);
    let reference_bytes = options
        .reference
        .as_deref()
        .map(|path| input::bounded_read(path, 256 * 1024))
        .transpose()?;
    let reference = reference_bytes
        .as_ref()
        .map(|data| std::str::from_utf8(data))
        .transpose()?;
    if reference.is_some_and(|text| text.trim().is_empty()) {
        return Err("reference must contain non-whitespace UTF-8 text".into());
    }
    let host = host_observation();
    let load_started = Instant::now();
    let mut runtime = None;
    let variant = execution_mode.context_reduced().map(|reduced| {
        if reduced {
            runner::audio_context::Variant::Reduced
        } else {
            runner::audio_context::Variant::Reference
        }
    });
    let mut receipts = None;
    let mut unavailable =
        match runner::audio_context::load(options.model, options.definition, variant) {
            Ok((service, context_receipts)) => {
                receipts = context_receipts;
                match MeasuredRuntime::spawn(
                    service,
                    1,
                    AudioFormat {
                        sample_rate: 16_000,
                        channels: 1,
                    },
                ) {
                    Ok(measured) => {
                        let ready = runner::wait_ready(&measured);
                        runtime = Some(measured);
                        ready.err()
                    }
                    Err(error) => Some(error.to_string()),
                }
            }
            Err(error) => Some(error.to_string()),
        };
    let load_millis = load_started.elapsed().as_millis();
    let load_error = unavailable.clone();
    let mut results = Vec::new();
    let mut observations = Vec::new();
    for index in 0..=options.runs {
        let id = if index == 0 {
            "warmup".into()
        } else {
            format!("measured-{index:03}")
        };
        let mut output = if let Some(error) = &unavailable {
            runner::unavailable(id, index == 0, error.clone())
        } else if let Some(measured) = &runtime {
            if let Some(receipts) = &receipts {
                runner::one_context(
                    measured,
                    &samples,
                    options.language,
                    id,
                    index == 0,
                    runner::ContextRun {
                        receipts,
                        reference,
                        outcome: options.expected_outcome,
                    },
                )
            } else if execution_mode.scheduled() {
                runner::one_scheduled(
                    measured,
                    &samples,
                    options.language,
                    id,
                    index == 0,
                    reference,
                    options.expected_outcome,
                )
            } else {
                match options.expected_outcome {
                    runner::ExpectedOutcome::Local => runner::one(
                        measured,
                        &samples,
                        options.language,
                        id,
                        index == 0,
                        reference,
                    ),
                    runner::ExpectedOutcome::Empty => runner::one_expected(
                        measured,
                        &samples,
                        options.language,
                        id,
                        index == 0,
                        None,
                        options.expected_outcome,
                    ),
                }
            }
        } else {
            runner::unavailable(id, index == 0, "runtime was not initialized".into())
        };
        output.observation.expected_outcome = options.expected_outcome;
        if output.observation.status != "ok" && unavailable.is_none() {
            unavailable = Some(
                output
                    .observation
                    .error
                    .clone()
                    .unwrap_or_else(|| "preceding run failed".into()),
            );
        }
        results.push(
            serde_json::json!({"id":output.observation.id,"warmup":output.observation.warmup,
            "status":output.observation.status,"text":output.text}),
        );
        observations.push(output.observation);
    }
    let latencies = observations
        .iter()
        .filter(|row| !row.warmup)
        .map(|row| {
            if row.status == "ok" {
                row.stop_to_text_nanos
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let cleanup = runtime.map(runner::cleanup);
    let context_cleanup = receipts
        .as_ref()
        .map(runner::audio_context::Receipts::finalize);
    if let Some(error) = context_cleanup.as_ref().and_then(|c| c.error.as_ref()) {
        unavailable.get_or_insert(error.clone());
        if let Some(last) = observations.last_mut()
            && last.status == "ok"
        {
            last.status = "failed";
            last.error = Some(error.clone());
            if let Some(result) = results.last_mut() {
                result["status"] = serde_json::json!("failed");
            }
        }
    }
    let passed = unavailable.is_none() && cleanup.as_ref().is_some_and(|value| value.joined);
    let summary = if passed && options.expected_outcome == runner::ExpectedOutcome::Local {
        statistics::summarize(&latencies)
    } else {
        None
    };
    let measured_rows = observations
        .iter()
        .filter(|row| !row.warmup)
        .collect::<Vec<_>>();
    let terminal_values = measured_rows
        .iter()
        .map(|row| {
            if row.status == "ok" {
                row.stop_to_terminal_nanos
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let terminal_summary = if passed {
        statistics::summarize(&terminal_values)
    } else {
        None
    };
    let engine_values = measured_rows
        .iter()
        .map(|row| {
            if row.status == "ok" {
                row.engine_elapsed_nanos
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let engine_summary = if passed {
        statistics::summarize(&engine_values)
    } else {
        None
    };
    let engine_rtf_summary =
        engine_summary
            .as_ref()
            .filter(|_| !samples.is_empty())
            .map(|summary| {
                let factor = 16000.0 / samples.len() as f64 / 1_000_000_000.0;
                serde_json::json!({"count":summary.count,"median":summary.median_nanos*factor,
            "min":summary.min_nanos as f64*factor,"max":summary.max_nanos as f64*factor,
            "range":summary.range_nanos as f64*factor})
            });
    let metadata = serde_json::json!({
        "schemaVersion":1,"kind":"paced-runtime-complete-recording","passed":passed,
        "debugAssertions":cfg!(debug_assertions),
        "processId":std::process::id(),"sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "fixtureSha256":declared.sha256,"inputFrames":samples.len(),"sampleRate":16000,"channels":1,
        "model":{"id":options.definition.id(),"sha256":options.definition.sha256()},
        "language":options.language.code(),"parameterDocumentSha256":input::digest(&parameter_bytes),
        "parameters":parameter_document,"contextCleanup":context_cleanup,"hostObservation":host,
        "referenceSha256":reference_bytes.as_ref().map(|data| input::digest(data)),
        "referenceComparison":"trimmed exact text only; provenance and independent WER are separate",
        "warmupRuns":1,"measuredRunsRequested":options.runs,"loadMillis":load_millis,"loadError":load_error,
        "runs":observations,"summary":summary,"terminalSummary":terminal_summary,
        "engineElapsedSummary":engine_summary,"engineRtfSummary":engine_rtf_summary,
        "engineTimingCoverage":{"measuredRuns":measured_rows.len(),"withActualEngineTiming":measured_rows.iter().filter(|row|row.engine_elapsed_nanos.is_some()).count()},
        "expectedOutcome":options.expected_outcome,
        "engineRtfDefinition":"actual Decoder::decode duration / (passed 16-kHz frames / 16000); production Engine includes energy/VAD/preparation/fresh ASR state; not pure ASR or stop-to-text",
        "cleanup":cleanup,
        "thirtyMeasuredRunsCompleted":options.runs==30&&passed,
        "L":null,"notAggregateL":true,"qualified":false,
        "limits":["single explicitly selected fixture; no six-case aggregate L",
            "all failures retained; no partial-case summary or retry of a failed run",
            "no microphone, hotkey, native paste, RSS/heap acceptance or 300-s endpoint qualification",
            "parameter document is frozen before model work and must equal the compiled source template"]
    });
    input::write_reports(
        Path::new(&options.output),
        &serde_json::to_vec_pretty(&results)?,
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    println!("paced runtime report written; success={passed}; no transcript printed");
    if !passed {
        return Err("paced runtime run failed; inspect complete private report".into());
    }
    Ok(())
}

fn host_observation() -> serde_json::Value {
    fn read(program: &str, args: &[&str]) -> Option<String> {
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()
            .map(|text| text.trim().to_owned())
    }
    let power = read("/usr/bin/pmset", &["-g", "batt"]).map(|text| {
        if text.contains("AC Power") {
            "AC Power"
        } else if text.contains("Battery Power") {
            "Battery Power"
        } else {
            "unknown"
        }
    });
    serde_json::json!({"architecture":std::env::consts::ARCH,
        "cpu":read("/usr/sbin/sysctl", &["-n","machdep.cpu.brand_string"]),
        "memoryBytes":read("/usr/sbin/sysctl", &["-n","hw.memsize"]).and_then(|s| s.parse::<u64>().ok()),
        "macOS":read("/usr/bin/sw_vers", &["-productVersion"]),
        "macOSBuild":read("/usr/bin/sw_vers", &["-buildVersion"]),"powerSource":power,
        "rustc":read("rustc", &["--version"]),
        "scope":"read-only observation before model initialization; not a resource measurement"})
}
