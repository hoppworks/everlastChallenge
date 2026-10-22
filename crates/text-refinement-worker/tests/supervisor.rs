#[path = "../../../src-tauri/src/refinement_service.rs"]
mod service;
use dictation_core::refinement::OriginalTranscript;
use service::{Cancellation, ExpectedIdentity, Limits, Worker, WorkerSpec};
use std::{num::NonZeroU64, path::PathBuf, time::Duration};
use text_refinement_protocol::{JobKey, Language};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn rejected_start_and_closed_worker_cannot_receive_work() -> TestResult {
    let root = tempfile::tempdir()?;
    let cancelled = Cancellation::new();
    cancelled.cancel();
    assert!(matches!(
        Worker::spawn(spec(root.path(), "normal")?, limits(), &cancelled),
        Err(service::Error {
            reason: service::Failure::Cancelled,
            ..
        })
    ));
    let mut invalid = spec(root.path(), "normal")?;
    invalid.executable = PathBuf::from("relative-worker");
    assert!(matches!(
        Worker::spawn(invalid, limits(), &Cancellation::new()),
        Err(service::Error {
            reason: service::Failure::InvalidPath,
            ..
        })
    ));
    let mut worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    worker.close()?;
    let original = OriginalTranscript::new("Hello.".into())?;
    let job = JobKey {
        generation: NonZeroU64::new(1).ok_or("zero")?,
        session: NonZeroU64::new(1).ok_or("zero")?,
        request: NonZeroU64::new(1).ok_or("zero")?,
    };
    assert!(matches!(
        worker.run(job, &original, Language::English, &cancelled),
        Err(service::Error {
            reason: service::Failure::Cancelled,
            ..
        })
    ));
    assert!(matches!(
        worker.run(job, &original, Language::English, &Cancellation::new()),
        Err(service::Error {
            reason: service::Failure::Closed,
            ..
        })
    ));
    Ok(())
}

#[test]
fn runtime_preparation_secures_directories_and_rejects_files_and_symlinks() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir()?;
    let runtime = root.path().join("nested/runtime");
    service::scratch::prepare(&runtime)?;
    assert_eq!(
        std::fs::metadata(&runtime)?.permissions().mode() & 0o777,
        0o700
    );
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755))?;
    service::scratch::prepare(&runtime)?;
    assert_eq!(
        std::fs::metadata(&runtime)?.permissions().mode() & 0o777,
        0o700
    );

    let file = root.path().join("file");
    std::fs::write(&file, b"keep")?;
    assert!(service::scratch::prepare(&file).is_err());
    assert_eq!(std::fs::read(&file)?, b"keep");
    let linked = root.path().join("linked");
    symlink(&runtime, &linked)?;
    assert!(service::scratch::prepare(&linked).is_err());
    assert!(std::fs::symlink_metadata(&linked)?.file_type().is_symlink());
    Ok(())
}

