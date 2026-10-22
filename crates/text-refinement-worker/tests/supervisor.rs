#[path = "../../../src-tauri/src/refinement_service.rs"]
mod service;
use dictation_core::refinement::OriginalTranscript;
use service::{Cancellation, ExpectedIdentity, Limits, Worker, WorkerSpec};
use std::{num::NonZeroU64, path::PathBuf, time::Duration};
use text_refinement_protocol::{JobKey, Language};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn spec(root: &std::path::Path, mode: &str) -> Result<WorkerSpec, Box<dyn std::error::Error>> {
    let model_directory = root.join(mode);
    std::fs::create_dir_all(&model_directory)?;
    Ok(WorkerSpec {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_refinement-fixture")),
        model_directory,
        runtime_directory: root.to_path_buf(),
        identity: ExpectedIdentity {
            generation: NonZeroU64::MIN,
            model_manifest_sha256: "0".repeat(64),
            prompt_version: 6,
            policy_version: 2,
        },
    })
}
fn limits() -> Limits {
    Limits {
        startup: Duration::from_secs(2),
        request: Duration::from_secs(2),
        cancel_grace: Duration::from_millis(30),
    }
}
fn job(request: u64) -> Result<JobKey, Box<dyn std::error::Error>> {
    Ok(JobKey {
        session: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        request: NonZeroU64::new(request).ok_or("zero")?,
    })
}
#[test]
fn owned_worker_roundtrips_then_reaps_and_removes_its_snapshot() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    let scratch = worker.scratch_directory().to_path_buf();
    let original = OriginalTranscript::new("Hello.".into())?;
    let accepted = worker.run(job(1)?, &original, Language::English, &Cancellation::new())?;
    assert_eq!(accepted.as_str(), "Hello.");
    let termination = worker.close()?;
    assert!(termination.status.success());
    assert!(!termination.forced);
    assert!(!scratch.exists());
    Ok(())
}

#[test]
fn deadline_kills_a_hung_child_and_removes_its_snapshot() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut bounded = limits();
    bounded.request = Duration::from_millis(30);
    let mut worker = Worker::spawn(spec(root.path(), "hang")?, bounded, &Cancellation::new())?;
    let scratch = worker.scratch_directory().to_path_buf();
    assert!(worker.pid() > 0);
    let original = OriginalTranscript::new("Hello.".into())?;
    let error = worker
        .run(job(1)?, &original, Language::English, &Cancellation::new())
        .err()
        .ok_or("accepted hung child")?;
    assert_eq!(error.reason, service::Failure::RequestTimeout);
    assert!(!error.cleanup_failed);
    let termination = worker.close()?;
    assert!(termination.forced);
    assert!(!termination.status.success());
    assert!(!scratch.exists());
    Ok(())
}

