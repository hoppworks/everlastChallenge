//! Bounded RAM diagnostic through the existing runtime/resource workload.
//! OS observations deliberately perturb timing; this is not a speed gate.
#![forbid(unsafe_code)]

#[allow(dead_code, reason = "reuse bounded corpus input helpers")]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "runtime_resource_probe/metrics.rs"]
mod metrics;
#[allow(
    dead_code,
    reason = "short profile uses its own bounded workload counts"
)]
#[path = "runtime_resource_probe/runner.rs"]
mod runner;
#[allow(dead_code, reason = "reuse readiness and confirmed runtime cleanup")]
#[path = "paced_runtime_probe/runner.rs"]
mod shared;

use crossbeam_channel::{Sender, bounded};
use dictation_core::audio::AudioFormat;
use everlast_dictation::{
    inference_service::Decoder,
    runtime::measurement::MeasuredRuntime,
    transcription::{InferenceError, Language, mlx::MlxEngine},
};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

type Error = Box<dyn std::error::Error>;
const WARMUPS: u32 = 3;
const CYCLES: u32 = 13;

#[derive(Clone)]
struct Observations {
    sender: Sender<Value>,
    overflow: Arc<AtomicBool>,
    start: Instant,
}

impl Observations {
    fn capture(&self, phase: &str, details: Value) {
        let active = mlxrs::memory::active_memory();
        let cache = mlxrs::memory::cache_memory();
        let peak = mlxrs::memory::peak_memory();
        let os = metrics::observe(std::process::id(), true);
        let row = json!({
            "phase": phase, "details": details,
            "elapsedMicros": self.start.elapsed().as_micros(),
            "thread": format!("{:?}", std::thread::current().id()),
            "activeBytes": active.as_ref().ok(), "activeError": active.err().map(|e|e.to_string()),
            "cacheBytes": cache.as_ref().ok(), "cacheError": cache.err().map(|e|e.to_string()),
            "peakBytes": peak.as_ref().ok(), "peakError": peak.err().map(|e|e.to_string()),
            "os": os
        });
        if self.sender.try_send(row).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
}

struct ObservedDecoder {
    inner: MlxEngine,
    observations: Observations,
    calls: u64,
    warm_peak_reset: bool,
}

impl Decoder for ObservedDecoder {
    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        self.calls += 1;
        if self.warm_peak_reset && self.calls == u64::from(WARMUPS) + 1 {
            self.observations
                .capture("before-warm-peak-reset", Value::Null);
            let reset = mlxrs::memory::reset_peak_memory();
            self.observations.capture(
                "after-warm-peak-reset",
                json!({
                    "resetSucceeded":reset.is_ok(), "purpose":"warm-workspace accounting only"
                }),
            );
            reset.map_err(|_| InferenceError::Context)?;
        }
        let detail = json!({"call":self.calls,"frames":audio.len()});
        self.observations.capture("before-decode", detail.clone());
        let result = self.inner.decode(audio, language);
        self.observations.capture(
            "after-decode",
            json!({
                "call":self.calls,"frames":audio.len(),"decodeSucceeded":result.is_ok()
            }),
        );
        result
    }

    fn finish(self) -> Result<(), InferenceError> {
        self.observations
            .capture("before-finish", json!({"calls":self.calls}));
        let result = self.inner.finish();
        // Counter getters do not create a stream or reset allocator state.
        self.observations
            .capture("after-finish", json!({"finishSucceeded":result.is_ok()}));
        result
    }
}

