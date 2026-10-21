//! Developer L measurement: one warm context, one warmup, six by thirty runs.
//! Freeze with --print-parameters; then MODEL reference|small SIX_CASE_MANIFEST
//! PARAMETERS_JSON NEW_OUTPUT_JSON. Example manifest: tests/fixtures/corpus/
//! central-reference-v1.json. No microphone, GUI, native delivery or P05 pass.
#![forbid(unsafe_code)]
#[path = "paced_reference_probe/batch.rs"]
mod batch;
#[path = "paced_reference_probe/host.rs"]
mod host;
#[allow(dead_code, reason = "reuse bounded corpus input helpers")]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "paced_reference_probe/manifest.rs"]
mod manifest;
#[path = "paced_reference_probe/options.rs"]
mod options;
#[path = "paced_reference_probe/parameters.rs"]
mod parameters;
#[path = "paced_reference_probe/report.rs"]
mod report;
#[path = "paced_runtime_probe/runner.rs"]
mod runner;
#[path = "paced_runtime_probe/statistics.rs"]
mod statistics;
use dictation_core::audio::AudioFormat;
use everlast_dictation::transcription::Language;
use std::{path::Path, sync::Arc, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).take(6).collect::<Vec<_>>();
    if args.len() == 1
        && let Some(flag) = args[0].to_str()
        && let Some(value) = parameters::print_template(flag)?
    {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    let options = options::Options::parse(&args)?;
    report::check_new(&options.output)?;
    let parameter_bytes = input::bounded_read(&options.parameters, 1024 * 1024)?;
    let (parameter_document, execution_mode) = parameters::select(&parameter_bytes)?;
    execution_mode.validate_model(options.definition)?;
    let manifest_bytes = input::bounded_read(&options.manifest, input::MAX_MANIFEST)?;
    let cases = manifest::parse(&manifest_bytes)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    // Verify actual immutable input bytes before warming the model. Keep only
    // the six bounded normalized PCM vectors (960,000 samples in total).
    let samples = cases
        .iter()
        .map(|case| {
            input::load_audio(root, &case.audio)
                .map(Arc::new)
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>();
    let host = host::observe();
    let mut runtime = None;
    let load_started = Instant::now();
    let mut failure = samples.as_ref().err().cloned();
    let mut receipts = None;
    let variant = execution_mode.context_reduced().map(|reduced| {
        if reduced {
            runner::audio_context::Variant::Reduced
        } else {
            runner::audio_context::Variant::Reference
        }
    });
    if failure.is_none() {
        let loaded = (|| {
            #[cfg(feature = "mlx")]
            if execution_mode == parameters::ExecutionMode::MlxReferenceEncoder {
                return parameters::load_mlx_service(options.model, options.definition)
                    .map(|service| (service, None))
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>);
            }
            if execution_mode.full_prefix().is_some() {
                everlast_dictation::inference_service::InferenceService::load_prefix_cache(
                    options.model,
                    options.definition,
                )
                .map(|service| (service, None))
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            } else {
                runner::audio_context::load(options.model, options.definition, variant)
            }
        })();
        failure = match loaded {
            Ok((service, context_receipts)) => {
                receipts = context_receipts;
                match execution_mode.spawn_runtime(
                    service,
                    1,
                    AudioFormat {
                        sample_rate: 16000,
                        channels: 1,
                    },
                ) {
                    Ok(measured) => {
                        let result = runner::wait_ready(&measured);
                        runtime = Some(measured);
                        result.err()
                    }
                    Err(error) => Some(error.to_string()),
                }
            }
            Err(error) => Some(error.to_string()),
        };
    }
    let load_millis = load_started.elapsed().as_millis();
    let mut rows = batch::run(&cases, failure.as_deref(), |slot| {
        eprintln!(
            "paced reference slot {} fixture={} run={} warmup={}",
            slot.id, slot.fixture_id, slot.run_index, slot.warmup
        );
        match (&runtime, &samples) {
            (Some(runtime), Ok(samples)) => {
                let language = if slot.language == "de" {
                    Language::German
                } else {
                    Language::English
                };
                if let Some(candidate) = execution_mode.full_prefix() {
                    runner::one_full_prefix(
                        runtime,
                        &samples[slot.fixture_index],
                        language,
                        slot.id.clone(),
                        slot.warmup,
                        None,
                        runner::ExpectedOutcome::Local,
                        if candidate {
                            runner::full_prefix::Variant::Candidate
                        } else {
                            runner::full_prefix::Variant::Reference
                        },
                    )
                } else if execution_mode.pretranscription().is_some() {
                    runner::one_pretranscription(
                        runtime,
                        &samples[slot.fixture_index],
                        language,
                        slot.id.clone(),
                        slot.warmup,
                        None,
                        runner::ExpectedOutcome::Local,
                    )
                } else if let Some(receipts) = &receipts {
                    runner::one_context(
                        runtime,
                        &samples[slot.fixture_index],
                        language,
                        slot.id.clone(),
                        slot.warmup,
                        runner::ContextRun {
                            receipts,
                            reference: None,
                            outcome: runner::ExpectedOutcome::Local,
                        },
                    )
                } else if execution_mode.scheduled() {
                    runner::one_scheduled(
                        runtime,
                        &samples[slot.fixture_index],
                        language,
                        slot.id.clone(),
                        slot.warmup,
                        None,
                        runner::ExpectedOutcome::Local,
                    )
                } else {
                    runner::one(
                        runtime,
                        &samples[slot.fixture_index],
                        language,
                        slot.id.clone(),
                        slot.warmup,
                        None,
                    )
                }
            }
            _ => runner::unavailable(
                slot.id.clone(),
                slot.warmup,
                "runtime or verified inputs unavailable".into(),
            ),
        }
    });
    let trace_reader = runtime.as_ref().map(|runtime| runtime.job_trace_handle());
    let cleanup = runtime.map(runner::cleanup);
    let (job_trace_after_cleanup, job_trace_after_cleanup_error) =
        match trace_reader.map(|reader| reader.snapshot()).transpose() {
            Ok(trace) => {
                let error = trace.as_ref().and_then(|trace| trace.error.clone());
                (trace.filter(|trace| trace.enabled), error)
            }
            Err(error) => (None, Some(error.to_string())),
        };
    let context_cleanup = receipts
        .as_ref()
        .map(runner::audio_context::Receipts::finalize);
    for error in context_cleanup
        .as_ref()
        .and_then(|c| c.error.as_ref())
        .into_iter()
        .chain(job_trace_after_cleanup_error.as_ref())
    {
        failure.get_or_insert(error.clone());
        if let Some(last) = rows.last_mut() {
            if last.output.observation.status == "ok" {
                last.output.observation.status = "failed";
            }
            last.output.observation.error = Some(match last.output.observation.error.take() {
                Some(previous) => format!("{previous}; {error}"),
                None => error.clone(),
            });
        }
    }
    // Do not form statistics while an old inference/model owner could remain.
    let summary = batch::aggregate(
        &cases,
        &rows,
        cleanup.as_ref().is_some_and(|value| value.joined),
    );
    let complete = summary.is_some();
    let hypotheses=rows.iter().map(|row|serde_json::json!({"slot":row.slot,"status":row.output.observation.status,"text":row.output.text})).collect::<Vec<_>>();
    let observations = rows
        .iter()
        .map(|row| serde_json::json!({"slot":row.slot,"observation":row.output.observation}))
        .collect::<Vec<_>>();
    let fixtures=cases.iter().map(|case|serde_json::json!({"id":case.id,"language":case.language,"class":"clean","split":"acceptance","durationSeconds":case.seconds,
        "audioSha256":case.audio.sha256,"inputFrames":case.audio.frames,"sampleRate":case.audio.sample_rate,"referenceUtf8Sha256":input::digest(case.reference.as_bytes())})).collect::<Vec<_>>();
    let metadata = serde_json::json!({
        "schemaVersion":1,"processId":std::process::id(),"kind":if execution_mode.full_prefix().is_some(){"paced-six-case-full-prefix-experiment"}else if execution_mode.pretranscription().is_some(){"paced-six-case-pretranscription-experiment"}else{"paced-six-case-complete-recording-reference"},"measurementComplete":complete,
        "qualified":false,"p05Qualified":false,"qualityEvaluated":false,
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "debugAssertions":cfg!(debug_assertions),"hostObservation":host,
        "manifestSha256":input::digest(&manifest_bytes),"parameterDocumentSha256":input::digest(&parameter_bytes),"parameters":parameter_document,"contextCleanup":context_cleanup,"pretranscriptionCandidate":execution_mode.pretranscription(),"fullPrefixCandidate":execution_mode.full_prefix(),"jobTraceAfterCleanup":job_trace_after_cleanup,"jobTraceAfterCleanupError":job_trace_after_cleanup_error,
        "backend":if options.definition.is_reference_encoder(){"mlxrs-encoder+whisper.cpp-decoder"}else{"whisper.cpp"},"model":{"id":options.definition.id(),"sha256":options.definition.sha256()},
        "engineRtfDefinition":if execution_mode.full_prefix().is_some(){Some("sum actual scoped decoder elapsed / (unique final recorded N/16000); includes preparation/hash/hit work; processed overlapping frames separate; not native kernel time or stop-to-text")}else{None},
        "warmupRuns":1,"measuredRunsPerCase":batch::RUNS_PER_CASE,"requestedRows":batch::TOTAL_RUNS,"fixtures":fixtures,
        "loadMillis":load_millis,"initialFailure":failure,"runs":observations,"cleanup":cleanup,"statistics":summary,
        "hypothesesSha256":input::digest(&serde_json::to_vec_pretty(&hypotheses)?),
        "limits":["six fixed clean acceptance fixtures; published references and hypotheses require separate WER/original-content/anchor review",
            "no byte-exact published-reference requirement is imposed by the timing harness",
            "all181 rows must succeed and join must be confirmed before any six-case L/statistic appears",
            "each file is published atomically0600 without replacement; the two-file pair is not a transaction",
            "no microphone/hotkey/native delivery, resource or fullP05 qualification; debug timings cannot establish release performance"]
    });
    report::write(
        &options.output,
        &serde_json::to_vec_pretty(&hypotheses)?,
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    println!("paced reference report written; measurementComplete={complete}; qualified=false");
    if !complete {
        return Err("six-case measurement incomplete; every failure/unexecuted slot retained in private report".into());
    }
    Ok(())
}