fn until(mut ready: impl FnMut() -> bool) -> TestResult {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !ready() {
        if std::time::Instant::now() >= deadline {
            return Err("condition did not settle".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

fn owner_job(
    session: u64,
    cancel: Cancellation,
) -> Result<service::owner::Job, Box<dyn std::error::Error>> {
    Ok(service::owner::Job {
        session: NonZeroU64::new(session).ok_or("zero")?,
        original: OriginalTranscript::new("Hello.".into())?,
        language: Language::English,
        cancel,
    })
}

fn owner_result(
    owner: &service::owner::Owner,
) -> Result<service::owner::Completed, Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(done) = owner.take_result()? {
            assert_eq!(done.original.as_str(), "Hello.");
            return Ok(done);
        }
        if std::time::Instant::now() >= deadline {
            return Err("owner did not settle".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn async_owner_unload_cancels_startup_and_generation_without_blocking_the_caller() -> TestResult {
    use service::owner::Owner;
    for mode in ["hang", "startup-hang"] {
        let root = tempfile::tempdir()?;
        let mut bounded = limits();
        bounded.request = Duration::from_secs(10);
        bounded.startup = Duration::from_secs(10);
        let owner = Owner::spawn(spec(root.path(), mode)?, bounded, Duration::from_secs(120))?;
        let cancel = Cancellation::new();
        owner.submit(owner_job(1, cancel.clone())?)?;
        until(|| {
            std::fs::read_dir(root.path()).is_ok_and(|entries| {
                entries.flatten().any(|e| {
                    e.path()
                        .join(if mode == "hang" {
                            "run-started"
                        } else {
                            "model-snapshot"
                        })
                        .exists()
                })
            })
        })?;
        owner.unload()?;
        assert!(cancel.is_cancelled());
        let error = owner_result(&owner)?
            .result
            .err()
            .ok_or("cancelled job accepted")?;
        assert_eq!(error.reason, service::Failure::Cancelled);
        assert!(!error.cleanup_failed);
        assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
        owner.shutdown()?;
    }
    Ok(())
}

#[test]
fn async_owner_idle_unloads_then_restarts_with_a_new_generation() -> TestResult {
    use service::owner::Owner;
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "normal")?,
        limits(),
        Duration::from_millis(30),
    )?;
    owner.submit(owner_job(1, Cancellation::new())?)?;
    assert_eq!(owner_result(&owner)?.result?.as_str(), "Hello.");
    until(|| std::fs::read_dir(root.path()).is_ok_and(|entries| entries.count() == 2))?;
    owner.submit(owner_job(2, Cancellation::new())?)?;
    assert_eq!(owner_result(&owner)?.result?.as_str(), "Hello.");
    let trace = std::fs::read_to_string(root.path().join("normal/requests.log"))?;
    let rows = trace
        .lines()
        .map(str::split_whitespace)
        .map(Iterator::collect::<Vec<_>>)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1], "1");
    assert_eq!(rows[1][1], "2");
    assert_ne!(rows[0][0], rows[1][0]);
    owner.shutdown()?;
    Ok(())
}

#[test]
fn async_owner_cannot_release_a_completion_revoked_before_dequeue() -> TestResult {
    use service::owner::Owner;
    for unload in [false, true] {
        let root = tempfile::tempdir()?;
        let owner = Owner::spawn(
            spec(root.path(), "normal")?,
            limits(),
            Duration::from_millis(30),
        )?;
        let cancel = Cancellation::new();
        owner.submit(owner_job(1, cancel.clone())?)?;
        // The idle close happens only after publishing this job's completion.
        until(|| {
            root.path().join("normal/requests.log").exists()
                && std::fs::read_dir(root.path()).is_ok_and(|entries| entries.count() == 2)
        })?;
        if unload {
            owner.unload()?;
        } else {
            cancel.cancel();
        }
        assert!(cancel.is_cancelled());
        let error = owner_result(&owner)?
            .result
            .err()
            .ok_or("revoked completion released")?;
        assert_eq!(error.reason, service::Failure::Cancelled);
        owner.shutdown()?;
    }
    Ok(())
}

#[test]
fn async_owner_valid_stored_result_survives_a_rejected_followup_job() -> TestResult {
    use service::owner::{Owner, OwnerError};
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "normal")?,
        limits(),
        Duration::from_millis(30),
    )?;
    owner.submit(owner_job(1, Cancellation::new())?)?;
    // Idle cleanup proves completion was stored, rather than merely in flight.
    until(|| {
        root.path().join("normal/requests.log").exists()
            && std::fs::read_dir(root.path()).is_ok_and(|entries| entries.count() == 2)
    })?;
    assert!(matches!(
        owner.submit(owner_job(2, Cancellation::new())?),
        Err(OwnerError::Busy)
    ));
    let first = owner_result(&owner)?;
    assert_eq!(first.session.get(), 1);
    assert_eq!(first.result?.as_str(), "Hello.");
    owner.submit(owner_job(2, Cancellation::new())?)?;
    assert_eq!(owner_result(&owner)?.session.get(), 2);
    owner.shutdown()?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn async_owner_shutdown_preserves_a_cleanup_failure_during_cancel() -> TestResult {
    use service::owner::{Owner, OwnerError};
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "hang")?,
        limits(),
        Duration::from_secs(120),
    )?;
    owner.submit(owner_job(1, Cancellation::new())?)?;
    let mut scratch = None;
    until(|| {
        scratch = std::fs::read_dir(root.path()).ok().and_then(|entries| {
            entries
                .flatten()
                .find(|e| e.path().join("run-started").exists())
                .map(|e| e.path())
        });
        scratch.is_some()
    })?;
    let scratch = scratch.ok_or("no running child")?;
    std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o500))?;
    let result = owner.shutdown();
    if scratch.exists() {
        std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o700))?;
    }
    let error = result.err().ok_or("shutdown hid failed cleanup")?;
    assert!(matches!(
        error,
        OwnerError::Worker(service::Error {
            cleanup_failed: true,
            ..
        })
    ));
    Ok(())
}

