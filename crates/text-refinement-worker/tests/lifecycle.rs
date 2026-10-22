#![cfg(unix)]
use dictation_core::refinement::StagedEdits;
use std::{
    num::NonZeroU64,
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use text_refinement_protocol::{
    FailureKind, JobKey, Language, Request, Response, read_response, write_request,
};
use text_refinement_worker::{Generator, Identity, serve};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Immediate;
impl Generator for Immediate {
    fn generate(
        &mut self,
        _: &str,
        _: Language,
        _: &AtomicBool,
    ) -> Result<StagedEdits, FailureKind> {
        Ok(StagedEdits::default())
    }
    fn finish(self) -> Result<(), FailureKind> {
        Ok(())
    }
}
fn key(request: u64, generation: u64) -> Result<JobKey, Box<dyn std::error::Error>> {
    Ok(JobKey {
        session: NonZeroU64::MIN,
        generation: NonZeroU64::new(generation).ok_or("zero generation")?,
        request: NonZeroU64::new(request).ok_or("zero request")?,
    })
}
fn pair() -> Result<(UnixStream, UnixStream), Box<dyn std::error::Error>> {
    let (client, server) = UnixStream::pair()?;
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    client.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok((client, server))
}
fn identity() -> Identity {
    Identity {
        generation: NonZeroU64::MIN,
        model_manifest_sha256: "0".repeat(64),
        prompt_version: 1,
        policy_version: 1,
    }
}
fn run<G: Generator + Send + 'static>(
    server: UnixStream,
    generator: G,
) -> Result<
    thread::JoinHandle<Result<(), text_refinement_worker::WorkerError>>,
    Box<dyn std::error::Error>,
> {
    let input = server.try_clone()?;
    Ok(thread::spawn(move || {
        serve(
            input,
            server,
            identity(),
            Duration::from_secs(10),
            |_| Ok(generator),
            Arc::new(|_| {}),
        )
    }))
}

#[test]
fn ready_requests_and_shutdown_preserve_identity_and_finish_cleanly() -> TestResult {
    let (mut client, server) = pair()?;
    let handle = run(server, Immediate)?;
    assert!(
        matches!(read_response(&mut client)?,Some(Response::Ready {generation,..}) if generation==NonZeroU64::MIN)
    );
    let job = key(7, 1)?;
    write_request(
        &mut client,
        &Request::Run {
            job,
            language: Language::German,
            text: "Hallo.".into(),
        },
    )?;
    assert!(
        matches!(read_response(&mut client)?,Some(Response::Completed {job:actual,proposal}) if actual==job && proposal == StagedEdits::default())
    );
    write_request(&mut client, &Request::Shutdown {})?;
    handle.join().map_err(|_| "worker panicked")??;
    Ok(())
}

#[test]
fn stale_generation_and_replayed_request_cannot_run() -> TestResult {
    let (mut client, server) = pair()?;
    let handle = run(server, Immediate)?;
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Ready { .. })
    ));
    for (job, valid) in [
        (key(1, 2)?, false),
        (key(2, 1)?, true),
        (key(2, 1)?, false),
        (key(3, 1)?, true),
    ] {
        write_request(
            &mut client,
            &Request::Run {
                job,
                language: Language::German,
                text: "Hallo.".into(),
            },
        )?;
        let response = read_response(&mut client)?;
        assert!(if valid {
            matches!(response,Some(Response::Completed {job:actual,..}) if actual==job)
        } else {
            matches!(response,Some(Response::Failed {job:Some(actual),code:FailureKind::InvalidRequest}) if actual==job)
        });
    }
    write_request(&mut client, &Request::Shutdown {})?;
    handle.join().map_err(|_| "worker panicked")??;
    Ok(())
}

struct Blocking {
    started: mpsc::Sender<()>,
    finished: Arc<AtomicBool>,
}
impl Generator for Blocking {
    fn generate(
        &mut self,
        _: &str,
        _: Language,
        cancel: &AtomicBool,
    ) -> Result<StagedEdits, FailureKind> {
        self.started.send(()).map_err(|_| FailureKind::Generation)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        while !cancel.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                return Err(FailureKind::Generation);
            }
            thread::park_timeout(Duration::from_millis(1));
        }
        Ok(StagedEdits::default())
    }
    fn finish(self) -> Result<(), FailureKind> {
        self.finished.store(true, Ordering::Release);
        Ok(())
    }
}

