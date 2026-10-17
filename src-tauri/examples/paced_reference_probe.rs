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
use everlast_dictation::{
    inference_service::InferenceService, runtime::measurement::MeasuredRuntime,
    transcription::Language,
};
use std::{path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).take(6).collect::<Vec<_>>();
    if args.len() == 1 && args[0] == "--print-parameters" {
        println!(
            "{}",
            serde_json::to_string_pretty(&parameters::template()?)?
        );
        return Ok(());
    }
    let options = options::Options::parse(&args)?;
    report::check_new(&options.output)?;
    let parameter_bytes = input::bounded_read(&options.parameters, 1024 * 1024)?;
    let parameter_document = parameters::validate(&parameter_bytes)?;
    let manifest_bytes = input::bounded_read(&options.manifest, input::MAX_MANIFEST)?;
    let cases = manifest::parse(&manifest_bytes)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    // Verify actual immutable input bytes before warming the model. Keep only
    // the six bounded normalized PCM vectors (960,000 samples in total).
    let samples = cases
        .iter()
        .map(|case| input::load_audio(root, &case.audio).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>();
    let host = host::observe();
    let mut runtime = None;
    let load_started = Instant::now();
    let mut failure = samples.as_ref().err().cloned();
    if failure.is_none() {
        failure = match InferenceService::load(options.model, options.definition) {
            Ok(service) => match MeasuredRuntime::spawn(
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
            },
            Err(error) => Some(error.to_string()),
        };
    }
    let load_millis = load_started.elapsed().as_millis();
    let rows = batch::run(&cases, failure.as_deref(), |slot| {
        eprintln!(
            "paced reference slot {} fixture={} run={} warmup={}",
            slot.id, slot.fixture_id, slot.run_index, slot.warmup
        );
        match (&runtime, &samples) {
            (Some(runtime), Ok(samples)) => runner::one(
                runtime,
                &samples[slot.fixture_index],
                if slot.language == "de" {
                    Language::German
                } else {
                    Language::English
                },
                slot.id.clone(),
                slot.warmup,
                None,
            ),
            _ => runner::unavailable(
                slot.id.clone(),
                slot.warmup,
                "runtime or verified inputs unavailable".into(),
            ),
        }
    });
    let cleanup = runtime.map(runner::cleanup);
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
        "schemaVersion":1,"kind":"paced-six-case-complete-recording-reference","measurementComplete":complete,
        "qualified":false,"p05Qualified":false,"qualityEvaluated":false,
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "debugAssertions":cfg!(debug_assertions),"hostObservation":host,
        "manifestSha256":input::digest(&manifest_bytes),"parameterDocumentSha256":input::digest(&parameter_bytes),"parameters":parameter_document,
        "model":{"id":options.definition.id(),"sha256":options.definition.sha256()},
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
