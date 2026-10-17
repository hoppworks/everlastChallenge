//! Explicit, model-free publisher scheduling diagnostic. Never a replay default.
use super::{
    input,
    runner::{self, Run},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use dictation_core::audio::AudioFormat;
use everlast_dictation::audio::{CaptureHandle, replay::ReplayInput};
use macos_integration::realtime::{self, PolicyError, PolicySnapshot, PublisherReport};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::Arc,
    thread::JoinHandle,
    time::{Duration, Instant},
};
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 16_000,
    channels: 1,
};
const INPUT_FRAMES: usize = 480_000;
const PUBLICATIONS: usize = 1_500;
const READY_BUDGET: Duration = Duration::from_secs(10);
const FINISH_BUDGET: Duration = Duration::from_secs(10);

use macos_integration::realtime::PublisherPolicy;
use serde::Serialize;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeResult {
    pub mode: String,
    pub status: String,
    pub errors: Vec<String>,
    pub publisher_joined: bool,
    pub audio: Option<Run>,
    pub policy: Option<Value>,
    pub before_feed: Option<Value>,
    pub after_feed: Option<Value>,
    pub outlier_policy: Vec<Value>,
}
impl ModeResult {
    fn can_continue(&self) -> bool {
        self.errors.is_empty()
            && self.publisher_joined
            && self.audio.as_ref().is_some_and(Run::owner_valid)
    }
}

trait Backend {
    fn spawn(&mut self, policy: PublisherPolicy) -> Result<(), String>;
    fn ready(&mut self) -> Result<(), String>;
    fn capture(&mut self) -> Result<(), String>;
    fn submit(&mut self) -> Result<(), String>;
    fn close_input(&mut self);
    fn join(&mut self) -> Result<(), String>;
    fn finish(&mut self) -> ModeResult;
}

fn run_mode(backend: &mut impl Backend, policy: PublisherPolicy) -> ModeResult {
    let mut errors = Vec::new();
    let spawned = match backend.spawn(policy) {
        Ok(()) => true,
        Err(error) => {
            errors.push(error);
            false
        }
    };
    if spawned {
        let input = backend
            .ready()
            .and_then(|()| backend.capture())
            .and_then(|()| backend.submit());
        if let Err(error) = input {
            errors.push(error);
        }
    }
    backend.close_input();
    if spawned && let Err(error) = backend.join() {
        errors.push(error);
    }
    let mut result = backend.finish();
    result.errors.extend(errors);
    result.mode = policy_name(policy).into();
    result.status = if !result.can_continue() {
        "failed"
    } else if result
        .audio
        .as_ref()
        .is_some_and(|audio| audio.timing_valid)
    {
        "completed"
    } else {
        "invalid_setup"
    }
    .into();
    result
}