#[test]
fn cancellation_revokes_a_running_job_then_reaps_the_child() -> TestResult {
    for mode in ["hang", "late"] {
        let root = tempfile::tempdir()?;
        let mut worker = Worker::spawn(spec(root.path(), mode)?, limits(), &Cancellation::new())?;
        let scratch = worker.scratch_directory().to_path_buf();
        let original = OriginalTranscript::new("Hello.".into())?;
        let cancellation = Cancellation::new();
        let outcome = std::thread::scope(|scope| -> Result<_, Box<dyn std::error::Error>> {
            let active = &cancellation;
            let work = scope.spawn(|| {
                worker.run(
                    job(1).map_err(|_| service::Error {
                        reason: service::Failure::InvalidRequest,
                        cleanup_failed: false,
                    })?,
                    &original,
                    Language::English,
                    active,
                )
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            while !scratch.join("run-started").exists() {
                if std::time::Instant::now() > deadline {
                    return Err("child did not start".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            cancellation.cancel();
            Ok(work.join().map_err(|_| "supervisor panicked")?)
        })?;
        let error = outcome.err().ok_or("cancelled work was accepted")?;
        assert_eq!(error.reason, service::Failure::Cancelled);
        assert!(!error.cleanup_failed);
        assert!(!scratch.exists());
    }
    Ok(())
}

#[test]
fn stderr_flood_is_bounded_and_cannot_become_a_successful_result() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut worker = Worker::spawn(
        spec(root.path(), "stderr-flood")?,
        limits(),
        &Cancellation::new(),
    )?;
    let scratch = worker.scratch_directory().to_path_buf();
    let original = OriginalTranscript::new("Hello.".into())?;
    let result = worker.run(job(1)?, &original, Language::English, &Cancellation::new());
    assert!(result.is_err());
    assert!(!scratch.exists());
    Ok(())
}

#[test]
fn startup_identity_must_match_every_bound_component() -> TestResult {
    for mode in [
        "wrong-generation",
        "wrong-manifest",
        "wrong-prompt",
        "wrong-policy",
    ] {
        let root = tempfile::tempdir()?;
        let result = Worker::spawn(spec(root.path(), mode)?, limits(), &Cancellation::new());
        let error = result.err().ok_or("accepted wrong worker identity")?;
        assert_eq!(error.reason, service::Failure::Identity);
        assert!(!error.cleanup_failed);
        assert_eq!(std::fs::read_dir(root.path())?.count(), 1);
    }
    Ok(())
}

#[test]
fn malformed_startup_and_startup_timeout_leave_no_owned_process_directory() -> TestResult {
    for (mode, expected) in [
        ("malformed", service::Failure::Transport),
        ("startup-hang", service::Failure::StartupTimeout),
    ] {
        let root = tempfile::tempdir()?;
        let mut bounded = limits();
        if mode == "startup-hang" {
            bounded.startup = Duration::from_millis(50);
        }
        let error = Worker::spawn(spec(root.path(), mode)?, bounded, &Cancellation::new())
            .err()
            .ok_or("accepted invalid startup")?;
        assert!(
            error.reason == expected
                || mode == "malformed" && error.reason == service::Failure::Exited,
            "{mode}: {:?}",
            error.reason
        );
        assert!(!error.cleanup_failed);
        assert_eq!(std::fs::read_dir(root.path())?.count(), 1);
    }
    Ok(())
}

#[test]
fn stale_answers_and_invalid_edits_cannot_release_text() -> TestResult {
    for (mode, expected) in [
        ("stale", service::Failure::Identity),
        ("invalid-edit", service::Failure::InvalidCandidate),
    ] {
        let root = tempfile::tempdir()?;
        let mut worker = Worker::spawn(spec(root.path(), mode)?, limits(), &Cancellation::new())?;
        let scratch = worker.scratch_directory().to_path_buf();
        let original = OriginalTranscript::new("Hello.".into())?;
        let error = worker
            .run(job(1)?, &original, Language::English, &Cancellation::new())
            .err()
            .ok_or("released invalid answer")?;
        assert_eq!(error.reason, expected);
        assert!(!error.cleanup_failed);
        assert!(!scratch.exists());
    }
    Ok(())
}

#[test]
fn unconsumed_large_request_cannot_block_deadline_cleanup() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut bounded = limits();
    bounded.request = Duration::from_millis(30);
    let mut worker = Worker::spawn(spec(root.path(), "no-read")?, bounded, &Cancellation::new())?;
    let original = OriginalTranscript::new("\\".repeat(30_000))?;
    let started = std::time::Instant::now();
    let error = worker
        .run(job(1)?, &original, Language::English, &Cancellation::new())
        .err()
        .ok_or("unconsumed request accepted")?;
    assert_eq!(error.reason, service::Failure::RequestTimeout);
    assert!(!error.cleanup_failed);
    assert!(started.elapsed() < Duration::from_secs(2));
    Ok(())
}

#[test]
fn request_replay_and_wrong_generation_do_not_poison_the_next_valid_job() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    let original = OriginalTranscript::new("Hello.".into())?;
    let token = Cancellation::new();
    worker.run(job(1)?, &original, Language::English, &token)?;
    assert_eq!(
        worker
            .run(job(1)?, &original, Language::English, &token)
            .err()
            .ok_or("replay accepted")?
            .reason,
        service::Failure::InvalidRequest
    );
    let mut wrong = job(2)?;
    wrong.generation = NonZeroU64::new(2).ok_or("generation")?;
    assert_eq!(
        worker
            .run(wrong, &original, Language::English, &token)
            .err()
            .ok_or("wrong generation accepted")?
            .reason,
        service::Failure::InvalidRequest
    );
    worker.run(job(2)?, &original, Language::English, &token)?;
    worker.close()?;
    Ok(())
}

#[test]
fn dropping_the_owner_also_waits_and_removes_its_snapshot() -> TestResult {
    let root = tempfile::tempdir()?;
    let worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    let scratch = worker.scratch_directory().to_path_buf();
    drop(worker);
    assert!(!scratch.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn failed_snapshot_cleanup_is_reported_on_initial_and_repeated_close() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let mut worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    let scratch = worker.scratch_directory().to_path_buf();
    std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o500))?;
    let first = worker.close();
    let repeated = worker.close();
    if scratch.exists() {
        std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o700))?;
    }
    for result in [first, repeated] {
        let error = result.err().ok_or("cleanup failure was accepted")?;
        assert_eq!(error.reason, service::Failure::Cleanup);
        assert!(error.cleanup_failed);
    }
    Ok(())
}

#[test]
fn unsolicited_output_flood_never_completes_a_job_and_is_reaped() -> TestResult {
    let root = tempfile::tempdir()?;
    let started = Worker::spawn(
        spec(root.path(), "stdout-flood")?,
        limits(),
        &Cancellation::new(),
    );
    match started {
        Ok(mut worker) => {
            let original = OriginalTranscript::new("Hello.".into())?;
            assert!(
                worker
                    .run(job(1)?, &original, Language::English, &Cancellation::new())
                    .is_err()
            );
        }
        Err(error) => assert!(!error.cleanup_failed),
    }
    assert_eq!(std::fs::read_dir(root.path())?.count(), 1);
    Ok(())
}