#[test]
fn cancellation_is_job_scoped_and_a_busy_worker_never_queues_another_job() -> TestResult {
    let (mut client, server) = pair()?;
    let (started, ready) = mpsc::channel();
    let finished = Arc::new(AtomicBool::new(false));
    let handle = run(
        server,
        Blocking {
            started,
            finished: finished.clone(),
        },
    )?;
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Ready { .. })
    ));
    let job = key(1, 1)?;
    write_request(
        &mut client,
        &Request::Run {
            job,
            language: Language::English,
            text: "Hello.".into(),
        },
    )?;
    ready.recv_timeout(Duration::from_secs(3))?;
    write_request(&mut client, &Request::Cancel { job: key(2, 1)? })?;
    write_request(
        &mut client,
        &Request::Run {
            job: key(3, 1)?,
            language: Language::English,
            text: "second".into(),
        },
    )?;
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Failed {
            code: FailureKind::Busy,
            ..
        })
    ));
    write_request(&mut client, &Request::Cancel { job })?;
    assert!(
        matches!(read_response(&mut client)?,Some(Response::Cancelled {job:actual}) if actual==job)
    );
    write_request(&mut client, &Request::Shutdown {})?;
    handle.join().map_err(|_| "worker panicked")??;
    assert!(finished.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn requests_before_ready_are_rejected_without_consuming_the_request_id() -> TestResult {
    let (mut client, server) = pair()?;
    let input = server.try_clone()?;
    let (release, loading) = mpsc::channel();
    let handle = thread::spawn(move || {
        serve(
            input,
            server,
            identity(),
            Duration::from_secs(10),
            |_| {
                loading
                    .recv_timeout(Duration::from_secs(3))
                    .map_err(|_| FailureKind::ModelLoad)?;
                Ok(Immediate)
            },
            Arc::new(|_| {}),
        )
    });
    let job = key(1, 1)?;
    write_request(
        &mut client,
        &Request::Run {
            job,
            language: Language::English,
            text: "Hello.".into(),
        },
    )?;
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Failed {
            code: FailureKind::NotReady,
            ..
        })
    ));
    release.send(())?;
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Ready { .. })
    ));
    write_request(
        &mut client,
        &Request::Run {
            job,
            language: Language::English,
            text: "Hello.".into(),
        },
    )?;
    assert!(
        matches!(read_response(&mut client)?,Some(Response::Completed {job:actual,..}) if actual==job)
    );
    write_request(&mut client, &Request::Shutdown {})?;
    handle.join().map_err(|_| "worker panicked")??;
    Ok(())
}

#[test]
fn losing_the_parent_cancels_active_work_and_releases_the_generator() -> TestResult {
    use std::sync::atomic::AtomicUsize;
    let (mut client, server) = pair()?;
    let input = server.try_clone()?;
    let (started, ready) = mpsc::channel();
    let finished = Arc::new(AtomicBool::new(false));
    let closed = Arc::new(AtomicUsize::new(0));
    let observed = closed.clone();
    let generator = Blocking {
        started,
        finished: finished.clone(),
    };
    let (done, completion) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = serve(
            input,
            server,
            identity(),
            Duration::from_secs(10),
            |_| Ok(generator),
            Arc::new(move |_| {
                observed.fetch_add(1, Ordering::AcqRel);
            }),
        );
        let notified = done.send(result.is_ok());
        (result, notified)
    });
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Ready { .. })
    ));
    write_request(
        &mut client,
        &Request::Run {
            job: key(1, 1)?,
            language: Language::German,
            text: "Hallo.".into(),
        },
    )?;
    ready.recv_timeout(Duration::from_secs(3))?;
    drop(client);
    let _success = completion.recv_timeout(Duration::from_secs(3))?;
    let (_result, notification) = handle.join().map_err(|_| "worker panicked")?;
    notification?;
    assert!(finished.load(Ordering::Acquire));
    assert_eq!(closed.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn malformed_transport_ends_the_owner_without_dispatching_work() -> TestResult {
    use std::{io::Write, sync::atomic::AtomicUsize};
    let (mut client, server) = pair()?;
    let input = server.try_clone()?;
    let closed = Arc::new(AtomicUsize::new(0));
    let observed = closed.clone();
    let handle = thread::spawn(move || {
        serve(
            input,
            server,
            identity(),
            Duration::from_secs(10),
            |_| Ok(Immediate),
            Arc::new(move |code| {
                observed.store(code as usize + 1, Ordering::Release);
            }),
        )
    });
    assert!(matches!(
        read_response(&mut client)?,
        Some(Response::Ready { .. })
    ));
    client.write_all(&u32::MAX.to_be_bytes())?;
    assert!(handle.join().map_err(|_| "worker panicked")?.is_err());
    assert_eq!(closed.load(Ordering::Acquire), 2);
    Ok(())
}

#[test]
fn shutdown_during_loading_does_not_report_a_model_failure() -> TestResult {
    let (mut client, server) = pair()?;
    let input = server.try_clone()?;
    let handle = thread::spawn(move || {
        serve::<_, _, Immediate>(
            input,
            server,
            identity(),
            Duration::from_secs(10),
            |cancel| {
                let deadline = std::time::Instant::now() + Duration::from_secs(3);
                while !cancel.load(Ordering::Acquire) {
                    if std::time::Instant::now() > deadline {
                        return Err(FailureKind::ModelLoad);
                    }
                    thread::park_timeout(Duration::from_millis(1));
                }
                Err(FailureKind::Incomplete)
            },
            Arc::new(|_| {}),
        )
    });
    write_request(&mut client, &Request::Shutdown {})?;
    assert!(handle.join().map_err(|_| "worker panicked")?.is_ok());
    assert!(read_response(&mut client)?.is_none());
    Ok(())
}

#[test]
fn blocked_output_does_not_block_parent_eof() -> TestResult {
    struct BlockedWriter {
        started: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl std::io::Write for BlockedWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.started.send(()).map_err(std::io::Error::other)?;
            self.release.recv().map_err(std::io::Error::other)?;
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "released",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (client, server) = pair()?;
    let (started, writing) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (closed, observed) = mpsc::channel();
    let handle = thread::spawn(move || {
        serve(
            server,
            BlockedWriter {
                started,
                release: blocked,
            },
            identity(),
            Duration::from_secs(10),
            |_| Ok(Immediate),
            Arc::new(move |code| {
                if let Err(error) = closed.send(code) {
                    eprintln!("disconnect receiver already ended: {error}");
                }
            }),
        )
    });
    writing.recv_timeout(Duration::from_secs(3))?;
    drop(client);
    let result = observed.recv_timeout(Duration::from_secs(3));
    release.send(())?;
    assert_eq!(result?, 0);
    handle.join().map_err(|_| "worker panicked")??;
    Ok(())
}
