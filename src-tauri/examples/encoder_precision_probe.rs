//! Developer-only direct backend diagnosis; no paced Stop-to-text claim.
#![forbid(unsafe_code)]

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use everlast_dictation::transcription::Language;
    use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(5..=7).contains(&arguments.len()) {
        return Err(
            "usage: encoder_precision_probe MODEL fp32|fp16 WAV de|en NEW_OUTPUT [REPEATS] [--cache-budget-mib=256|512|768]".into(),
        );
    }
    let precision = match arguments[1].to_str() {
        Some("fp32") => "fp32",
        Some("fp16") => "fp16",
        _ => return Err("explicit fp32 or fp16 diagnostic precision required".into()),
    };
    let language = match arguments[3].to_str() {
        Some("de") => Language::German,
        Some("en") => Language::English,
        _ => return Err("explicit de or en language required".into()),
    };
    let cache_budget = arguments
        .get(6)
        .map(|value| match value.to_str() {
            Some("--cache-budget-mib=256") => Ok(256 * 1024 * 1024),
            Some("--cache-budget-mib=512") => Ok(512 * 1024 * 1024),
            Some("--cache-budget-mib=768") => Ok(768 * 1024 * 1024),
            _ => Err("explicit supported cache-budget option required"),
        })
        .transpose()?;
    if cache_budget.is_some() && precision != "fp16" {
        return Err("cache-budget screening requires the explicit fp16 profile".into());
    }
    let repeats = arguments
        .get(5)
        .map(|value| {
            value
                .to_str()
                .ok_or("invalid repeat count")?
                .parse::<usize>()
                .map_err(|_| "invalid repeat count")
        })
        .transpose()?
        .unwrap_or(1);
    if !(1..=8).contains(&repeats) {
        return Err("diagnosis requires 1..=8 calls; none may be discarded".into());
    }
    let model = PathBuf::from(&arguments[0]);
    let wav = PathBuf::from(&arguments[2]);
    if fs::metadata(&wav)?.len() > 10 * 1024 * 1024 {
        return Err("bounded diagnostic WAV required".into());
    }
    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000
        || spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || reader.duration() == 0
        || reader.duration() > 4_800_000
    {
        return Err("nonempty mono PCM16 at 16 kHz and at most 300 s required".into());
    }
    let samples = reader
        .samples::<i16>()
        .map(|value| value.map(|sample| f32::from(sample) / 32768.0))
        .collect::<Result<Vec<_>, _>>()?;
    if samples.is_empty() || samples.len() > 4_800_000 {
        return Err("diagnostic PCM extent differs".into());
    }
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&arguments[4])?;
    let worker = std::thread::Builder::new()
        .name("mlx-encoder-dtype-diagnostic".into())
        .spawn(move || diagnose(model, precision, samples, language, repeats, cache_budget))?;
    let result = worker
        .join()
        .map_err(|_| "encoder dtype diagnostic worker panicked")?;
    let report = match result {
        Ok(report) => report,
        Err(error) => serde_json::json!({
            "schemaVersion":1,"passed":false,"qualified":false,
            "requestedPrecision":precision,"loadOrObservationError":error,
            "workerJoined":true,"requestedCalls":repeats,"rows":[]
        }),
    };
    serde_json::to_writer_pretty(&mut output, &report)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    if report["passed"] != true {
        return Err("encoder dtype diagnosis failed; preserve report and log".into());
    }
    Ok(())
}

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn diagnose(
    model: std::path::PathBuf,
    precision: &'static str,
    samples: Vec<f32>,
    language: everlast_dictation::transcription::Language,
    repeats: usize,
    cache_budget: Option<u64>,
) -> Result<serde_json::Value, String> {
    use everlast_dictation::{
        inference_service::Decoder,
        model::{MLX_REFERENCE_COEFFICIENTS, REFERENCE},
        transcription::mlx::MlxEngine,
    };
    use mlxrs::Dtype;
    use std::time::Instant;
    let started = Instant::now();
    let mut engine = match (precision, cache_budget) {
        ("fp32", None) => MlxEngine::load_reference_encoder_f32_for_measurement(&model),
        ("fp16", None) => MlxEngine::load_reference_encoder_f16_for_measurement(&model),
        ("fp16", Some(bytes)) => {
            MlxEngine::load_reference_encoder_with_cache_budget_for_measurement(&model, bytes)
        }
        _ => return Err("unsupported diagnostic precision".into()),
    }
    .map_err(|error| error.to_string())?;
    let load_millis = started.elapsed().as_millis();
    let actual_dtype = engine.reference_encoder_dtype_for_measurement();
    let expected_dtype = if precision == "fp16" {
        Dtype::F16
    } else {
        Dtype::F32
    };
    let actual_label = match actual_dtype {
        Some(Dtype::F16) => "fp16",
        Some(Dtype::F32) => "fp32",
        _ => "unsupported",
    };
    let mut rows = Vec::new();
    let actual_cache_budget = engine.cache_budget_for_measurement();
    let loaded_memory = observe_memory();
    let workload: Result<(), String> = (|| {
        if actual_dtype != Some(expected_dtype) {
            return Err("observed model dtype differs from requested encoder profile".into());
        }
        if actual_cache_budget != cache_budget {
            return Err("observed cache policy differs from the explicit request".into());
        }
        loaded_memory.as_ref().map_err(Clone::clone)?;
        for index in 0..repeats {
            let before = Instant::now();
            let answer = engine.transcribe(&samples, language);
            let elapsed = before.elapsed().as_nanos();
            match answer {
                Ok(text) => {
                    let memory = observe_memory();
                    rows.push(serde_json::json!({
                        "runIndex":index,"firstCall":index==0,"status":"ok","text":text,
                        "directBackendCallNanos":elapsed,"memory":memory.as_ref().ok(),
                        "memoryError":memory.as_ref().err()
                    }));
                    memory?;
                }
                Err(error) => {
                    rows.push(serde_json::json!({
                        "runIndex":index,"firstCall":index==0,"status":"error","text":"",
                        "directBackendCallNanos":elapsed
                    }));
                    return Err(error.to_string());
                }
            }
        }
        Ok(())
    })();
    let finish = engine.finish();
    let after_finish_memory = observe_memory();
    Ok(serde_json::json!({
        "schemaVersion":1,"kind":"reference-encoder-dtype-direct-diagnostic",
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "probeSha256":format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(include_bytes!("encoder_precision_probe.rs"))),
        "passed":workload.is_ok()&&finish.is_ok()&&after_finish_memory.is_ok(),"qualified":false,"workerJoined":true,
        "requestedPrecision":precision,"encoderComputeDtype":actual_label,
        "checkpointDtype":"fp32","bridgeFeatureDtype":"fp32",
        "checkpoint":{"id":MLX_REFERENCE_COEFFICIENTS.id(),
            "bytes":MLX_REFERENCE_COEFFICIENTS.bytes(),"sha256":MLX_REFERENCE_COEFFICIENTS.sha256()},
        "decoderBackend":"whisper.cpp-reference","decoderModelSha256":REFERENCE.sha256(),
        "originalFrames":samples.len(),"sampleRate":16000,"requestedCalls":repeats,
        "completedCalls":rows.len(),"notExecutedCalls":repeats-rows.len(),"rows":rows,
        "cacheBudgetRequestedBytes":cache_budget,"cacheBudgetObservedBytes":actual_cache_budget,
        "afterLoadMemory":loaded_memory.as_ref().ok(),"afterLoadMemoryError":loaded_memory.err(),
        "afterFinishMemory":after_finish_memory.as_ref().ok(),"afterFinishMemoryError":after_finish_memory.err(),
        "loadMillis":load_millis,"workloadError":workload.err(),
        "finishError":finish.err().map(|error|error.to_string()),
        "scope":"direct same-worker backend calls; counters sampled outside timed calls; not paced Stop-to-text, L, p95 or full quality qualification",
        "noOutputDiscarding":true,"cppPolicy":"unchanged existing full reference decoder",
        "longInputAndLaterSeekFallback":"unchanged native CPP"
    }))
}

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn observe_memory() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "activeBytes":mlxrs::memory::active_memory().map_err(|e|e.to_string())?,
        "cacheBytes":mlxrs::memory::cache_memory().map_err(|e|e.to_string())?,
        "peakActiveBytes":mlxrs::memory::peak_memory().map_err(|e|e.to_string())?
    }))
}

#[cfg(not(all(feature = "mlx", feature = "measurement")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("encoder_precision_probe requires features mlx,measurement".into())
}
