//! Developer-only phase attribution through the actual serial InferenceService.
//! --print-parameters performs no model work. No hypothesis text is exported.
#![forbid(unsafe_code)]
#![recursion_limit = "256"]
#[allow(
    dead_code,
    reason = "reuse the audited bounded WAV boundary, not corpus export"
)]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "engine_profile_probe/request.rs"]
mod request;
use dictation_core::audio::{AudioFormat, AudioProcessor};
use everlast_dictation::{
    inference_service::{Decoder, InferenceService, Job, WorkToken},
    model::REFERENCE,
    transcription::{
        Engine, InferenceError, Language,
        profile::{EngineProfile, Phase},
    },
};
use request::{Request, RequestGate};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    io::{Cursor, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
type Error = Box<dyn std::error::Error>;
const WAIT: Duration = Duration::from_secs(120);
const RUNS: u64 = 4;
const ROUTINE: &str = "production-reference-transcribe-profiled-v1";

struct Receipt {
    request: Request,
    thread_name: Option<String>,
    thread_id: String,
    profile: EngineProfile,
}
trait ProfileEngine: Send + 'static {
    fn profile(
        &mut self,
        audio: &[f32],
        language: Language,
    ) -> everlast_dictation::transcription::ProfiledTranscription;
}
impl ProfileEngine for Engine {
    fn profile(
        &mut self,
        audio: &[f32],
        language: Language,
    ) -> everlast_dictation::transcription::ProfiledTranscription {
        self.transcribe_profiled(audio, language)
    }
}
struct ProfiledDecoder<E = Engine> {
    engine: E,
    sender: mpsc::SyncSender<Receipt>,
    gate: Arc<RequestGate>,
}
impl<E: ProfileEngine> Decoder for ProfiledDecoder<E> {
    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        let request = self
            .gate
            .consume(audio.len(), language.code(), &pcm_fingerprint(audio))
            .map_err(|_| InferenceError::Observation)?;
        let actual = thread::current();
        let result = self.engine.profile(audio, language);
        let receipt = Receipt {
            request,
            thread_name: actual.name().map(str::to_owned),
            thread_id: format!("{:?}", actual.id()),
            profile: result.profile,
        };
        self.sender
            .try_send(receipt)
            .map_err(|_| InferenceError::Observation)?;
        result.result
    }
}