#[test]
fn async_owner_reaps_a_clean_idle_exit_and_starts_the_next_explicit_job() -> TestResult {
    use service::owner::Owner;
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "self-idle")?,
        limits(),
        Duration::from_secs(120),
    )?;
    owner.submit(owner_job(1, Cancellation::new())?)?;
    owner_result(&owner)?.result?;
    std::fs::write(root.path().join("self-idle/allow-idle-exit"), b"exit")?;
    until(|| std::fs::read_dir(root.path()).is_ok_and(|entries| entries.count() == 2))?;
    std::fs::remove_file(root.path().join("self-idle/allow-idle-exit"))?;
    owner.submit(owner_job(2, Cancellation::new())?)?;
    owner_result(&owner)?.result?;
    std::fs::write(root.path().join("self-idle/allow-idle-exit"), b"exit")?;
    owner.shutdown()?;
    let trace = std::fs::read_to_string(root.path().join("self-idle/requests.log"))?;
    let generations = trace
        .lines()
        .map(|line| line.split_whitespace().nth(1))
        .collect::<Vec<_>>();
    assert_eq!(generations, vec![Some("1"), Some("2")]);
    Ok(())
}

#[test]
fn async_owner_idle_crash_is_reported_without_a_restart_loop() -> TestResult {
    use service::owner::{Owner, OwnerError};
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "idle-crash")?,
        limits(),
        Duration::from_secs(120),
    )?;
    owner.submit(owner_job(1, Cancellation::new())?)?;
    owner_result(&owner)?.result?;
    std::fs::write(root.path().join("idle-crash/allow-idle-exit"), b"exit")?;
    until(|| matches!(owner.take_result(), Err(OwnerError::Closed)))?;
    assert!(matches!(
        owner.submit(owner_job(2, Cancellation::new())?),
        Err(OwnerError::Closed)
    ));
    let error = owner.shutdown().err().ok_or("idle crash hidden")?;
    assert!(matches!(
        error,
        OwnerError::Worker(service::Error {
            reason: service::Failure::Exited,
            cleanup_failed: false
        })
    ));
    assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
    assert_eq!(
        std::fs::read_to_string(root.path().join("idle-crash/requests.log"))?
            .lines()
            .count(),
        1
    );
    Ok(())
}

#[test]
fn async_owner_failed_jobs_only_restart_for_a_new_explicit_session() -> TestResult {
    use service::owner::{Owner, OwnerError};
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "invalid-edit")?,
        limits(),
        Duration::from_secs(120),
    )?;
    let cancelled = Cancellation::new();
    cancelled.cancel();
    assert!(matches!(
        owner.submit(owner_job(1, cancelled)?),
        Err(OwnerError::Worker(_))
    ));
    assert!(!root.path().join("invalid-edit/requests.log").exists());
    for session in [1, 2] {
        owner.submit(owner_job(session, Cancellation::new())?)?;
        let error = owner_result(&owner)?
            .result
            .err()
            .ok_or("invalid candidate released")?;
        assert_eq!(error.reason, service::Failure::InvalidCandidate);
        assert!(!error.cleanup_failed);
        assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
        assert!(matches!(
            owner.submit(owner_job(session, Cancellation::new())?),
            Err(OwnerError::InvalidJob)
        ));
    }
    let trace = std::fs::read_to_string(root.path().join("invalid-edit/requests.log"))?;
    let generations = trace
        .lines()
        .map(|line| line.split_whitespace().nth(1))
        .collect::<Vec<_>>();
    assert_eq!(generations, vec![Some("1"), Some("2")]);
    owner.shutdown()?;
    Ok(())
}