fn policy_name(policy: PublisherPolicy) -> &'static str {
    match policy {
        PublisherPolicy::UserInteractive => "userInteractive",
        PublisherPolicy::TimeConstrained => "timeConstrained",
    }
}
fn snapshot_value(snapshot: PolicySnapshot) -> Value {
    let tc = snapshot.time_constraint;
    json!({"timeConstraint":{"periodTicks":tc.period_ticks,"computationTicks":tc.computation_ticks,
        "constraintTicks":tc.constraint_ticks,"preemptible":tc.preemptible},
        "defaultPolicy":snapshot.default_policy,"basicPolicy":snapshot.basic_policy,
        "cpu":{"userNanos":snapshot.cpu.user.as_nanos(),"systemNanos":snapshot.cpu.system.as_nanos()}})
}
fn validate_snapshot(
    snapshot: PolicySnapshot,
    expected: Option<realtime::TimeConstraint>,
    point: &str,
    errors: &mut Vec<String>,
) {
    if let Some(expected) = expected
        && (snapshot.default_policy
            || snapshot.basic_policy != 2
            || snapshot.time_constraint != expected)
    {
        errors.push(format!("{point}: observed time-constraint policy differs from request (defaultPolicy={}, basicPolicy={})",snapshot.default_policy,snapshot.basic_policy));
    }
}
fn read_value(
    read: Result<PolicySnapshot, PolicyError>,
    errors: &mut Vec<String>,
    expected: Option<realtime::TimeConstraint>,
    point: &str,
) -> Value {
    match read {
        Ok(snapshot) => {
            validate_snapshot(snapshot, expected, point, errors);
            json!({"snapshot":snapshot_value(snapshot),"error":null})
        }
        Err(error) => {
            let error = error.to_string();
            errors.push(error.clone());
            json!({"snapshot":null,"error":error})
        }
    }
}
struct Input {
    source: ReplayInput,
    handle: Arc<CaptureHandle>,
    samples: Arc<Vec<f32>>,
}
struct Outlier {
    end_frame: usize,
    after_publish_nanos: u128,
    read: Result<PolicySnapshot, PolicyError>,
}
#[derive(Default)]
struct ChildOutput {
    audio: Option<Run>,
    started: Option<Instant>,
    before: Option<Result<PolicySnapshot, PolicyError>>,
    after: Option<Result<PolicySnapshot, PolicyError>>,
    outliers: Vec<Outlier>,
    error: Option<String>,
}
#[derive(PartialEq, Eq)]
enum SnapshotPoint {
    BeforeFeed,
    AfterFeed,
    Outlier,
}
fn publish_input(
    input: Input,
    output: &mut ChildOutput,
    mut snapshot: impl FnMut(SnapshotPoint) -> Result<PolicySnapshot, PolicyError>,
) {
    output.before = Some(snapshot(SnapshotPoint::BeforeFeed));
    let (audio, started) =
        runner::publish(input.source, &input.handle, &input.samples, |publication| {
            output.outliers.push(Outlier {
                end_frame: publication.end_frame,
                after_publish_nanos: publication.after_publish_nanos,
                read: snapshot(SnapshotPoint::Outlier),
            });
        });
    // Stop and port release precede this observation; drain/join stays on parent.
    output.after = Some(snapshot(SnapshotPoint::AfterFeed));
    output.audio = Some(audio);
    output.started = Some(started);
}