fn template() -> Result<Value, Error> {
    // The historical complete reference manifest supplies the unchanged exact
    // decoder/VAD/state configuration, not its old source/measurement claims.
    let baseline: Value = serde_json::from_slice(include_bytes!(
        "../../tests/fixtures/corpus/p06-pretrans-screening-reference-parameters-v1.json"
    ))?;
    Ok(json!({
        "schemaVersion":1, "kind":"engine-phase-diagnostic-parameters", "routineId":ROUTINE,
        "model":{"id":REFERENCE.id(),"sha256":REFERENCE.sha256()},
        "runtime":baseline["runtime"],"state":baseline["state"],"preprocessing":baseline["preprocessing"],
        "diagnostic":{"warmupRuns":1,"measuredRuns":3,"serialService":"InferenceService::with_decoder",
            "engineInitialization":"CLI main, then owned Engine moved into actual local-inference worker",
            "noParameterOrSchedulingChange":true,
            "inputProducer":"existing AudioProcessor 16k mono, push_interleaved320-frame chunks and exactly one finish/flush; frame and float32-LE SHA equality required",
            "inputScope":"same prepared PCM primitive as Runtime, not a paced CaptureOwner/Stop or whole-Runtime cost",
            "receiptIdentity":"one armed request before Submit; worker checks exact frames/language/float32-LE hash, receipt and Completed match SID/generation/job/storage/timeline/ordinal",
            "qosScope":"pthread QoS class/relative-priority API readback, not effective override or scheduling entitlement","qualified":false,"L":null,
            "phaseScope":"Instant wall time; native Full includes Mel/encoder/decoder; CPU snapshots only calling worker",
            "totalScope":"after before-thread snapshot through end of Engine call, before after-thread snapshot",
            "unassigned":"padding, parameter preparation and observation bookkeeping; phase sums need not equal total",
            "limits":"No native-kernel attribution, CPU/GPU utilization claim, continuous QoS claim or stop-to-text measurement"},
    "baselineSourceHashes":{
            "capture": {"filename":"src-tauri/src/audio.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/audio.rs"))},
            "cargoLock": {"filename":"Cargo.lock","sha256":input::digest(include_bytes!("../../Cargo.lock"))},
            "clock": {"filename":"src-tauri/src/audio/replay/clock.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/audio/replay/clock.rs"))},
            "controller": {"filename":"src-tauri/src/controller.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/controller.rs"))},
            "cpp": {"filename":"vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp","sha256":input::digest(include_bytes!("../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"))},
            "energy": {"filename":"crates/dictation-core/src/audio.rs","sha256":input::digest(include_bytes!("../../crates/dictation-core/src/audio.rs"))},
            "engine": {"filename":"src-tauri/src/transcription.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/transcription.rs"))},
            "inference": {"filename":"src-tauri/src/inference_service.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/inference_service.rs"))},
            "jobTrace": {"filename":"src-tauri/src/runtime/measurement/job_trace.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/runtime/measurement/job_trace.rs"))},
            "manifest": {"filename":"src-tauri/Cargo.toml","sha256":input::digest(include_bytes!("../../src-tauri/Cargo.toml"))},
            "measurement": {"filename":"src-tauri/src/runtime/measurement.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/runtime/measurement.rs"))},
            "models": {"filename":"src-tauri/src/model.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/model.rs"))},
            "nativeLibrary": {"filename":"crates/macos-integration/src/lib.rs","sha256":input::digest(include_bytes!("../../crates/macos-integration/src/lib.rs"))},
            "nativeManifest": {"filename":"crates/macos-integration/Cargo.toml","sha256":input::digest(include_bytes!("../../crates/macos-integration/Cargo.toml"))},
            "nativeRealtime": {"filename":"crates/macos-integration/src/realtime.rs","sha256":input::digest(include_bytes!("../../crates/macos-integration/src/realtime.rs"))},
            "options": {"filename":"src-tauri/examples/paced_runtime_probe/options.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/options.rs"))},
            "parameters": {"filename":"src-tauri/examples/paced_runtime_probe/parameters.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/parameters.rs"))},
            "paramsWrapper": {"filename":"vendor/whisper-rs/src/whisper_params.rs","sha256":input::digest(include_bytes!("../../vendor/whisper-rs/src/whisper_params.rs"))},
            "pausePolicy": {"filename":"crates/dictation-core/src/audio/pauses.rs","sha256":input::digest(include_bytes!("../../crates/dictation-core/src/audio/pauses.rs"))},
            "prefixOwnership": {"filename":"src-tauri/src/audio/pretranscription.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/audio/pretranscription.rs"))},
            "pretransAudit": {"filename":"src-tauri/examples/paced_runtime_probe/pretranscription.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/pretranscription.rs"))},
            "pretransConfiguration": {"filename":"src-tauri/examples/paced_runtime_probe/pretranscription_configuration.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/pretranscription_configuration.rs"))},
            "probe": {"filename":"src-tauri/examples/paced_runtime_probe.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe.rs"))},
            "publisher": {"filename":"src-tauri/examples/paced_runtime_probe/publisher.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/publisher.rs"))},
            "publisherQos": {"filename":"crates/macos-integration/src/qos.rs","sha256":input::digest(include_bytes!("../../crates/macos-integration/src/qos.rs"))},
            "replay": {"filename":"src-tauri/src/audio/replay.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/audio/replay.rs"))},
            "runner": {"filename":"src-tauri/examples/paced_runtime_probe/runner.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/runner.rs"))},
            "runtime": {"filename":"src-tauri/src/runtime.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/runtime.rs"))},
            "schedule": {"filename":"src-tauri/src/audio/replay/schedule.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/audio/replay/schedule.rs"))},
            "sessionLedger": {"filename":"src-tauri/src/controller/pretranscription.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/controller/pretranscription.rs"))},
            "speechGate": {"filename":"src-tauri/src/transcription/speech_gate.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/transcription/speech_gate.rs"))},
            "statistics": {"filename":"src-tauri/examples/paced_runtime_probe/statistics.rs","sha256":input::digest(include_bytes!("../../src-tauri/examples/paced_runtime_probe/statistics.rs"))},
            "vadWrapper": {"filename":"vendor/whisper-rs/src/whisper_vad.rs","sha256":input::digest(include_bytes!("../../vendor/whisper-rs/src/whisper_vad.rs"))},
            "workerObservation": {"filename":"src-tauri/src/inference_service/observation.rs","sha256":input::digest(include_bytes!("../../src-tauri/src/inference_service/observation.rs"))},
            "workspaceManifest": {"filename":"Cargo.toml","sha256":input::digest(include_bytes!("../../Cargo.toml"))}
        },
        "sourceHashes":{
            "probe":{"filename":"src-tauri/examples/engine_profile_probe.rs","sha256":input::digest(include_bytes!("engine_profile_probe.rs"))},
            "engine":{"filename":"src-tauri/src/transcription.rs","sha256":input::digest(include_bytes!("../src/transcription.rs"))},
            "gate":{"filename":"src-tauri/src/transcription/speech_gate.rs","sha256":input::digest(include_bytes!("../src/transcription/speech_gate.rs"))},
            "request":{"filename":"src-tauri/examples/engine_profile_probe/request.rs","sha256":input::digest(include_bytes!("engine_profile_probe/request.rs"))},
            "cargoConfig":{"filename":".cargo/config.toml","sha256":input::digest(include_bytes!("../../.cargo/config.toml"))},
            "profile":{"filename":"src-tauri/src/transcription/profile.rs","sha256":input::digest(include_bytes!("../src/transcription/profile.rs"))},
            "phase":{"filename":"src-tauri/src/transcription/phase.rs","sha256":input::digest(include_bytes!("../src/transcription/phase.rs"))},
            "inference":{"filename":"src-tauri/src/inference_service.rs","sha256":input::digest(include_bytes!("../src/inference_service.rs"))},
            "nativeObserver":{"filename":"crates/macos-integration/src/thread_observation.rs","sha256":input::digest(include_bytes!("../../crates/macos-integration/src/thread_observation.rs"))},
            "nativeLibrary":{"filename":"crates/macos-integration/src/lib.rs","sha256":input::digest(include_bytes!("../../crates/macos-integration/src/lib.rs"))},
            "input":{"filename":"src-tauri/examples/corpus_probe/input.rs","sha256":input::digest(include_bytes!("corpus_probe/input.rs"))},
            "lock":{"filename":"Cargo.lock","sha256":input::digest(include_bytes!("../../Cargo.lock"))},
            "manifest":{"filename":"src-tauri/Cargo.toml","sha256":input::digest(include_bytes!("../Cargo.toml"))},
            "completeReferenceConfig":{"filename":"tests/fixtures/corpus/p06-pretrans-screening-reference-parameters-v1.json","sha256":input::digest(include_bytes!("../../tests/fixtures/corpus/p06-pretrans-screening-reference-parameters-v1.json"))}
        }
    }))
}
fn parameter_bytes() -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(&template()?)?;
    bytes.push(10);
    Ok(bytes)
}
struct Options {
    model: PathBuf,
    wav: PathBuf,
    language: Language,
    parameters: PathBuf,
    output: PathBuf,
}
fn parse_args(args: Vec<OsString>) -> Result<Option<Options>, Error> {
    if args.len() == 1 && args[0] == "--print-parameters" {
        return Ok(None);
    }
    if args.len() != 5
        || args
            .iter()
            .any(|a| a.is_empty() || a.as_encoded_bytes().len() > 4096)
    {
        return Err(
            "usage: engine_profile_probe MODEL WAV de|en PARAMETERS_JSON NEW_OUTPUT_JSON".into(),
        );
    }
    let language = match args[2].to_str() {
        Some("de") => Language::German,
        Some("en") => Language::English,
        _ => return Err("language must be de or en".into()),
    };
    Ok(Some(Options {
        model: args[0].clone().into(),
        wav: args[1].clone().into(),
        language,
        parameters: args[3].clone().into(),
        output: args[4].clone().into(),
    }))
}
fn pcm_fingerprint(samples: &[f32]) -> String {
    let mut hash = Sha256::new();
    for sample in samples {
        hash.update(sample.to_le_bytes());
    }
    (hash.finalize())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}
