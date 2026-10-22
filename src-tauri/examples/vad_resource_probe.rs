//! Constructed maximum-duration resource workload; never speech-quality evidence.
#![forbid(unsafe_code)]

use everlast_dictation::model::{VAD, verify_file};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Instant,
};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams};

type Error = Box<dyn std::error::Error>;
const MAX_SAMPLES: usize = 4_800_000;

fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().skip(1).take(4).collect();
    if !(2..=3).contains(&args.len())
        || args
            .iter()
            .any(|a| a.is_empty() || a.as_encoded_bytes().len() > 4096)
    {
        return Err("usage: vad_resource_probe MODEL_PATH NEW_OUTPUT_JSON [max]".into());
    }
    let maximum = match args.get(2).and_then(|a| a.to_str()) {
        None if args.len() == 2 => false,
        Some("max") => true,
        _ => return Err("optional workload must be max".into()),
    };
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(Path::new(&args[1]))?;
    verify_file(Path::new(&args[0]), VAD)?;
    // Initialize from a private snapshot of the actually verified bytes.
    let mut bytes = Vec::new();
    std::fs::File::open(&args[0])?
        .take(VAD.bytes() + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != VAD.bytes()
        || (Sha256::digest(&bytes))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != VAD.sha256()
    {
        return Err("VAD snapshot hash mismatch".into());
    }
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&bytes)?;
    snapshot.flush()?;
    drop(bytes);
    whisper_rs::install_logging_hooks();
    let mut params = WhisperVadContextParams::new();
    params.set_n_threads(1);
    params.set_use_gpu(false);
    let started = Instant::now();
    let mut context = WhisperVadContext::new(
        snapshot.path().to_str().ok_or("invalid snapshot path")?,
        params,
    )?;
    let init_micros = started.elapsed().as_micros();
    let frames = if maximum { MAX_SAMPLES } else { 3200 };
    let mut original = Vec::new();
    original.try_reserve_exact(frames)?;
    original.extend((0..frames).map(|i| ((i % 32) as f32 - 15.5) * 0.00001));
    let mut scratch = Vec::new();
    scratch.try_reserve_exact(frames)?;
    let peak = original.iter().map(|x| x.abs()).fold(0.0, f32::max);
    let gain = 0.95 / f64::from(peak);
    let prepare = Instant::now();
    scratch.extend(original.iter().map(|x| (f64::from(*x) * gain) as f32));
    let prepare_micros = prepare.elapsed().as_micros();
    let tick = Instant::now();
    context.detect_speech(&scratch)?;
    let maximum_call_micros = tick.elapsed().as_micros();
    let probabilities = context.probabilities();
    if probabilities.len() != frames.div_ceil(512)
        || probabilities
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
    {
        return Err("invalid constructed-workload scores".into());
    }
    let max_probability = probabilities.iter().copied().fold(0.0, f32::max);
    let retained_windows = probabilities.len();
    let repeated = Instant::now();
    for _ in 0..100 {
        context.detect_speech(&scratch[..3200])?;
        let probabilities = context.probabilities();
        if probabilities.len() != 7
            || probabilities
                .iter()
                .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err("invalid short-repeat scores".into());
        }
    }
    let repeats_micros = repeated.elapsed().as_micros();
    let report = serde_json::json!({
        "schemaVersion":1,"kind":"constructed-vad-resource-workload","speechQualityEvidence":false,"nativeSessionAcceptance":false,
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "probeSourceSha256":(Sha256::digest(include_bytes!("vad_resource_probe.rs"))).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "cppSourceSha256":(Sha256::digest(include_bytes!("../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"))).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "model":{"id":VAD.id(),"sha256":VAD.sha256()},
        "workload":{"frames":frames,"sampleRate":16000,"waveform":"constructed-periodic-not-speech","peakTarget":0.95,"gain":gain,"warmContext":true,"shortRepeats":100,"shortRepeatFrames":3200,"threads":1,"useGpu":false},
        "timing":{"initMicros":init_micros,"prepareMicros":prepare_micros,"maximumCallMicros":maximum_call_micros,"shortRepeatsMicros":repeats_micros},
        "observed":{"probabilityWindows":retained_windows,"maxProbability":max_probability,"originalCapacityBytes":original.capacity()*4,"scratchCapacityBytes":scratch.capacity()*4},
        "limits":{"rssExternal":true,"heapLeakProven":false,"cancelLatencyMeasured":false,"realTimeL":false,"naturalMonologue":false}
    });
    output.write_all(&serde_json::to_vec_pretty(&report)?)?;
    output.sync_all()?;
    Ok(())
}