struct Native {
    samples: Arc<Vec<f32>>,
    ready: Option<Receiver<()>>,
    send: Option<Sender<Input>>,
    input: Option<ReplayInput>,
    handle: Option<Arc<CaptureHandle>>,
    child: Option<JoinHandle<PublisherReport<ChildOutput>>>,
    report: Option<PublisherReport<ChildOutput>>,
    joined: bool,
}
impl Native {
    fn new(samples: Arc<Vec<f32>>) -> Self {
        Self {
            samples,
            ready: None,
            send: None,
            input: None,
            handle: None,
            child: None,
            report: None,
            joined: false,
        }
    }
}
impl Backend for Native {
    fn spawn(&mut self, policy: PublisherPolicy) -> Result<(), String> {
        // Allocate outlier storage on the normal parent before policy entry.
        let mut outliers = Vec::new();
        outliers
            .try_reserve_exact(PUBLICATIONS)
            .map_err(|error| error.to_string())?;
        let (ack, ready) = bounded(1);
        let (send, receive) = bounded::<Input>(1);
        let child = realtime::spawn_publisher(policy, move |observer| {
            let mut output = ChildOutput {
                outliers,
                ..ChildOutput::default()
            };
            if ack.try_send(()).is_err() {
                output.error = Some("parent readiness receiver closed".into());
                return output;
            }
            let input = match receive.recv_timeout(READY_BUDGET) {
                Ok(input) => input,
                Err(error) => {
                    output.error = Some(format!("publisher input unavailable: {error}"));
                    return output;
                }
            };
            publish_input(input, &mut output, |_| observer.snapshot());
            output
        })
        .map_err(|error| format!("publisher spawn failed: {error}"))?;
        self.ready = Some(ready);
        self.send = Some(send);
        self.child = Some(child);
        Ok(())
    }
    fn ready(&mut self) -> Result<(), String> {
        self.ready
            .as_ref()
            .ok_or("missing publisher readiness channel")?
            .recv_timeout(READY_BUDGET)
            .map_err(|error| format!("publisher not ready: {error}"))
    }
    fn capture(&mut self) -> Result<(), String> {
        let (send, receive) = bounded(1);
        // This normal parent starts the DSP only AFTER publisher policy ACK.
        self.handle = Some(Arc::new(
            CaptureHandle::start_replay(1, FORMAT, send).map_err(|error| error.to_string())?,
        ));
        self.input = Some(
            receive
                .recv_timeout(READY_BUDGET)
                .map_err(|error| format!("capture input not ready: {error}"))?,
        );
        Ok(())
    }
    fn submit(&mut self) -> Result<(), String> {
        let source = self.input.take().ok_or("capture input missing")?;
        let handle = Arc::clone(self.handle.as_ref().ok_or("capture handle missing")?);
        self.send
            .as_ref()
            .ok_or("publisher input channel missing")?
            .try_send(Input {
                source,
                handle,
                samples: Arc::clone(&self.samples),
            })
            .map_err(|error| format!("publisher input rejected: {error}"))
    }
    fn close_input(&mut self) {
        self.send.take();
    }
    fn join(&mut self) -> Result<(), String> {
        if let Some(child) = self.child.take() {
            match child.join() {
                Ok(report) => {
                    self.joined = true;
                    self.report = Some(report);
                    Ok(())
                }
                Err(_) => Err("publisher panicked; join failed".into()),
            }
        } else {
            Err("publisher join handle missing".into())
        }
    }
    fn finish(&mut self) -> ModeResult {
        let mut result = ModeResult {
            publisher_joined: self.joined,
            ..ModeResult::default()
        };
        let mut started = None;
        if let Some(report) = self.report.take() {
            // Admission's pre-policy `before` is intentionally not classified.
            // Baseline after QoS cleanup may also legitimately be timeshare.
            let expected = if report.policy == PublisherPolicy::TimeConstrained {
                if report.requested.is_none() {
                    result
                        .errors
                        .push("timeConstrained: requested policy missing".into());
                }
                report.requested
            } else {
                None
            };
            for (point, snapshot) in [("configured", report.configured), ("post", report.after)] {
                if let Some(snapshot) = snapshot {
                    validate_snapshot(snapshot, expected, point, &mut result.errors);
                }
            }
            result.policy = Some(json!({"policy":policy_name(report.policy),
                "requested":report.requested.map(|tc|json!({"periodTicks":tc.period_ticks,"computationTicks":tc.computation_ticks,"constraintTicks":tc.constraint_ticks,"preemptible":tc.preemptible})),
                "before":report.before.map(snapshot_value),"configured":report.configured.map(snapshot_value),"after":report.after.map(snapshot_value),
                "setupError":report.setup_error.map(|e|e.to_string()),"postError":report.post_error.map(|e|e.to_string()),"finishError":report.finish_error.map(|e|e.to_string())}));
            for error in [report.setup_error, report.post_error, report.finish_error]
                .into_iter()
                .flatten()
            {
                result.errors.push(error.to_string());
            }
            if let Some(output) = report.output {
                result.audio = output.audio;
                started = output.started;
                if let Some(error) = output.error {
                    result.errors.push(error);
                }
                result.before_feed = output
                    .before
                    .map(|r| read_value(r, &mut result.errors, expected, "beforeFeed"));
                result.after_feed = output
                    .after
                    .map(|r| read_value(r, &mut result.errors, expected, "afterFeed"));
                result.outlier_policy=output.outliers.into_iter().map(|o|json!({"endFrame":o.end_frame,"afterPublishNanos":o.after_publish_nanos,"policy":read_value(o.read,&mut result.errors,expected,"outlier")})).collect();
            }
        }
        // This stage runs after publisher join, on the normal parent.
        if let Some(handle) = self.handle.take() {
            if started.is_none() {
                handle.request_stop();
            }
            drop(self.input.take());
            match Arc::try_unwrap(handle) {
                Ok(handle) => {
                    let mut audio = result.audio.take().unwrap_or_default();
                    if let Some(started) = started {
                        runner::finish(handle, &mut audio, started, &self.samples, FINISH_BUDGET);
                    } else {
                        audio.observation_failure =
                            Some("publisher did not return a feed clock".into());
                        runner::finish(
                            handle,
                            &mut audio,
                            Instant::now(),
                            &self.samples,
                            FINISH_BUDGET,
                        );
                    }
                    result.audio = Some(audio);
                }
                Err(_) => result
                    .errors
                    .push("publisher retained the audio handle after join".into()),
            }
        }
        result
    }
}