#[test]
fn async_owner_shutdown_and_drop_revoke_and_join_active_work() -> TestResult {
    use service::owner::Owner;
    for mode in ["hang", "startup-hang"] {
        for implicit in [false, true] {
            let root = tempfile::tempdir()?;
            let mut bounded = limits();
            bounded.startup = Duration::from_secs(60);
            bounded.request = Duration::from_secs(60);
            let owner = Owner::spawn(spec(root.path(), mode)?, bounded, Duration::from_secs(120))?;
            let cancel = Cancellation::new();
            owner.submit(owner_job(1, cancel.clone())?)?;
            until(|| {
                std::fs::read_dir(root.path()).is_ok_and(|entries| {
                    entries.flatten().any(|e| {
                        e.path()
                            .join(if mode == "hang" {
                                "run-started"
                            } else {
                                "model-snapshot"
                            })
                            .exists()
                    })
                })
            })?;
            if implicit {
                drop(owner);
            } else {
                owner.shutdown()?;
            }
            assert!(cancel.is_cancelled());
            assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
        }
    }
    Ok(())
}

#[test]
fn async_owner_is_lazy_reuses_the_process_and_preserves_unconsumed_results() -> TestResult {
    use service::owner::{Job, Owner, OwnerError};
    let root = tempfile::tempdir()?;
    let owner = Owner::spawn(
        spec(root.path(), "normal")?,
        limits(),
        Duration::from_secs(2),
    )?;
    assert!(!root.path().join("normal/requests.log").exists());
    let submit = |session| -> Result<(), Box<dyn std::error::Error>> {
        owner.submit(Job {
            session: NonZeroU64::new(session).ok_or("zero")?,
            original: OriginalTranscript::new("Hello.".into())?,
            language: Language::English,
            cancel: Cancellation::new(),
        })?;
        Ok(())
    };
    submit(1)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !root.path().join("normal/requests.log").exists() {
        if std::time::Instant::now() >= deadline {
            return Err("worker did not receive job".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        owner.submit(Job {
            session: NonZeroU64::new(2).ok_or("zero")?,
            original: OriginalTranscript::new("Hello.".into())?,
            language: Language::English,
            cancel: Cancellation::new(),
        }),
        Err(OwnerError::Busy)
    ));
    let wait = || -> Result<_, Box<dyn std::error::Error>> {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(done) = owner.take_result()? {
                return Ok(done);
            }
            if std::time::Instant::now() >= deadline {
                return Err("owner did not complete".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    let done = wait()?;
    assert_eq!(done.session.get(), 1);
    assert_eq!(done.result?.as_str(), "Hello.");
    submit(2)?;
    assert_eq!(wait()?.result?.as_str(), "Hello.");
    let trace = std::fs::read_to_string(root.path().join("normal/requests.log"))?;
    let rows = trace
        .lines()
        .map(str::split_whitespace)
        .map(Iterator::collect::<Vec<_>>)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], rows[1][0]);
    assert_eq!(rows[0][1], rows[1][1]);
    assert_eq!(rows[0][2], "1");
    assert_eq!(rows[1][2], "2");
    owner.shutdown()?;
    assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
    Ok(())
}

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
            prompt_version: text_refinement_protocol::TEXT_PROMPT_VERSION,
            policy_version: text_refinement_protocol::TEXT_POLICY_VERSION,
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&scratch)?.permissions().mode() & 0o077,
            0,
            "worker snapshot must be private before model loading"
        );
    }
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
        assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
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
        assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
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
    assert_eq!(std::fs::read_dir(root.path())?.count(), 2);
    Ok(())
}

