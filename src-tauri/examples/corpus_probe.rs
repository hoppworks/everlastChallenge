//! Explicit private corpus export; file-backend timing is not real-time L.
#[path = "corpus_probe/input.rs"]
mod input;
use everlast_dictation::{
    inference_service::{InferenceService, Job, WorkToken},
    model::{ModelDefinition, REFERENCE, SMALL, VAD},
    transcription::Language,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

type Error = Box<dyn std::error::Error>;
const WAIT_LIMIT: Duration = Duration::from_secs(120);
#[derive(Serialize)]
struct ResultRow {
    id: String,
    status: &'static str,
    text: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    id: String,
    status: &'static str,
    job_millis: Option<u128>,
    error: Option<String>,
}

struct Cli {
    model: PathBuf,
    definition: ModelDefinition,
    manifest: PathBuf,
    output: PathBuf,
    native_timestamps: bool,
}

fn parse_arguments(arguments: Vec<std::ffi::OsString>) -> Result<Cli, Error> {
    if !(4..=5).contains(&arguments.len()) {
        return Err(
            "usage: corpus_probe MODEL_PATH reference|small MANIFEST_JSON OUTPUT_JSON [--native-timestamps]".into(),
        );
    }
    if arguments
        .iter()
        .any(|argument| argument.is_empty() || argument.as_encoded_bytes().len() > 4096)
    {
        return Err("invalid bounded CLI arguments".into());
    }
    if arguments[..4]
        .iter()
        .any(|argument| argument.as_encoded_bytes().starts_with(b"--"))
    {
        return Err("flags are allowed only as the optional fifth argument".into());
    }
    let native_timestamps = if let Some(flag) = arguments.get(4) {
        if flag != "--native-timestamps" {
            return Err("unknown corpus probe flag".into());
        }
        if !cfg!(feature = "measurement") {
            return Err("--native-timestamps requires the measurement feature".into());
        }
        true
    } else {
        false
    };
    let definition = match arguments[1].to_str() {
        Some("reference") => REFERENCE,
        Some("small") => SMALL,
        _ => return Err("model selection must be reference or small".into()),
    };
    Ok(Cli {
        model: PathBuf::from(&arguments[0]),
        definition,
        manifest: PathBuf::from(&arguments[2]),
        output: PathBuf::from(&arguments[3]),
        native_timestamps,
    })
}

fn main() -> Result<(), Error> {
    let cli = parse_arguments(std::env::args_os().skip(1).take(6).collect())?;
    let definition = cli.definition;
    let bytes = input::bounded_read(&cli.manifest, input::MAX_MANIFEST)?;
    let corpus_hash = input::digest(&bytes);
    let manifest = input::parse_manifest(&bytes)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    let started = Instant::now();
    let mut service = None;
    let mut unavailable = match load_service(cli.model, definition, cli.native_timestamps) {
        Ok(worker) => {
            let result = wait_ready(&worker);
            service = Some(worker);
            result.err()
        }
        Err(error) => Some(error.to_string()),
    };
    let load_millis = started.elapsed().as_millis();
    let load_error = unavailable.clone();
    let mut results = Vec::with_capacity(manifest.cases.len());
    let mut observations = Vec::with_capacity(manifest.cases.len());
    for (index, case) in manifest.cases.iter().enumerate() {
        let mut job_millis = None;
        let outcome = if let Some(reason) = &unavailable {
            Err(format!("inference service unavailable: {reason}"))
        } else {
            match input::load_audio(root, &case.audio) {
                Err(error) => Err(error.to_string()),
                Ok(samples) => {
                    let worker = service.as_ref().ok_or("inference owner unavailable")?;
                    let tick = Instant::now();
                    let result = infer(
                        worker,
                        index as u64 + 1,
                        samples,
                        if case.language == "de" {
                            Language::German
                        } else {
                            Language::English
                        },
                    );
                    job_millis = Some(tick.elapsed().as_millis());
                    match result {
                        Ok(text) => Ok(text),
                        Err((reason, fatal)) => {
                            if fatal {
                                unavailable = Some(reason.clone());
                            }
                            Err(reason)
                        }
                    }
                }
            }
        };
        let (text, error) = match outcome {
            Ok(text) => (Some(text), None),
            Err(error) => (None, Some(error)),
        };
        let status = if error.is_none() { "ok" } else { "failed" };
        results.push(ResultRow {
            id: case.id.clone(),
            status,
            text,
        });
        observations.push(Observation {
            id: case.id.clone(),
            status,
            job_millis,
            error,
        });
    }
    // Observation timeouts cancel admission, but do not claim a hard backend
    // interrupt. The owned worker is always joined before exporting results.
    let cleanup_started = Instant::now();
    let cleanup_error = service
        .take()
        .and_then(|worker| worker.shutdown().err())
        .map(|error| error.to_string());
    let cleanup_millis = cleanup_started.elapsed().as_millis();
    let gate_metadata = serde_json::json!({"modelId":VAD.id(),"modelSha256":VAD.sha256(),"modelBytes":VAD.bytes(),"useGpu":false,"threads":1,
                "windowSamples":512,"mode":"whole-recording-admission; ASR PCM unchanged","peakTarget":0.95,"zeroPeakGain":1.0,
                "positiveThreshold":0.5,"aggregation":"any finite window >= threshold; all windows validated first",
                "maximumScratchSamples":4800000,"failurePolicy":"error; never silence"});
    let metadata = serde_json::json!({
        "schemaVersion": 1, "referenceKind": "file-backend-only", "corpusFileSha256": corpus_hash,
        "sourceRevision": std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "model": { "id": definition.id(), "sha256": definition.sha256() },
        "loadMillis": load_millis, "loadError": load_error, "serviceError": unavailable, "cleanupMillis": cleanup_millis, "cleanupError": cleanup_error,
        "observationTimeoutMillis": WAIT_LIMIT.as_millis(), "timeoutIsHardAbort": false,
        "nativeTimestampDiagnostic": {
            "enabled": cli.native_timestamps,
            "modelInitializationOwner": if cli.native_timestamps { "developer-main-before-serial-worker" } else { "serial-inference-worker" },
            "candidateInitializationDifference": cli.native_timestamps,
            "boundedAcceptedSegmentObservationAllocatedPerJob": cli.native_timestamps,
            "sharedInferenceServiceJobLifecycle": true,
            "resourceQualified": false, "realTimeLatencyProtocolExecuted": false
        },
        "engine": { "threads":4, "samplingStrategy":"greedy", "bestOf":1, "useGpu":true, "flashAttention":false,
            "suppressNonSpeechTokens":false, "noSpeechThreshold":0.6, "logprobThreshold":-1.0, "entropyThreshold":2.4,
            "utteranceMode":"complete-recording", "freshStatePerJob":true,
            "inputSegmentation":"energy-range", "energyStartRms":0.00005, "energyEndRms":0.000025,
            "energyFrameSamples":320, "prerollMillis":150, "tailMillis":200, "minimumBackendSamples":16000, "noContext":true, "translate":false, "detectLanguage":false,
            "noTimestamps":!cli.native_timestamps, "printSpecial":false, "printProgress":false, "printRealtime":false, "printTimestamps":false,
            "speechGate":gate_metadata },
        "cases": observations,
    });
    input::write_reports(
        &cli.output,
        &serde_json::to_vec_pretty(&results)?,
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    if results.iter().any(|row| row.status == "failed") || cleanup_error.is_some() {
        return Err("corpus probe recorded failures; inspect private output and metadata".into());
    }
    Ok(())
}
#[cfg(feature = "measurement")]
struct NativeTimestampDecoder(everlast_dictation::transcription::Engine);

#[cfg(feature = "measurement")]
impl everlast_dictation::inference_service::Decoder for NativeTimestampDecoder {
    fn decode(
        &mut self,
        audio: &[f32],
        language: Language,
    ) -> Result<String, everlast_dictation::transcription::InferenceError> {
        self.0
            .transcribe_timestamped_observed(audio, language)
            .map(|observed| observed.text)
    }
}

fn load_service(
    model: PathBuf,
    definition: ModelDefinition,
    native_timestamps: bool,
) -> Result<InferenceService, Error> {
    if native_timestamps {
        #[cfg(feature = "measurement")]
        {
            let engine = everlast_dictation::transcription::Engine::load(&model, definition)?;
            return Ok(InferenceService::with_decoder(NativeTimestampDecoder(
                engine,
            ))?);
        }
        #[cfg(not(feature = "measurement"))]
        return Err("native timestamp diagnostic requires measurement".into());
    }
    Ok(InferenceService::load(model, definition)?)
}

fn wait_ready(service: &InferenceService) -> Result<(), String> {
    let started = Instant::now();
    loop {
        if service.ready() {
            return Ok(());
        }
        if service.is_finished() {
            return Err("worker stopped before model readiness".into());
        }
        if started.elapsed() >= WAIT_LIMIT {
            return Err(
                "model readiness observation exceeded 120 seconds; cleanup may take longer".into(),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
}
fn infer(
    service: &InferenceService,
    id: u64,
    samples: Vec<f32>,
    language: Language,
) -> Result<String, (String, bool)> {
    let frames = samples.len();
    let token = WorkToken::default();
    service
        .submit(Job {
            session: id,
            generation: 1,
            job_id: id,
            sample_range: 0..frames,
            audio: Arc::new(samples),
            language,
            token: token.clone(),
        })
        .map_err(|error| (error.to_string(), true))?;
    let started = Instant::now();
    loop {
        if let Some(completed) = service
            .take_result()
            .map_err(|error| (error.to_string(), true))?
        {
            if completed.session != id
                || completed.generation != 1
                || completed.job_id != id
                || completed.sample_range != (0..frames)
            {
                token.cancel();
                return Err(("inference result identity mismatch".into(), true));
            }
            return completed.result.map_err(|error| (error.to_string(), false));
        }
        if service.is_finished() {
            return Err(("worker stopped before inference result".into(), true));
        }
        if started.elapsed() >= WAIT_LIMIT {
            token.cancel();
            return Err((
                "job observation exceeded 120 seconds; cleanup may take longer".into(),
                true,
            ));
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everlast_dictation::{inference_service::Decoder, transcription::InferenceError};
    struct FixtureDecoder;
    impl Decoder for FixtureDecoder {
        fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
            if audio != [0.0, 0.5] {
                return Err(InferenceError::InvalidAudio);
            }
            Ok(match language {
                Language::German => "Guten Tag.",
                Language::English => "Good day.",
            }
            .into())
        }
    }
    #[test]
    fn one_warm_service_accepts_fresh_tagged_jobs_with_the_requested_language() -> Result<(), Error>
    {
        let worker = InferenceService::with_decoder(FixtureDecoder)?;
        wait_ready(&worker)?;
        assert_eq!(
            infer(&worker, 1, vec![0.0, 0.5], Language::German).map_err(|(error, _)| error)?,
            "Guten Tag."
        );
        assert_eq!(
            infer(&worker, 2, vec![0.0, 0.5], Language::English).map_err(|(error, _)| error)?,
            "Good day."
        );
        assert!(matches!(
            infer(&worker, 3, vec![0.0], Language::German),
            Err((_, false))
        ));
        assert_eq!(
            infer(&worker, 4, vec![0.0, 0.5], Language::German).map_err(|(error, _)| error)?,
            "Guten Tag."
        );
        worker.shutdown()?;
        Ok(())
    }
    #[test]
    fn evaluator_results_contain_only_id_status_and_text() -> Result<(), Error> {
        let result = ResultRow {
            id: "case-1".into(),
            status: "failed",
            text: None,
        };
        assert_eq!(
            serde_json::to_value(result)?,
            serde_json::json!({"id":"case-1","status":"failed","text":null})
        );
        Ok(())
    }
    fn cli_args(extra: &[&str]) -> Vec<std::ffi::OsString> {
        ["model.bin", "reference", "manifest.json", "new-output.json"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(Into::into)
            .collect()
    }

    #[cfg(feature = "measurement")]
    #[test]
    fn explicit_native_timestamp_candidate_is_accepted_only_as_optional_fifth_argument()
    -> Result<(), Error> {
        assert!(parse_arguments(cli_args(&["--native-timestamps"]))?.native_timestamps);
        Ok(())
    }

    #[test]
    fn ordinary_cli_and_model_selection_remain_valid() -> Result<(), Error> {
        let parsed = parse_arguments(cli_args(&[]))?;
        assert!(!parsed.native_timestamps);
        assert_eq!(parsed.model, Path::new("model.bin"));
        assert_eq!(parsed.manifest, Path::new("manifest.json"));
        assert_eq!(parsed.output, Path::new("new-output.json"));
        assert_eq!(parsed.definition.id(), REFERENCE.id());
        let mut arguments = cli_args(&[]);
        arguments[1] = "small".into();
        assert_eq!(parse_arguments(arguments)?.definition.id(), SMALL.id());
        Ok(())
    }

    #[cfg(not(feature = "measurement"))]
    #[test]
    fn native_timestamp_flag_without_feature_is_rejected_before_any_model_or_file_access() {
        let result = parse_arguments(cli_args(&["--native-timestamps"]));
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|e| e.to_string().contains("requires the measurement feature"))
        );
    }

    #[test]
    fn malformed_unknown_and_duplicate_flags_are_rejected_without_file_access() {
        for extra in [
            vec!["--unknown"],
            vec!["--native-timestamps", "--native-timestamps"],
            vec!["--native-timestamps", "extra"],
        ] {
            assert!(parse_arguments(cli_args(&extra)).is_err());
        }
        let mut args = cli_args(&[]);
        args[0] = "--native-timestamps".into();
        assert!(parse_arguments(args).is_err());
        let mut args = cli_args(&[]);
        args[1] = "large".into();
        assert!(parse_arguments(args).is_err());
        let mut args = cli_args(&[]);
        args[2] = "".into();
        assert!(parse_arguments(args).is_err());
        let mut args = cli_args(&[]);
        args[3] = "x".repeat(4097).into();
        assert!(parse_arguments(args).is_err());
        assert!(parse_arguments(Vec::new()).is_err());
    }
}