fn prepare_pcm(samples: &[f32]) -> Result<(Vec<f32>, Value), Error> {
    let before = pcm_fingerprint(samples);
    let mut processor = AudioProcessor::new(AudioFormat {
        sample_rate: 16000,
        channels: 1,
    })?;
    for chunk in samples.chunks(320) {
        processor.push_interleaved(chunk)?;
    }
    let prepared = processor.finish()?;
    let after = pcm_fingerprint(&prepared);
    if prepared.len() != samples.len() || after != before {
        return Err("AudioProcessor/flush did not preserve fixed16k mono fixture bytes".into());
    }
    let evidence = json!({"producer":"AudioProcessor::push_interleaved + finish","sampleRate":16000,"channels":1,
        "chunkFrames":320,"finishCalls":1,"sourceFrames":samples.len(),"preparedFrames":prepared.len(),
        "sourceFloat32LeSha256":before,"preparedFloat32LeSha256":after,"exactFramesAndBytes":true});
    Ok((prepared, evidence))
}
fn request_json(request: &Request) -> Value {
    json!({"ordinal":request.ordinal,"session":request.session,"generation":request.generation,"jobId":request.job_id,
        "storageStart":request.storage.start,"storageEnd":request.storage.end,
        "timelineStart":request.timeline.start,"timelineEnd":request.timeline.end,
        "frames":request.frames,"language":request.language,"pcmFloat32LeSha256":request.pcm_sha256})
}
fn gate_json(gate: &RequestGate) -> Value {
    match gate.evidence() {
        Ok(evidence) => json!({"expected":evidence.expected.as_ref().map(request_json),
            "consumed":evidence.consumed,"error":evidence.error}),
        Err(error) => json!({"error":error.to_string()}),
    }
}
fn snapshot(
    value: &Result<
        macos_integration::thread_observation::ThreadSnapshot,
        macos_integration::thread_observation::ObservationError,
    >,
) -> Value {
    match value {
        Ok(s) => json!({"qosClassRaw":s.qos_class,"relativePriority":s.relative_priority,
        "qosObservation":"pthread_get_qos_class_np API class/relative priority; not effective override",
        "basicPolicyRaw":s.basic_policy,"userCpuNanos":s.user_cpu.as_nanos(),"systemCpuNanos":s.system_cpu.as_nanos(),
        "error":null}),
        Err(e) => json!({"error":e.to_string()}),
    }
}
fn profile(receipt: &Receipt) -> Value {
    let p = &receipt.profile;
    json!({"request":request_json(&receipt.request),
        "routineId":ROUTINE,"probeCallerThreadName":receipt.thread_name,"probeCallerThreadId":receipt.thread_id,
        "totalElapsedNanos":p.total_elapsed.as_nanos(),"error":p.error,
        "before":snapshot(&p.thread_before),"after":snapshot(&p.thread_after),
        "validationEnergyNanos":p.elapsed(Phase::ValidationEnergy).map(|v|v.as_nanos()),
        "vadNormalizationNanos":p.elapsed(Phase::VadNormalization).map(|v|v.as_nanos()),
        "vadInferenceNanos":p.elapsed(Phase::VadInference).map(|v|v.as_nanos()),
        "stateCreateNanos":p.elapsed(Phase::StateCreate).map(|v|v.as_nanos()),
        "nativeFullNanos":p.elapsed(Phase::NativeFull).map(|v|v.as_nanos()),
        "textJoinNanos":p.elapsed(Phase::TextJoin).map(|v|v.as_nanos()),
        "stateDropNanos":p.elapsed(Phase::StateDrop).map(|v|v.as_nanos())})
}
fn publish(output: &Path, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("diagnostic output exceeds bound".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(output)?;
    Ok(())
}
fn wait_ready(service: &InferenceService) -> Result<(), Error> {
    let deadline = Instant::now() + WAIT;
    loop {
        if service.ready() {
            return Ok(());
        }
        if service.is_finished() || Instant::now() >= deadline {
            return Err("worker readiness failed".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
}
struct WorkerIdentity<'a> {
    main_thread: &'a str,
    worker_thread: Option<String>,
    gate: &'a RequestGate,
}
fn one(
    service: &InferenceService,
    receipts: &mpsc::Receiver<Receipt>,
    audio: Arc<Vec<f32>>,
    language: Language,
    ordinal: u64,
    identity: &mut WorkerIdentity<'_>,
) -> Result<Value, Error> {
    let gate = identity.gate;
    let main_thread = identity.main_thread;
    let worker_thread = &mut identity.worker_thread;
    match receipts.try_recv() {
        Ok(extra) => {
            return Ok(
                json!({"ordinal":ordinal,"warmup":ordinal==1,"status":"invalid_profile",
            "error":"unexpected unconsumed profile receipt","profile":profile(&extra),"requestState":gate_json(gate)}),
            );
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            return Err("profile receipt channel disconnected".into());
        }
    }
    let frames = audio.len();
    let expected = Request {
        ordinal,
        session: ordinal,
        generation: 1,
        job_id: ordinal,
        storage: 0..frames,
        timeline: 0..frames,
        frames,
        language: language.code(),
        pcm_sha256: pcm_fingerprint(&audio),
    };
    gate.arm(expected.clone())?;
    service.submit(Job {
        session: ordinal,
        generation: 1,
        job_id: ordinal,
        sample_range: 0..frames,
        timeline_range: 0..frames,
        audio,
        language,
        token: WorkToken::default(),
    })?;
    let deadline = Instant::now() + WAIT;
    let completed = loop {
        if let Some(result) = service.take_result()? {
            break result;
        }
        if service.is_finished() || Instant::now() >= deadline {
            return Err("completion timeout/worker ended".into());
        }
        thread::sleep(Duration::from_millis(1));
    };
    let receipt = match receipts.recv_timeout(WAIT) {
        Ok(receipt) => receipt,
        Err(_) => {
            return Ok(
                json!({"ordinal":ordinal,"warmup":ordinal==1,"status":"failed",
            "error":"missing profile receipt","engineError":completed.result.as_ref().err().map(ToString::to_string),
            "session":completed.session,"generation":completed.generation,"jobId":completed.job_id,
            "expectedRequest":request_json(&expected),"requestState":gate_json(gate),"profile":null}),
            );
        }
    };
    let evidence = profile(&receipt);
    let cpu_valid = match (
        &receipt.profile.thread_before,
        &receipt.profile.thread_after,
    ) {
        (Ok(before), Ok(after)) => {
            after.user_cpu >= before.user_cpu && after.system_cpu >= before.system_cpu
        }
        _ => false,
    };
    let valid = cpu_valid
        && receipt.request == expected
        && receipt.request.matches_completed(
            completed.session,
            completed.generation,
            completed.job_id,
            &completed.sample_range,
            &completed.timeline_range,
        )
        && receipt.thread_name.as_deref() == Some("local-inference")
        && receipt.thread_id != main_thread
        && worker_thread
            .as_ref()
            .is_none_or(|id| id == &receipt.thread_id)
        && receipt.profile.error.is_none()
        && receipt.profile.thread_before.is_ok()
        && receipt.profile.thread_after.is_ok();
    if valid {
        gate.acknowledge(&receipt.request)?;
    }
    if worker_thread.is_none() {
        *worker_thread = Some(receipt.thread_id.clone());
    }
    let (status, error, text_bytes, text_sha) = match completed.result {
        Ok(text) => (
            "ok",
            None,
            Some(text.len()),
            Some(input::digest(text.as_bytes())),
        ),
        Err(error) => ("failed", Some(error.to_string()), None, None),
    };
    Ok(
        json!({"ordinal":ordinal,"warmup":ordinal==1,"session":completed.session,"generation":completed.generation,
        "jobId":completed.job_id,"status":if valid {status} else {"invalid_profile"},
        "error":if valid {error.clone()} else {Some("profile identity/snapshot/evidence invalid".into())},
        "engineError":error,"profileIdentityValid":valid,
        "textBytes":text_bytes,"textSha256":text_sha,"profile":evidence}),
    )
}
fn main() -> Result<(), Error> {
    let Some(Options {
        model,
        wav,
        language,
        parameters,
        output,
    }) = parse_args(std::env::args_os().skip(1).take(7).collect())?
    else {
        print!("{}", String::from_utf8(parameter_bytes()?)?);
        return Ok(());
    };
    let frozen = input::bounded_read(&parameters, 1024 * 1024)?;
    if frozen != parameter_bytes()? {
        return Err("parameters do not match this compiled diagnostic".into());
    }
    if output.try_exists()? {
        return Err("fresh output required".into());
    }
    let bytes = input::bounded_read(&wav, input::MAX_AUDIO)?;
    let declaration = {
        let reader = hound::WavReader::new(Cursor::new(&bytes))?;
        input::Audio {
            path: "authorized-local-fixture.wav".into(),
            sha256: input::digest(&bytes),
            frames: reader.duration(),
            sample_rate: reader.spec().sample_rate,
        }
    };
    let source_pcm = input::decode_audio(&bytes, &declaration)?;
    drop(bytes);
    let (prepared_pcm, pcm_preparation) = prepare_pcm(&source_pcm)?;
    drop(source_pcm);
    let audio = Arc::new(prepared_pcm);
    let exe = std::env::current_exe()?;
    let binary_sha = input::digest(&input::bounded_read(&exe, 256 * 1024 * 1024)?);
    let main_thread = format!("{:?}", thread::current().id());
    let load = Instant::now();
    let engine = Engine::load(&model, REFERENCE)?;
    let load_millis = load.elapsed().as_millis();
    let (sender, receipts) = mpsc::sync_channel(1);
    let gate = Arc::new(RequestGate::default());
    let service = InferenceService::with_decoder(ProfiledDecoder {
        engine,
        sender,
        gate: Arc::clone(&gate),
    })?;
    let mut rows = Vec::with_capacity(RUNS as usize);
    let mut failure = wait_ready(&service).err().map(|e| e.to_string());
    let mut identity = WorkerIdentity {
        main_thread: &main_thread,
        worker_thread: None,
        gate: &gate,
    };
    for ordinal in 1..=RUNS {
        if failure.is_some() {
            rows.push(json!({"ordinal":ordinal,"warmup":ordinal==1,"status":"unavailable","error":failure,"profile":null}));
            continue;
        }
        match one(
            &service,
            &receipts,
            Arc::clone(&audio),
            language,
            ordinal,
            &mut identity,
        ) {
            Ok(row) => {
                if row["status"] != "ok" {
                    failure = Some("diagnostic row failed".into());
                }
                rows.push(row);
            }
            Err(error) => {
                failure = Some(error.to_string());
                rows.push(json!({"ordinal":ordinal,"warmup":ordinal==1,
                "status":"failed","error":failure,"profile":null}));
            }
        }
    }
    // Cleanup is off any GUI; native calls are not interruptible. This explicit
    // join is not represented as bounded native cancellation.
    let cleanup = service.shutdown().err().map(|e| e.to_string());
    while let Ok(extra) = receipts.try_recv() {
        failure = Some("extra/late receipt after cleanup".into());
        rows.push(json!({"status":"late_profile","profile":profile(&extra)}));
    }
    let passed = failure.is_none() && cleanup.is_none();
    let report = json!({"schemaVersion":1,"kind":"engine-phase-diagnostic","qualified":false,"L":null,
        "passed":passed,"error":failure,"sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "processId":std::process::id(),"binarySha256":binary_sha,
        "build":{"architecture":std::env::consts::ARCH,"debugAssertions":cfg!(debug_assertions)},"parameters":template()?,
        "parameterDocumentSha256":input::digest(&frozen),"fixtureSha256":declaration.sha256,
        "inputFrames":audio.len(),"pcmPreparation":pcm_preparation,"requestStateAfterJoin":gate_json(&gate),"language":language.code(),"model":REFERENCE.id(),"modelSha256":REFERENCE.sha256(),
        "loadMillis":load_millis,"mainThreadId":main_thread,"actualWorkerThreadId":identity.worker_thread,
        "cleanup":{"joined":cleanup.is_none(),"error":cleanup},"runs":rows,
        "limits":["One warmup plus three file calls, no capture/pacing/stop clock or L qualification.",
          "Warm Engine initialized on CLI main then moved into the actual existing InferenceService worker.",
          "Snapshots observe executing worker only, sequential public queries, no continuous or effective QoS guarantee.",
          "All phase values are wall time; None means not completed, never inferred zero. No backend utilization attribution."]});
    publish(&output, &serde_json::to_vec_pretty(&report)?)?;
    if !passed {
        return Err("diagnostic failed; see private content-free evidence".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct ControlledEngine {
        fail: bool,
    }
    impl ProfileEngine for ControlledEngine {
        fn profile(
            &mut self,
            audio: &[f32],
            language: Language,
        ) -> everlast_dictation::transcription::ProfiledTranscription {
            assert_eq!(audio, &[0.125; 3200]);
            assert_eq!(language, Language::English);
            let before = macos_integration::thread_observation::current();
            let after = macos_integration::thread_observation::current();
            everlast_dictation::transcription::ProfiledTranscription {
                result: if self.fail {
                    Err(InferenceError::Decode)
                } else {
                    Ok("controlled".into())
                },
                profile: EngineProfile {
                    phases: [None; 7],
                    total_elapsed: Duration::ZERO,
                    thread_before: before,
                    thread_after: after,
                    error: None,
                },
            }
        }
    }
    #[test]
    fn actual_service_worker_keeps_pcm_identity_and_error_profile() -> Result<(), Error> {
        for fail in [false, true] {
            let (sender, receipts) = mpsc::sync_channel(1);
            let gate = Arc::new(RequestGate::default());
            let service = InferenceService::with_decoder(ProfiledDecoder {
                engine: ControlledEngine { fail },
                sender,
                gate: Arc::clone(&gate),
            })?;
            wait_ready(&service)?;
            let main = format!("{:?}", thread::current().id());
            let mut identity = WorkerIdentity {
                main_thread: &main,
                worker_thread: None,
                gate: &gate,
            };
            let outcome = one(
                &service,
                &receipts,
                Arc::new(vec![0.125; 3200]),
                Language::English,
                1,
                &mut identity,
            );
            service.shutdown()?;
            let row = outcome?;
            assert_eq!(row["status"], if fail { "failed" } else { "ok" });
            assert_eq!(row["profile"]["probeCallerThreadName"], "local-inference");
            assert_ne!(row["profile"]["probeCallerThreadId"], main);
            assert_eq!(
                row["profile"]["nativeFullNanos"],
                Value::Null,
                "the controlled engine must not invent a native call"
            );
            assert_eq!(
                row["textSha256"],
                if fail {
                    Value::Null
                } else {
                    json!(input::digest(b"controlled"))
                }
            );
        }
        Ok(())
    }
    #[test]
    fn actual_audio_processor_flush_keeps_partial_tail_and_exact_f32_bytes() -> Result<(), Error> {
        let mut samples: Vec<f32> = (0..3517)
            .map(|i| ((i % 97) as f32 - 48.0) / 32768.0)
            .collect();
        samples[3516] = 0.8125;
        let (prepared, evidence) = prepare_pcm(&samples)?;
        assert_eq!(prepared, samples);
        assert_eq!(evidence["preparedFrames"], 3517);
        assert_eq!(
            evidence["sourceFloat32LeSha256"],
            evidence["preparedFloat32LeSha256"]
        );
        assert_eq!(evidence["exactFramesAndBytes"], true);
        assert!(prepare_pcm(&[f32::NAN]).is_err());
        Ok(())
    }
    #[test]
    fn printed_source_bindings_have_flat_sha_strings_including_all_new_dependencies()
    -> Result<(), Error> {
        let parameters = template()?;
        let baseline = parameters["baselineSourceHashes"]
            .as_object()
            .ok_or("missing baseline source map")?;
        assert_eq!(baseline.len(), 35);
        for group in [
            baseline,
            parameters["sourceHashes"]
                .as_object()
                .ok_or("missing diagnostic source map")?,
        ] {
            for binding in group.values() {
                assert!(binding["filename"].as_str().is_some_and(|v| !v.is_empty()));
                assert!(
                    binding["sha256"]
                        .as_str()
                        .is_some_and(|v| v.len() == 64 && v.bytes().all(|v| v.is_ascii_hexdigit()))
                );
            }
        }
        assert_eq!(
            parameters["sourceHashes"]["request"]["filename"],
            "src-tauri/examples/engine_profile_probe/request.rs"
        );
        assert_eq!(
            parameters["sourceHashes"]["cargoConfig"]["filename"],
            ".cargo/config.toml"
        );
        Ok(())
    }
    #[test]
    fn strict_cli_does_not_treat_unknown_flags_as_a_profile_request() {
        assert!(parse_args(vec!["--print-parameters".into()]).is_ok_and(|v| v.is_none()));
        assert!(parse_args(vec!["--unknown".into()]).is_err());
        assert!(
            parse_args(vec![
                "m".into(),
                "w".into(),
                "fr".into(),
                "p".into(),
                "o".into()
            ])
            .is_err()
        );
        assert!(
            parse_args(vec![
                "m".into(),
                "w".into(),
                "de".into(),
                "p".into(),
                "o".into(),
                "extra".into()
            ])
            .is_err()
        );
    }
}