#[test]
fn startup_recovers_only_released_versioned_scratch_directories() -> TestResult {
    use std::io::Write;
    let root = tempfile::tempdir()?;
    let stale = root.path().join("text-worker-v1-stale");
    let active = root.path().join("text-worker-v1-active");
    let foreign = root.path().join("text-worker-unmarked");
    for directory in [&stale, &active, &foreign] {
        std::fs::create_dir(directory)?;
    }
    for directory in [&stale, &active] {
        std::fs::write(directory.join(".owner.lock"), b"everlast-text-owner-v1\n")?;
        std::fs::write(directory.join("model-bytes"), b"retained snapshot")?;
    }
    let mut held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(active.join(".owner.lock"))?;
    held.lock_shared()?;
    held.flush()?;
    let mut worker = Worker::spawn(spec(root.path(), "normal")?, limits(), &Cancellation::new())?;
    assert!(!stale.exists(), "released scratch was not recovered");
    assert!(
        active.join("model-bytes").exists(),
        "live scratch was removed"
    );
    assert!(foreign.exists(), "unmarked directory was removed");
    worker.close()?;
    held.unlock()?;
    Ok(())
}

#[test]
#[ignore = "Subprocess fixture invoked by abrupt_host_exit_preserves_live_lease_then_recovers"]
fn scratch_crash_host_fixture() -> TestResult {
    let root = std::path::PathBuf::from(
        std::env::var_os("EVERLAST_CRASH_TEST_ROOT").ok_or("subprocess fixture root missing")?,
    );
    let worker = Worker::spawn(spec(&root, "normal")?, limits(), &Cancellation::new())?;
    std::fs::write(
        root.join("normal/host-ready"),
        worker.scratch_directory().as_os_str().as_encoded_bytes(),
    )?;
    loop {
        std::thread::park();
    }
}

#[test]
fn abrupt_host_exit_preserves_live_lease_then_recovers() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut host = std::process::Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", "scratch_crash_host_fixture"])
        .env("EVERLAST_CRASH_TEST_ROOT", root.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let ready = until(|| root.path().join("normal/host-ready").exists());
    // Always reap our fixture even when startup did not produce its marker.
    host.kill()?;
    host.wait()?;
    ready?;
    let scratch = PathBuf::from(std::fs::read_to_string(
        root.path().join("normal/host-ready"),
    )?);
    until(|| root.path().join("normal/child-exited").exists())?;
    until(|| service::scratch::recover(root.path()).is_ok() && !scratch.exists())?;
    assert!(root.path().join("normal").is_dir());
    Ok(())
}

#[cfg(unix)]
#[test]
fn recovery_preserves_foreign_markers_and_rejects_a_symlinked_root_lock() -> TestResult {
    let root = tempfile::tempdir()?;
    let unknown = root.path().join("text-worker-v1-unknown");
    std::fs::create_dir(&unknown)?;
    std::fs::write(unknown.join(".owner.lock"), b"foreign marker")?;
    service::scratch::recover(root.path())?;
    assert!(unknown.exists());
    std::fs::remove_file(root.path().join(".text-worker.lock"))?;
    let target = root.path().join("unrelated");
    std::fs::write(&target, b"untouched")?;
    std::os::unix::fs::symlink(&target, root.path().join(".text-worker.lock"))?;
    assert!(service::scratch::recover(root.path()).is_err());
    assert_eq!(std::fs::read(target)?, b"untouched");
    assert!(unknown.exists());
    Ok(())
}

#[test]
fn real_process_stages_require_host_validation_before_returning_any_text() -> TestResult {
    for mode in ["staged", "invalid-second"] {
        let root = tempfile::tempdir()?;
        let mut worker = Worker::spawn(spec(root.path(), mode)?, limits(), &Cancellation::new())?;
        let original = OriginalTranscript::new("Um, hello".into())?;
        let result = worker.run(job(1)?, &original, Language::English, &Cancellation::new());
        if mode == "staged" {
            let accepted = result?;
            assert_eq!(accepted.as_str(), "Hello.");
            assert!(accepted.original() == &original);
            assert_eq!(accepted.edit_count(), 2);
        } else {
            assert!(matches!(
                result,
                Err(service::Error {
                    reason: service::Failure::InvalidCandidate,
                    ..
                })
            ));
        }
        worker.close()?;
    }
    Ok(())
}