pub(super) fn parameters() -> Value {
    let source_hashes = json!({
            "probe":input::digest(include_bytes!("../paced_audio_probe.rs")),"runner":input::digest(include_bytes!("runner.rs")),"scheduling":input::digest(include_bytes!("scheduled.rs")),
            "input":input::digest(include_bytes!("../corpus_probe/input.rs")),"capture":input::digest(include_bytes!("../../src/audio.rs")),
            "replay":input::digest(include_bytes!("../../src/audio/replay.rs")),"clock":input::digest(include_bytes!("../../src/audio/replay/clock.rs")),"schedule":input::digest(include_bytes!("../../src/audio/replay/schedule.rs")),
            "dsp":input::digest(include_bytes!("../../../crates/dictation-core/src/audio.rs")),"coreManifest":input::digest(include_bytes!("../../../crates/dictation-core/Cargo.toml")),
            "realtime":input::digest(include_bytes!("../../../crates/macos-integration/src/realtime.rs")),"qos":input::digest(include_bytes!("../../../crates/macos-integration/src/qos.rs")),
            "nativeLibrary":input::digest(include_bytes!("../../../crates/macos-integration/src/lib.rs")),"nativeManifest":input::digest(include_bytes!("../../../crates/macos-integration/Cargo.toml")),
            "hostManifest":input::digest(include_bytes!("../../Cargo.toml")),"workspaceManifest":input::digest(include_bytes!("../../../Cargo.toml")),"cargoLock":input::digest(include_bytes!("../../../Cargo.lock")),"cargoConfig":input::digest(include_bytes!("../../../.cargo/config.toml"))});
    json!({"schemaVersion":1,"configurationId":"publisher-scheduling-relative-sleep-v1",
        "modes":["userInteractive","timeConstrained"],"inputFrames":INPUT_FRAMES,"sampleRate":16000,"channels":1,"sampleFormat":"PCM16",
        "chunkMillis":20,"jitterToleranceNanos":10000000,"maximumSleepNanos":1000000,"finalSpinNanos":250000,
        "maximumOutlierSnapshotsPerMode":PUBLICATIONS,"readyBudgetNanos":READY_BUDGET.as_nanos(),"finishBudgetNanos":FINISH_BUDGET.as_nanos(),
        "timeConstraint":{"periodNanos":realtime::PERIOD.as_nanos(),"computationNanos":realtime::COMPUTATION.as_nanos(),"constraintNanos":realtime::CONSTRAINT.as_nanos(),"preemptible":true},
        "threadOwnership":"policy only in newly owned child; Ready ACK precedes normal-parent capture/DSP startup; parent observes audio only after publisher join",
        "feed":"same frozen relative-sleep clockfeed in both modes; no MachWaiter, no policy fallback, all publications retained",
        "policyObservations":"raw before/configured/after and before/after feed; point-in-time outlier snapshot after each invalid publication; not continuous demotion evidence",
        "policyClassification":"timeConstrained requires requested policy and every observed configured/beforeFeed/afterFeed/outlier/post snapshot to match its ticks, defaultPolicy=false and basicPolicy=2; deviations are retained failures after feed, never hidden or interrupted; initial pre-policy/baseline snapshots excluded",
        "failurePolicy":"retain timing-only invalid mode and continue only after verified owner cleanup; OS/setup/join/observer/audio failure makes following mode unavailable",
        "noModel":true,"qualified":false,
        "build":{"debugAssertions":cfg!(debug_assertions),"architecture":std::env::consts::ARCH,"os":std::env::consts::OS},
        "sourceHashes":source_hashes
    })
}
fn validate_parameters(bytes: &[u8]) -> Result<Value, Box<dyn std::error::Error>> {
    let actual: Value = serde_json::from_slice(bytes)?;
    if actual != parameters() {
        return Err("parameters differ from this compiled scheduling configuration".into());
    }
    Ok(actual)
}
fn prepare_output(path: &Path) -> Result<tempfile::NamedTempFile, Box<dyn std::error::Error>> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Err("report exists; refusing to overwrite or start".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file = tempfile::Builder::new()
        .prefix(".publisher-scheduling-")
        .tempfile_in(parent)?;
    use std::os::unix::fs::PermissionsExt;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}