fn write_report(path: &Path, report: &Value) -> Result<(), Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer_pretty(&mut temporary, report)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(4..=5).contains(&args.len()) || args.iter().any(|a| a.is_empty()) {
        return Err(
            "usage: runtime_memory_profile MODEL WAV REFERENCE_UTF8 NEW_OUTPUT_JSON [--warm-peak-reset]".into(),
        );
    }
    let warm_peak_reset = match args.get(4) {
        None => false,
        Some(flag) if flag == "--warm-peak-reset" => true,
        Some(_) => return Err("unknown diagnostic option".into()),
    };
    let output = Path::new(&args[3]);
    if output.try_exists()? {
        return Err("output must be new".into());
    }
    let wav_bytes = input::bounded_read(Path::new(&args[1]), 1024 * 1024)?;
    let wav = hound::WavReader::new(Cursor::new(&wav_bytes))?;
    let audio = input::Audio {
        path: "explicit-local-fixture.wav".into(),
        sha256: input::digest(&wav_bytes),
        frames: wav.duration(),
        sample_rate: wav.spec().sample_rate,
    };
    let samples: Arc<[f32]> = input::decode_audio(&wav_bytes, &audio)?.into();
    if samples.len() != 80_000 {
        return Err("fixed five-second fixture required".into());
    }
    let reference_bytes = input::bounded_read(Path::new(&args[2]), 4096)?;
    let reference = std::str::from_utf8(&reference_bytes)?;
    if reference.trim().is_empty() {
        return Err("nonempty reference required".into());
    }
    let (sender, receiver) = bounded(128);
    let observations = Observations {
        sender,
        overflow: Arc::new(AtomicBool::new(false)),
        start: Instant::now(),
    };
    let worker_observations = observations.clone();
    let model = PathBuf::from(&args[0]);
    let load_start = Instant::now();
    let initialized = runner::controlled_service(
        move || {
            worker_observations.capture("before-load", Value::Null);
            let started = Instant::now();
            let engine = MlxEngine::load_reference_encoder(&model)?;
            let observed_dtype = match engine.reference_encoder_dtype_for_measurement() {
                Some(mlxrs::Dtype::F16) => "fp16",
                Some(mlxrs::Dtype::F32) => "fp32",
                _ => "unsupported",
            };
            worker_observations.capture(
                "after-load",
                json!({
                    "loadMicrosExcludingOsSamples":started.elapsed().as_micros(),
                    "observedEncoderDtype":observed_dtype,
                    "compiledEncoderPrecision":MlxEngine::reference_encoder_compute_precision(),
                    "cacheBudgetBytes":engine.cache_budget_for_measurement()
                }),
            );
            Ok(ObservedDecoder {
                inner: engine,
                observations: worker_observations,
                calls: 0,
                warm_peak_reset,
            })
        },
        Arc::clone(&samples),
        runner::WAIT,
    )
    .map_err(|e| e.to_string())
    .and_then(|(service, control)| {
        MeasuredRuntime::spawn(
            service,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )
        .map(|runtime| (runtime, control))
        .map_err(|e| e.to_string())
    });
    let worker_spawn_micros = load_start.elapsed().as_micros();
    let config = runner::Config {
        warmups: WARMUPS,
        cycles: CYCLES,
        timeout: runner::WAIT,
    };
    let (workload, cleanup, diagnostics, error) = match initialized {
        Ok((runtime, control)) => {
            let workload = match shared::wait_ready(&runtime) {
                Ok(()) => {
                    observations.capture("ready", Value::Null);
                    runner::workload(&runtime, &control, &samples, reference, config, |point| {
                        observations.capture("checkpoint", json!(point));
                    })
                }
                Err(e) => runner::unavailable(e, config),
            };
            let cleanup = shared::cleanup(runtime);
            let diagnostics = control.diagnostics();
            observations.capture("after-join", json!({"joined":cleanup.joined}));
            (workload, Some(cleanup), Some(diagnostics), None)
        }
        Err(e) => (runner::unavailable(e.clone(), config), None, None, Some(e)),
    };
    let rows = receiver.try_iter().collect::<Vec<_>>();
    let loaded = rows
        .iter()
        .filter(|r| r["phase"] == "after-load")
        .collect::<Vec<_>>();
    let precision_valid = loaded.len() == 1
        && loaded[0]["details"]["observedEncoderDtype"]
            == MlxEngine::reference_encoder_compute_precision();
    let metrics_complete = !observations.overflow.load(Ordering::Acquire)
        && !rows.is_empty()
        && rows.iter().all(|r| {
            r["activeBytes"].is_u64()
                && r["cacheBytes"].is_u64()
                && r["peakBytes"].is_u64()
                && r["os"]["rssBytes"].is_u64()
                && r["os"]["vmmapSummary"].is_string()
                && r["os"]["rssError"].is_null()
                && r["os"]["vmmapError"].is_null()
        });
    let behavior_passed = workload.behavior_passed
        && cleanup.as_ref().is_some_and(|c| c.joined)
        && diagnostics.as_ref().is_some_and(|d| {
            d.as_ref().is_ok_and(|d| {
                !d.active
                    && !d.armed
                    && d.started == d.completed
                    && d.mismatches == 0
                    && d.last_error.is_none()
            })
        });
    let report = json!({
        "schemaVersion":1,"kind":"bounded-runtime-ram-profile-v1","qualified":false,
        "behaviorPassed":behavior_passed,"metricsComplete":metrics_complete,"precisionValid":precision_valid,
        "warmups":WARMUPS,"cycles":CYCLES,"workerSpawnMicros":worker_spawn_micros,
        "warmPeakResetRequested":warm_peak_reset,
        "compiledEncoderPrecision":MlxEngine::reference_encoder_compute_precision(),
        "probeSha256":input::digest(include_bytes!("runtime_memory_profile.rs")),
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "fixture":{"sha256":audio.sha256,"frames":samples.len(),"sampleRate":16000,"channels":1},
        "referenceSha256":input::digest(&reference_bytes),"loadOrSetupError":error,
        "workload":workload,"cleanup":cleanup,"decoderDiagnostics":diagnostics,
        "observations":rows,"overflow":observations.overflow.load(Ordering::Acquire),
        "limits":["diagnostic OS sampling perturbs time; no speed or L acceptance",
            "workerSpawnMicros ends before asynchronous model initialization; use worker-local after-load timing for model load",
            "3 warmups and 13 cycles are a short RAM profile, not the full 100-cycle resource gate",
            "MLX active/cache/peak and RSS/macOS physical footprint are distinct overlapping metrics; never sum them",
            "OS phase observations do not capture every transient allocation; external time -l retains whole-process maxima",
            "allocator policy comes from the actual model loader; installed budget is observed after load and cleanup may release its cache",
            "the diagnostic adds no per-call cache clearing; optional warm peak accounting reset has explicit before/after observations",
            "decoder entry cancellation uses the existing wrapper barrier; no native kernel interruption claim",
            "input/reference bytes and bounded diagnostic records remain resident throughout the workload"]
    });
    write_report(output, &report)?;
    if !behavior_passed || !metrics_complete || !precision_valid {
        return Err("RAM profile incomplete; inspect retained report".into());
    }
    Ok(())
}
