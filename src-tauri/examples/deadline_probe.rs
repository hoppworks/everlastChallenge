//! Developer-only 1-ms wait-mechanism comparison; no audio, model or spin.
//! Usage: deadline_probe --print-parameters; freeze stdout privately, then
//! deadline_probe PARAMETERS_JSON NEW_OUTPUT_JSON. Never overwrites a report.
#![forbid(unsafe_code)]
use macos_integration::{deadline::MachWaiter, qos::PublisherQos};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MODE_DURATION: Duration = Duration::from_secs(30);
const REQUEST: Duration = Duration::from_millis(1);
const OVERSHOOT_THRESHOLD: Duration = Duration::from_millis(10);
const MAX_CALLS: usize = 30_001;
const MAX_PARAMETERS: u64 = 1_048_576;

#[derive(Debug, PartialEq, Eq)]
enum Options {
    PrintParameters,
    Run {
        parameters: PathBuf,
        output: PathBuf,
    },
}
fn parse(arguments: Vec<OsString>) -> Result<Options, String> {
    if arguments.len() == 1 && arguments[0] == "--print-parameters" {
        return Ok(Options::PrintParameters);
    }
    if arguments.len() != 2
        || arguments.iter().any(|arg| {
            arg.is_empty()
                || arg.as_encoded_bytes().len() > 4096
                || arg.as_encoded_bytes().starts_with(b"--")
        })
        || arguments[0] == arguments[1]
    {
        return Err(
            "usage: deadline_probe --print-parameters | PARAMETERS_JSON NEW_OUTPUT_JSON".into(),
        );
    }
    Ok(Options::Run {
        parameters: arguments[0].clone().into(),
        output: arguments[1].clone().into(),
    })
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn parameters() -> Value {
    json!({
        "schemaVersion":1,"configurationId":"wait-mechanisms-1ms-relative-then-mach-qos-v1",
        "modes":["relative","mach"],"modeDurationNanos":MODE_DURATION.as_nanos(),
        "requestNanos":REQUEST.as_nanos(),"maximumCallsPerMode":MAX_CALLS,
        "overshootThresholdNanos":OVERSHOOT_THRESHOLD.as_nanos(),
        "thresholdComparison":"strict greater-than; diagnostic only, continue the whole planned mode",
        "modeEnd":"before each request: finish once mode elapsed >= 30s; last request may finish later",
        "clock":"std::time::Instant offsets per mode; requested Mach duration uses mach_absolute_time",
        "publisherQos":{"class":"userInteractive","relativePriority":0,"baseRequestedQosChanged":false,
            "scope":"same current thread per mode, after allocation and mechanism init, through loop; finish before summary/report"},
        "allocation":"reserve all 30001 call slots before each mode enters QoS; no loop IO",
        "failurePolicy":"retain every collected row and mode error; no retry; following mode unavailable",
        "statistics":"all rows retained including failed calls; successful-call summary explicitly scoped; no corpus jitter or L claim",
        "noAudio":true,"noModel":true,"noSpin":true,"noRealtimeGuarantee":true,
        "build":{"debugAssertions":cfg!(debug_assertions),"architecture":std::env::consts::ARCH,"os":std::env::consts::OS},
        "sourceHashes":{
            "deadline":digest(include_bytes!("../../crates/macos-integration/src/deadline.rs")),
            "qos":digest(include_bytes!("../../crates/macos-integration/src/qos.rs")),
            "nativeLibrary":digest(include_bytes!("../../crates/macos-integration/src/lib.rs")),
            "probe":digest(include_bytes!("deadline_probe.rs")),
            "cargoLock":digest(include_bytes!("../../Cargo.lock")),
            "workspaceCargo":digest(include_bytes!("../../Cargo.toml")),
            "hostCargo":digest(include_bytes!("../Cargo.toml")),
            "nativeCargo":digest(include_bytes!("../../crates/macos-integration/Cargo.toml"))
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Mode {
    Relative,
    Mach,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Call {
    index: usize,
    requested_nanos: u128,
    before_nanos: u128,
    after_nanos: Option<u128>,
    elapsed_nanos: Option<u128>,
    overshoot_nanos: Option<u128>,
    error: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    collected_calls: usize,
    successful_calls: usize,
    failed_calls: usize,
    above_threshold_calls: usize,
    successful_max_elapsed_nanos: Option<u128>,
    successful_max_overshoot_nanos: Option<u128>,
}
fn summarize(rows: &[Call]) -> Summary {
    let successful = rows.iter().filter(|row| row.error.is_none());
    Summary {
        collected_calls: rows.len(),
        successful_calls: successful.clone().count(),
        failed_calls: rows.iter().filter(|row| row.error.is_some()).count(),
        above_threshold_calls: rows
            .iter()
            .filter(|row| {
                row.overshoot_nanos
                    .is_some_and(|n| n > OVERSHOOT_THRESHOLD.as_nanos())
            })
            .count(),
        successful_max_elapsed_nanos: successful.clone().filter_map(|row| row.elapsed_nanos).max(),
        successful_max_overshoot_nanos: successful.filter_map(|row| row.overshoot_nanos).max(),
    }
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    Completed,
    Failed,
    Unavailable,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    mode: Mode,
    status: Status,
    error: Option<String>,
    qos_entered: bool,
    qos_finished: bool,
    qos_error: Option<String>,
    observed_duration_nanos: Option<u128>,
    summary: Summary,
    calls: Vec<Call>,
}
impl Run {
    fn unavailable(mode: Mode, error: String) -> Self {
        Self {
            mode,
            status: Status::Unavailable,
            error: Some(error),
            qos_entered: false,
            qos_finished: false,
            qos_error: None,
            observed_duration_nanos: None,
            summary: summarize(&[]),
            calls: Vec::new(),
        }
    }
}

// The seam substitutes clock/wait/QoS boundaries only; both real modes execute
// this same collection and failure-retention path. Tests never wait in real time.
trait Backend {
    fn initialize(&mut self, mode: Mode) -> Result<(), String>;
    fn enter_qos(&mut self) -> Result<(), String>;
    fn now(&self) -> Duration;
    fn wait(&mut self, mode: Mode, request: Duration) -> Result<(), String>;
    fn finish_qos(&mut self) -> Result<(), String>;
}
struct Native {
    origin: Instant,
    mach: Option<MachWaiter>,
    qos: Option<PublisherQos>,
}
impl Backend for Native {
    fn initialize(&mut self, mode: Mode) -> Result<(), String> {
        if mode == Mode::Mach {
            self.mach = Some(MachWaiter::new().map_err(|error| error.to_string())?);
        }
        Ok(())
    }
    fn enter_qos(&mut self) -> Result<(), String> {
        self.qos = Some(PublisherQos::enter().map_err(|error| error.to_string())?);
        Ok(())
    }
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
    fn wait(&mut self, mode: Mode, request: Duration) -> Result<(), String> {
        match mode {
            Mode::Relative => {
                std::thread::sleep(request);
                Ok(())
            }
            Mode::Mach => self
                .mach
                .as_ref()
                .ok_or("Mach waiter was not initialized")?
                .sleep(request)
                .map_err(|error| error.to_string()),
        }
    }
    fn finish_qos(&mut self) -> Result<(), String> {
        self.qos
            .take()
            .ok_or("QoS scope missing at finish")?
            .finish()
            .map_err(|error| error.to_string())
    }
}

fn collect(
    backend: &mut impl Backend,
    mode: Mode,
    rows: &mut Vec<Call>,
    duration: Duration,
    maximum: usize,
) -> Result<Duration, String> {
    let origin = backend.now();
    loop {
        let before = backend.now();
        let before_offset = before
            .checked_sub(origin)
            .ok_or("mode clock moved backwards")?;
        if before_offset >= duration {
            return Ok(before_offset);
        }
        if rows.len() >= maximum {
            return Err("call capacity reached before planned mode duration".into());
        }
        let waited = backend.wait(mode, REQUEST);
        let after = backend.now();
        let elapsed = after.checked_sub(before);
        let after_offset = after.checked_sub(origin);
        let error = waited.err().or_else(|| {
            if elapsed.is_none() || after_offset.is_none() {
                Some("wait clock moved backwards".into())
            } else if elapsed.is_some_and(|elapsed| elapsed < REQUEST) {
                Some("wait returned before requested duration".into())
            } else {
                None
            }
        });
        let failed = error.is_some();
        rows.push(Call {
            index: rows.len() + 1,
            requested_nanos: REQUEST.as_nanos(),
            before_nanos: before_offset.as_nanos(),
            after_nanos: after_offset.map(|offset| offset.as_nanos()),
            elapsed_nanos: elapsed.map(|elapsed| elapsed.as_nanos()),
            overshoot_nanos: elapsed.map(|elapsed| elapsed.saturating_sub(REQUEST).as_nanos()),
            error,
        });
        if failed {
            return Err("wait failed; see retained call error".into());
        }
    }
}
fn run_one(backend: &mut impl Backend, mode: Mode, duration: Duration, maximum: usize) -> Run {
    let mut run = Run::unavailable(mode, "mode not started".into());
    run.status = Status::Failed;
    if let Err(error) = run.calls.try_reserve_exact(maximum) {
        run.error = Some(format!("call storage reservation failed: {error}"));
        return run;
    }
    if let Err(error) = backend.initialize(mode) {
        run.error = Some(format!("mechanism initialization failed: {error}"));
        return run;
    }
    if let Err(error) = backend.enter_qos() {
        run.qos_error = Some(error);
        run.error = Some("QoS entry failed".into());
        return run;
    }
    run.qos_entered = true;
    let collected = collect(backend, mode, &mut run.calls, duration, maximum);
    let finished = backend.finish_qos();
    run.qos_finished = finished.is_ok();
    run.qos_error = finished.err();
    run.observed_duration_nanos = run.calls.last().and_then(|row| row.after_nanos);
    run.error = collected.as_ref().err().cloned();
    if let Ok(elapsed) = collected {
        run.observed_duration_nanos = Some(elapsed.as_nanos());
    }
    if run.qos_error.is_some() {
        run.error = Some(format!(
            "{}; QoS cleanup failed",
            run.error.as_deref().unwrap_or("collection completed")
        ));
    }
    run.summary = summarize(&run.calls);
    if run.error.is_none() {
        run.status = Status::Completed;
    }
    run
}
fn run_modes(backend: &mut impl Backend, duration: Duration, maximum: usize) -> [Run; 2] {
    let first = run_one(backend, Mode::Relative, duration, maximum);
    let second = if first.status == Status::Completed {
        run_one(backend, Mode::Mach, duration, maximum)
    } else {
        Run::unavailable(
            Mode::Mach,
            "preceding relative mode failed; no retry".into(),
        )
    };
    [first, second]
}

fn validate_parameters(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() as u64 > MAX_PARAMETERS {
        return Err("parameter document exceeds 1 MiB".into());
    }
    let supplied: Value = serde_json::from_slice(bytes).map_err(|_| "invalid parameter JSON")?;
    if supplied != parameters() {
        return Err("parameter document differs from this compiled configuration".into());
    }
    Ok(supplied)
}
fn prepare_output(path: &Path) -> Result<tempfile::NamedTempFile, Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Err("output already exists; refusing to run or overwrite".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file = tempfile::Builder::new()
        .prefix(".deadline-probe-")
        .tempfile_in(parent)?;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}
fn publish(
    mut file: tempfile::NamedTempFile,
    path: &Path,
    report: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer_pretty(&mut file, report)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match parse(std::env::args_os().skip(1).take(4).collect())? {
        Options::PrintParameters => println!("{}", serde_json::to_string_pretty(&parameters())?),
        Options::Run { parameters, output } => {
            let mut bytes = Vec::new();
            std::fs::File::open(parameters)?
                .take(MAX_PARAMETERS + 1)
                .read_to_end(&mut bytes)?;
            let configuration = validate_parameters(&bytes)?;
            let file = prepare_output(&output)?;
            let revision = std::env::var("EVERLAST_BENCHMARK_REVISION").ok();
            if revision
                .as_ref()
                .is_some_and(|revision| revision.len() > 256)
            {
                return Err("source revision exceeds 256 bytes".into());
            }
            let mut backend = Native {
                origin: Instant::now(),
                mach: None,
                qos: None,
            };
            let runs = run_modes(&mut backend, MODE_DURATION, MAX_CALLS);
            let completed = runs.iter().all(|run| run.status == Status::Completed);
            let threshold_passed = completed
                && runs
                    .iter()
                    .all(|run| run.summary.above_threshold_calls == 0);
            let report = json!({"schemaVersion":1,"kind":"deadline-wait-mechanism-diagnostic",
                "parameters":configuration,"parametersFileSha256":digest(&bytes),
                "sourceRevision":revision,"pid":std::process::id(),"modes":runs,
                "completed":completed,"diagnosticThresholdPassed":threshold_passed,
                "limits":["Fixed relative-then-Mach order; no order randomization or causal OS diagnosis.",
                    "This is a 1ms waiting-only diagnostic without spin, audio, models or replay policy changes.",
                    "10ms overshoot threshold is diagnostic, not the corpus publication jitter budget or L.",
                    "Successful-call maxima exclude failed calls explicitly; all failed rows remain in modes.",
                    "QoS is best effort and changes no requested base policy; no realtime guarantee."]});
            publish(file, &output, &report)?;
            if !completed {
                return Err("wait diagnostic failed; see retained private report".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_requires_explicit_freeze_and_fresh_output_paths() -> Result<(), String> {
        assert_eq!(
            parse(vec!["--print-parameters".into()])?,
            Options::PrintParameters
        );
        assert_eq!(
            parse(vec!["frozen.json".into(), "new.json".into()])?,
            Options::Run {
                parameters: "frozen.json".into(),
                output: "new.json".into()
            }
        );
        for args in [
            vec![],
            vec!["--unknown"],
            vec!["--print-parameters", "extra"],
            vec!["same", "same"],
            vec!["", "out"],
            vec!["--unknown", "out"],
            vec!["one", "two", "three"],
        ] {
            assert!(parse(args.into_iter().map(Into::into).collect()).is_err());
        }
        Ok(())
    }
    struct Fake {
        now: std::cell::Cell<Duration>,
        waits: usize,
        finishes: usize,
        initialized: Vec<Mode>,
        overshoot: Duration,
        fail_call: Option<usize>,
        fail_finish: bool,
        fail_enter: bool,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                now: std::cell::Cell::new(Duration::ZERO),
                waits: 0,
                finishes: 0,
                initialized: Vec::new(),
                overshoot: Duration::ZERO,
                fail_call: None,
                fail_finish: false,
                fail_enter: false,
            }
        }
    }
    impl Backend for Fake {
        fn initialize(&mut self, mode: Mode) -> Result<(), String> {
            self.initialized.push(mode);
            Ok(())
        }
        fn enter_qos(&mut self) -> Result<(), String> {
            if self.fail_enter {
                Err("entry unavailable".into())
            } else {
                Ok(())
            }
        }
        fn now(&self) -> Duration {
            self.now.get()
        }
        fn wait(&mut self, _: Mode, request: Duration) -> Result<(), String> {
            self.waits += 1;
            self.now.set(self.now.get() + request + self.overshoot);
            if self.fail_call == Some(self.waits) {
                Err("OS wait code 14".into())
            } else {
                Ok(())
            }
        }
        fn finish_qos(&mut self) -> Result<(), String> {
            self.finishes += 1;
            if self.fail_finish {
                Err("finish code 22".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn planned_modes_collect_real_intervals_and_continue_after_diagnostic_overshoot() {
        let mut backend = Fake::new();
        backend.overshoot = Duration::from_millis(11);
        let runs = run_modes(&mut backend, Duration::from_millis(25), 10);
        assert_eq!(backend.initialized, [Mode::Relative, Mode::Mach]);
        assert_eq!(backend.finishes, 2);
        for run in runs {
            assert_eq!(run.status, Status::Completed);
            assert_eq!(run.calls.len(), 3);
            assert_eq!(run.observed_duration_nanos, Some(36_000_000));
            assert_eq!(run.summary.above_threshold_calls, 3);
            assert_eq!(run.summary.successful_max_overshoot_nanos, Some(11_000_000));
            assert_eq!(run.calls[2].before_nanos, 24_000_000);
            assert_eq!(run.calls[2].after_nanos, Some(36_000_000));
        }
    }

    #[test]
    fn wait_error_retains_the_failed_call_finishes_qos_and_skips_following_mode() {
        let mut backend = Fake::new();
        backend.fail_call = Some(2);
        let runs = run_modes(&mut backend, Duration::from_millis(5), 10);
        assert_eq!(runs[0].status, Status::Failed);
        assert_eq!(runs[0].calls.len(), 2);
        assert_eq!(runs[0].calls[1].error.as_deref(), Some("OS wait code 14"));
        assert_eq!(runs[0].summary.successful_calls, 1);
        assert_eq!(runs[0].summary.failed_calls, 1);
        assert!(runs[0].qos_finished);
        assert_eq!(runs[1].status, Status::Unavailable);
        assert!(runs[1].calls.is_empty());
        assert_eq!(backend.initialized, [Mode::Relative]);
        assert_eq!(backend.finishes, 1);
    }
    #[test]
    fn capacity_and_qos_errors_preserve_evidence_and_do_not_start_the_next_mode() {
        let mut backend = Fake::new();
        let runs = run_modes(&mut backend, Duration::from_millis(5), 2);
        assert_eq!(runs[0].status, Status::Failed);
        assert_eq!(runs[0].calls.len(), 2);
        assert!(
            runs[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("capacity"))
        );
        assert_eq!(runs[1].status, Status::Unavailable);
        let mut backend = Fake::new();
        backend.fail_finish = true;
        let runs = run_modes(&mut backend, Duration::from_millis(2), 3);
        assert_eq!(runs[0].calls.len(), 2);
        assert_eq!(runs[0].qos_error.as_deref(), Some("finish code 22"));
        assert!(!runs[0].qos_finished);
        assert_eq!(runs[1].status, Status::Unavailable);
        let mut backend = Fake::new();
        backend.fail_enter = true;
        let runs = run_modes(&mut backend, Duration::from_millis(2), 3);
        assert_eq!(runs[0].status, Status::Failed);
        assert_eq!(runs[0].qos_error.as_deref(), Some("entry unavailable"));
        assert_eq!(backend.finishes, 0);
        assert!(runs[0].calls.is_empty());
        assert_eq!(runs[1].status, Status::Unavailable);
    }
    #[test]
    fn frozen_configuration_rejects_policy_or_source_drift()
    -> Result<(), Box<dyn std::error::Error>> {
        let value = parameters();
        assert_eq!(validate_parameters(&serde_json::to_vec(&value)?)?, value);
        let mut changed = value.clone();
        changed["sourceHashes"]["deadline"] = json!("changed");
        assert!(validate_parameters(&serde_json::to_vec(&changed)?).is_err());
        changed = value;
        changed["requestNanos"] = json!(2_000_000);
        assert!(validate_parameters(&serde_json::to_vec(&changed)?).is_err());
        assert!(validate_parameters(b"{").is_err());
        Ok(())
    }
    #[test]
    fn private_atomic_publication_never_replaces_an_existing_output()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("new.json");
        let file = prepare_output(&path)?;
        assert_eq!(
            file.as_file().metadata()?.permissions().mode() & 0o777,
            0o600
        );
        assert!(!path.exists());
        // A racing creator must win unchanged; even a completed trial cannot overwrite it.
        std::fs::write(&path, b"existing")?;
        assert!(publish(file, &path, &json!({"new":true})).is_err());
        assert_eq!(std::fs::read(&path)?, b"existing");
        assert!(prepare_output(&path).is_err());
        let fresh = directory.path().join("fresh.json");
        publish(prepare_output(&fresh)?, &fresh, &json!({"complete":true}))?;
        assert_eq!(
            std::fs::metadata(&fresh)?.permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(fresh)?)?,
            json!({"complete":true})
        );
        Ok(())
    }
}