fn modes<B: Backend>(mut make: impl FnMut() -> B) -> Vec<ModeResult> {
    let first = run_mode(&mut make(), PublisherPolicy::UserInteractive);
    let second = if first.can_continue() {
        run_mode(&mut make(), PublisherPolicy::TimeConstrained)
    } else {
        ModeResult {
            mode: "timeConstrained".into(),
            status: "unavailable".into(),
            errors: vec!["preceding mode failed; no retry".into()],
            ..ModeResult::default()
        }
    };
    vec![first, second]
}
fn publish(
    mut staged: tempfile::NamedTempFile,
    output: &Path,
    report: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    serde_json::to_writer_pretty(&mut staged, report)?;
    staged.write_all(b"\n")?;
    staged.as_file().sync_all()?;
    staged.persist_noclobber(output)?;
    Ok(())
}

pub(super) fn run(
    wav: &Path,
    parameters_path: &Path,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let parameter_bytes = input::bounded_read(parameters_path, 1_048_576)?;
    let configuration = validate_parameters(&parameter_bytes)?;
    let staged = prepare_output(output)?;
    let bytes = input::bounded_read(wav, input::MAX_AUDIO)?;
    let audio = input::Audio {
        path: "authorized-fixed-fixture.wav".into(),
        sha256: input::digest(&bytes),
        frames: INPUT_FRAMES as u32,
        sample_rate: 16000,
    };
    let samples = Arc::new(input::decode_audio(&bytes, &audio)?);
    let source_revision = std::env::var("EVERLAST_BENCHMARK_REVISION").ok();
    if source_revision.as_ref().is_some_and(|s| s.len() > 256) {
        return Err("source revision exceeds bound".into());
    }
    let runs = modes(|| Native::new(Arc::clone(&samples)));
    let completed = runs.iter().all(|r| r.status == "completed");
    let report = json!({"schemaVersion":1,"kind":"model-free-publisher-scheduling-comparison","parameters":configuration,
        "parameterDocumentSha256":input::digest(&parameter_bytes),"sourceRevision":source_revision,"pid":std::process::id(),
        "fixtureSha256":audio.sha256,"inputFrames":INPUT_FRAMES,"runs":runs,"completed":completed,"qualified":false,"L":null,
        "limits":["Fixed userInteractive-then-timeConstrained order, one30s trial per mode, no causal or realtime guarantee.",
            "Policy mutation applies only to the newly owned publisher thread, which exits before parent audio observation.",
            "Point-in-time raw policy/CPU observations do not prove continuous absence of demotion; outlier observation overhead remains in later timing.",
            "Model-free ring/DSP validation; no microphone, ASR, L, installed-app or corpus qualification; replay defaults unchanged."]});
    publish(staged, output, &report)?;
    if !completed {
        return Err("publisher scheduling comparison invalid; complete evidence retained".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        events: Vec<&'static str>,
        ready_error: bool,
        join_error: bool,
    }
    impl Backend for Fake {
        fn spawn(&mut self, _: PublisherPolicy) -> Result<(), String> {
            self.events.push("spawn");
            Ok(())
        }
        fn ready(&mut self) -> Result<(), String> {
            self.events.push("ready");
            if self.ready_error {
                Err("policy setup failed".into())
            } else {
                Ok(())
            }
        }
        fn capture(&mut self) -> Result<(), String> {
            self.events.push("capture");
            Ok(())
        }
        fn submit(&mut self) -> Result<(), String> {
            self.events.push("submit");
            Ok(())
        }
        fn close_input(&mut self) {
            self.events.push("close");
        }
        fn join(&mut self) -> Result<(), String> {
            self.events.push("join");
            if self.join_error {
                Err("publisher join failed".into())
            } else {
                Ok(())
            }
        }
        fn finish(&mut self) -> ModeResult {
            self.events.push("finish");
            ModeResult::default()
        }
    }
    #[test]
    fn scheduling_ready_precedes_capture_and_join_precedes_parent_observation() {
        let mut backend = Fake {
            events: Vec::new(),
            ready_error: false,
            join_error: false,
        };
        run_mode(&mut backend, PublisherPolicy::UserInteractive);
        assert_eq!(
            backend.events,
            [
                "spawn", "ready", "capture", "submit", "close", "join", "finish"
            ]
        );
    }
    #[test]
    fn setup_and_join_errors_survive_without_starting_capture() {
        let mut backend = Fake {
            events: Vec::new(),
            ready_error: true,
            join_error: true,
        };
        let result = run_mode(&mut backend, PublisherPolicy::TimeConstrained);
        assert_eq!(
            backend.events,
            ["spawn", "ready", "close", "join", "finish"]
        );
        assert_eq!(
            result.errors,
            ["policy setup failed", "publisher join failed"]
        );
        assert!(!result.can_continue());
    }
    #[test]
    fn timing_only_failure_allows_following_mode_after_verified_cleanup() {
        let mut result = ModeResult {
            publisher_joined: true,
            audio: Some(Run {
                joined: true,
                started_events: 1,
                finished_events: 1,
                started_clock_verified: true,
                input_output_samples_equal: true,
                stop_requests: 1,
                timing_valid: false,
                ..Run::default()
            }),
            ..ModeResult::default()
        };
        assert!(result.can_continue());
        result.errors.push("policy post-readback failed".into());
        assert!(!result.can_continue());
        result.errors.clear();
        result.publisher_joined = false;
        assert!(!result.can_continue());
    }
    #[test]
    fn after_feed_observer_sees_finished_audio_without_parent_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let (send, receive) = bounded(1);
        let handle = Arc::new(CaptureHandle::start_replay(1, FORMAT, send)?);
        let source = receive.recv_timeout(Duration::from_secs(2))?;
        let input = Input {
            source,
            handle: Arc::clone(&handle),
            samples: Arc::new(vec![0.125; 320]),
        };
        let mut output = ChildOutput {
            outliers: Vec::with_capacity(PUBLICATIONS),
            ..ChildOutput::default()
        };
        let mut saw_finish = false;
        publish_input(input, &mut output, |point| {
            if point == SnapshotPoint::AfterFeed {
                let deadline = Instant::now() + Duration::from_millis(200);
                while Instant::now() < deadline {
                    if let Some(everlast_dictation::audio::CaptureEvent::Finished {
                        result: Ok(audio),
                        ..
                    }) = handle.next_event()
                    {
                        saw_finish = audio == [0.125; 320];
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Err(PolicyError::InvalidReply {
                operation: "test observer has no native policy",
            })
        });
        // Always stop/join before asserting, also on a failing ordering test.
        handle.request_stop();
        let handle = Arc::try_unwrap(handle).map_err(|_| "capture handle retained")?;
        handle.join()?;
        assert!(
            saw_finish,
            "after-feed observation ran before Stop/port release"
        );
        assert_eq!(output.audio.as_ref().map(|run| run.stop_requests), Some(1));
        Ok(())
    }

    #[test]
    fn parameter_and_audio_validation_precede_any_policy_thread()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let parameters_path = dir.path().join("params.json");
        let wav = dir.path().join("input.wav");
        let output = dir.path().join("out.json");
        let mut changed = parameters();
        changed["maximumSleepNanos"] = json!(2_000_000);
        std::fs::write(&parameters_path, serde_json::to_vec(&changed)?)?;
        assert!(run(&wav, &parameters_path, &output).is_err());
        assert!(!output.exists());
        std::fs::write(&parameters_path, serde_json::to_vec(&parameters())?)?;
        // Valid WAV but only one frame: must fail before spawn, not pace/fill30s.
        let mut writer = hound::WavWriter::create(
            &wav,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        writer.write_sample(1_i16)?;
        writer.finalize()?;
        let error = run(&wav, &parameters_path, &output)
            .err()
            .ok_or("one-frame input incorrectly accepted")?;
        assert_eq!(error.to_string(), "audio frame count mismatch");
        assert!(!output.exists());
        Ok(())
    }
    #[test]
    fn report_is_private_atomic_and_never_replaces_existing_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.json");
        let staged = prepare_output(&path)?;
        std::fs::write(&path, b"original")?;
        assert!(publish(staged, &path, &json!({"changed":true})).is_err());
        assert_eq!(std::fs::read(&path)?, b"original");
        assert!(prepare_output(&path).is_err());
        let path = dir.path().join("new.json");
        publish(prepare_output(&path)?, &path, &json!({"retained":true}))?;
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        Ok(())
    }
    #[test]
    fn failed_mode_makes_second_mode_unavailable_without_second_spawn() {
        let mut made = 0;
        let runs = modes(|| {
            made += 1;
            Fake {
                events: Vec::new(),
                ready_error: true,
                join_error: false,
            }
        });
        assert_eq!(made, 1);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].status, "failed");
        assert_eq!(runs[1].status, "unavailable");
        assert_eq!(runs[0].errors, ["policy setup failed"]);
    }
    #[test]
    fn post_policy_failure_preserves_returned_feed_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = Native::new(Arc::new(Vec::new()));
        backend.joined = true;
        backend.report = Some(PublisherReport {
            policy: PublisherPolicy::UserInteractive,
            requested: None,
            before: None,
            configured: None,
            after: None,
            setup_error: None,
            post_error: Some(PolicyError::Os {
                operation: "thread_info",
                code: 5,
            }),
            finish_error: None,
            output: Some(ChildOutput {
                audio: Some(Run {
                    timing_valid: false,
                    publications: vec![everlast_dictation::audio::replay::clock::Publication {
                        start_frame: 0,
                        end_frame: 320,
                        planned_nanos: 20_000_000,
                        before_publish_nanos: 32_000_000,
                        after_publish_nanos: 32_000_010,
                        deviation_nanos: 12_000_010,
                        timing: "late",
                        diagnostics: None,
                    }],
                    ..Run::default()
                }),
                ..ChildOutput::default()
            }),
        });
        let result = backend.finish();
        let value = serde_json::to_value(&result)?;
        assert_eq!(
            value["audio"]["publications"][0]["deviationNanos"],
            12_000_010_u64
        );
        assert_eq!(
            value["policy"]["postError"],
            "thread_info failed with OS code 5"
        );
        assert_eq!(result.errors, ["thread_info failed with OS code 5"]);
        assert!(!result.can_continue());
        Ok(())
    }
    fn admitted_snapshot() -> PolicySnapshot {
        PolicySnapshot {
            time_constraint: realtime::TimeConstraint {
                period_ticks: 480_000,
                computation_ticks: 12_000,
                constraint_ticks: 24_000,
                preemptible: true,
            },
            default_policy: false,
            basic_policy: 2,
            cpu: realtime::ThreadCpuTime {
                user: Duration::from_micros(1),
                system: Duration::from_micros(2),
            },
        }
    }
    fn finish_with_observation(
        policy: PublisherPolicy,
        point: &str,
        bad: PolicySnapshot,
        missing_requested: bool,
    ) -> ModeResult {
        let good = admitted_snapshot();
        let mut backend = Native::new(Arc::new(Vec::new()));
        backend.joined = true;
        let observed = |name: &str| if point == name { bad } else { good };
        backend.report = Some(PublisherReport {
            policy,
            requested: if missing_requested {
                None
            } else {
                Some(good.time_constraint)
            },
            before: Some(PolicySnapshot {
                default_policy: true,
                basic_policy: 1,
                ..good
            }),
            configured: Some(good),
            after: Some(observed("post")),
            setup_error: None,
            post_error: None,
            finish_error: None,
            output: Some(ChildOutput {
                before: Some(Ok(observed("beforeFeed"))),
                after: Some(Ok(observed("afterFeed"))),
                outliers: vec![Outlier {
                    end_frame: 320,
                    after_publish_nanos: 32_000_000,
                    read: Ok(observed("outlier")),
                }],
                audio: Some(Run {
                    timing_valid: true,
                    joined: true,
                    started_events: 1,
                    finished_events: 1,
                    stop_requests: 1,
                    started_clock_verified: true,
                    input_output_samples_equal: true,
                    ..Run::default()
                }),
                ..ChildOutput::default()
            }),
        });
        backend.finish()
    }
    #[test]
    fn observed_time_constraint_demotion_or_mismatch_fails_without_losing_raw_snapshots() {
        let good = admitted_snapshot();
        let bad_values = [
            PolicySnapshot {
                basic_policy: 1,
                ..good
            },
            PolicySnapshot {
                default_policy: true,
                ..good
            },
            PolicySnapshot {
                time_constraint: realtime::TimeConstraint {
                    constraint_ticks: 1,
                    ..good.time_constraint
                },
                ..good
            },
        ];
        for point in ["beforeFeed", "afterFeed", "outlier", "post"] {
            for bad in bad_values {
                let result =
                    finish_with_observation(PublisherPolicy::TimeConstrained, point, bad, false);
                assert!(
                    !result.can_continue(),
                    "observed {point} demotion/mismatch was accepted"
                );
                assert!(!result.errors.is_empty());
                let retained = match point {
                    "beforeFeed" => result.before_feed.as_ref().map(|v| &v["snapshot"]),
                    "afterFeed" => result.after_feed.as_ref().map(|v| &v["snapshot"]),
                    "outlier" => result
                        .outlier_policy
                        .first()
                        .map(|v| &v["policy"]["snapshot"]),
                    _ => result.policy.as_ref().map(|v| &v["after"]),
                };
                assert_eq!(retained, Some(&snapshot_value(bad)));
            }
        }
        assert!(
            finish_with_observation(PublisherPolicy::TimeConstrained, "none", good, false)
                .can_continue()
        );
        assert!(
            !finish_with_observation(PublisherPolicy::TimeConstrained, "none", good, true)
                .can_continue()
        );
        // Baseline may correctly read timeshare/default after its QoS scope ends.
        assert!(
            finish_with_observation(
                PublisherPolicy::UserInteractive,
                "post",
                bad_values[1],
                true
            )
            .can_continue()
        );
    }
}
