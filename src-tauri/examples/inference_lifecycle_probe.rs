//! Developer-only: cargo run -p everlast-dictation --example
//! inference_lifecycle_probe -- MODEL WAV
//! Requires REFERENCE and its VAD sibling. German only. No transcript logging.
//! Native experiments are explicitly run by the integrating task, not by tests.
#![forbid(unsafe_code)]

#[allow(
    dead_code,
    reason = "share the audited bounded WAV boundary, not corpus export"
)]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "inference_lifecycle_probe/runner.rs"]
mod runner;

use everlast_dictation::{model::REFERENCE, transcription::Engine};
use std::{
    io::Cursor,
    path::Path,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).take(3).collect();
    if args.len() != 2
        || args
            .iter()
            .any(|v| v.is_empty() || v.as_encoded_bytes().len() > 4096)
    {
        return Err(
            "usage: inference_lifecycle_probe REFERENCE_MODEL_PATH PCM16_MONO_16K_WAV".into(),
        );
    }
    let bytes = input::bounded_read(Path::new(&args[1]), input::MAX_AUDIO)?;
    let declared = {
        let wav = hound::WavReader::new(Cursor::new(&bytes))?;
        input::Audio {
            path: "authorized-local-fixture.wav".into(),
            sha256: input::digest(&bytes),
            frames: wav.duration(),
            sample_rate: wav.spec().sample_rate,
        }
    };
    let samples = input::decode_audio(&bytes, &declared)?;
    let load_tick = Instant::now();
    let engine = Engine::load(Path::new(&args[0]), REFERENCE)?;
    let load_millis = load_tick.elapsed().as_millis();
    let restart = runner::restart(engine, samples, Duration::from_secs(120));
    // First service must be verifiably joined before another model owner loads.
    let shutdown = if restart.is_ok() {
        match Engine::load(Path::new(&args[0]), REFERENCE) {
            Ok(engine) => match input::decode_audio(&bytes, &declared) {
                Ok(samples) => Some(runner::shutdown_held(
                    engine,
                    samples,
                    Duration::from_secs(120),
                )),
                Err(_) => Some(Err(runner::Failure::before_worker(
                    "second audio validation failed",
                ))),
            },
            Err(error) => Some(Err(runner::Failure::before_worker(&error.to_string()))),
        }
    } else {
        None
    };
    let passed = restart.is_ok() && shutdown.as_ref().is_some_and(Result::is_ok);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1, "kind": "developer-inference-lifecycle", "passed": passed,
            "model": REFERENCE.id(), "modelSha256": REFERENCE.sha256(), "language": "de",
            "audioSha256": declared.sha256, "audioFrames": declared.frames,
            "sourceRevision": std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
            "probeSourceHash": input::digest(include_bytes!("inference_lifecycle_probe.rs")),
            "runnerSourceHash": input::digest(include_bytes!("inference_lifecycle_probe/runner.rs")),
            "firstLoadMillis": load_millis, "restart": restart, "shutdownHeld": shutdown,
            "notes": [
                "Wrapper-entry barrier is before the actual native call; it is not a native preemption probe.",
                "Cancelled job 1 deliberately executes its real decoder after release; its result must be discarded.",
            "Shutdown must remain incomplete during a 100-ms receive observation before releasing the held decoder; early results are preserved for cleanup.",
                "Same-input transcript equality measures repeatability, not independent accuracy or language qualification.",
                "No microphone, GUI, paste, bundle rebuild or transcript output; not a 100-native-session test.",
                "Channel/poll/cleanup observations are bounded; timeout cannot interrupt native inference and explicitly leaves join unverified.",
            "Initial Engine loading is synchronous; actual Engine invocation does not prove its energy/VAD gate admitted a native Whisper decode.",
                "Peak fixture ownership is bounded to three independent journals plus one WAV byte buffer; no unbounded job queue.",
            ]
        }))?
    );
    if !passed {
        return Err("lifecycle probe failed; see text-free evidence and cleanup status".into());
    }
    Ok(())
}
