//! Bounded developer diagnostic of the unchanged hybrid backend's allocator.
//! Direct repeated input is not paced Stop-to-text or a new quality corpus.
#![forbid(unsafe_code)]

#[cfg(all(feature = "mlx", feature = "measurement"))]
#[path = "runtime_resource_probe/metrics.rs"]
mod metrics;

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 4 {
        return Err("usage: mlx_memory_probe MODEL WAV REFERENCE NEW_OUTPUT".into());
    }
    let model = PathBuf::from(&arguments[0]);
    let wav = PathBuf::from(&arguments[1]);
    let reference = PathBuf::from(&arguments[2]);
    if fs::metadata(&wav)?.len() > 10 * 1024 * 1024 || fs::metadata(&reference)?.len() > 4096 {
        return Err("bounded explicit diagnostic inputs required".into());
    }
    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000
        || spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("16-kHz mono PCM16 required".into());
    }
    let samples = reader
        .samples::<i16>()
        .map(|value| value.map(|sample| f32::from(sample) / 32768.0))
        .collect::<Result<Vec<_>, _>>()?;
    if samples.len() != 80_000 {
        return Err("fixed five-second diagnostic fixture required".into());
    }
    let expected = fs::read_to_string(reference)?;
    if expected.trim().is_empty() {
        return Err("nonempty independent reference required".into());
    }
    let worker = std::thread::Builder::new()
        .name("mlx-memory-diagnostic".into())
        .spawn(move || diagnose(model, samples, expected))?;
    let result = worker
        .join()
        .map_err(|_| "memory diagnostic worker panicked")?;
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            serde_json::json!({"passed":false,"loadOrObservationError":error,"workerJoined":true})
        }
    };
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&arguments[3])?;
    serde_json::to_writer_pretty(&mut output, &report)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    if report["passed"] != true {
        return Err("memory diagnostic failed; inspect private report".into());
    }
    Ok(())
}

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn diagnose(
    model: std::path::PathBuf,
    samples: Vec<f32>,
    expected: String,
) -> Result<serde_json::Value, String> {
    use everlast_dictation::{
        inference_service::Decoder,
        transcription::{Language, mlx::MlxEngine},
    };
    use std::time::Instant;
    let started = Instant::now();
    let mut engine =
        MlxEngine::load_reference_encoder(&model).map_err(|error| error.to_string())?;
    let load_millis = started.elapsed().as_millis();
    let mut rows = Vec::new();
    let workload: Result<(), String> = (|| {
        mlxrs::Stream::default_gpu()
            .and_then(|stream| stream.synchronize())
            .map_err(|error| error.to_string())?;
        rows.push(memory_row(0, true)?);
        for index in 1..=305 {
            let text = engine
                .transcribe(&samples, Language::German)
                .map_err(|error| error.to_string())?;
            if text != expected {
                return Err(format!(
                    "output mismatch at iteration {index}; no text exported"
                ));
            }
            mlxrs::Stream::default_gpu()
                .and_then(|stream| stream.synchronize())
                .map_err(|error| error.to_string())?;
            rows.push(memory_row(
                index,
                index == 5 || index % 25 == 0 || index == 305,
            )?);
            if index == 5 || index % 25 == 0 || index == 305 {
                eprintln!("completed iteration {index}/305; output matched");
            }
        }
        Ok(())
    })();
    let finish = engine.finish();
    // Counter getters only: do not recreate a stream after terminal cleanup.
    let after_finish = memory_row(306, true)?;
    Ok(
        serde_json::json!({"schemaVersion":1,"kind":"same-worker-repeated-hybrid-memory-diagnostic",
        "passed":workload.is_ok() && finish.is_ok(),"loadMillis":load_millis,
        "loadScope":"synchronous model construction on the owning worker; OS caches retained",
        "warmups":5,"measuredRequested":300,"completedTranscriptions":rows.len()-1,
        "allCompletedOutputsMatched":workload.is_ok(),"workloadError":workload.err(),
        "finishError":finish.err().map(|error| error.to_string()),"workerJoined":true,
        "rows":rows,"afterFinish":after_finish,"qualified":false,
        "limits":["direct repeated backend calls; no capture/controller/cancel/paced L proof",
                  "MLX active/cache/peak counters are process-global allocator bytes, not total process footprint",
                  "no cache clearing, limit changes, peak reset or stream cleanup between iterations"]}),
    )
}

#[cfg(all(feature = "mlx", feature = "measurement"))]
fn memory_row(index: usize, observe_os: bool) -> Result<serde_json::Value, String> {
    let active = mlxrs::memory::active_memory().map_err(|error| error.to_string())?;
    let cache = mlxrs::memory::cache_memory().map_err(|error| error.to_string())?;
    let peak = mlxrs::memory::peak_memory().map_err(|error| error.to_string())?;
    Ok(
        serde_json::json!({"iteration":index,"activeBytes":active,"cacheBytes":cache,"peakBytes":peak,
        "os":observe_os.then(|| metrics::observe(std::process::id(),false))}),
    )
}

#[cfg(not(all(feature = "mlx", feature = "measurement")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("mlx_memory_probe requires features mlx,measurement".into())
}
