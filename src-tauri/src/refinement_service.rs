//! Owns one local worker process. Call on a service thread, never an audio callback.
#[path = "refinement_service/owner.rs"]
pub mod owner;
#[path = "refinement_service/scratch.rs"]
pub mod scratch;
use dictation_core::refinement::{AcceptedTranscript, OriginalTranscript, validate};
use std::{
    io::{Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use text_refinement_protocol::{
    FailureKind, JobKey, Language, Request, Response, read_response, write_request,
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
#[derive(Clone)]
pub struct ExpectedIdentity {
    pub generation: NonZeroU64,
    pub model_manifest_sha256: String,
    pub prompt_version: u16,
    pub policy_version: u16,
}
/// Paths come from trusted application code, not IPC or user preferences.
#[derive(Clone)]
pub struct WorkerSpec {
    pub executable: PathBuf,
    pub model_directory: PathBuf,
    pub runtime_directory: PathBuf,
    pub identity: ExpectedIdentity,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub startup: Duration,
    pub request: Duration,
    pub cancel_grace: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            startup: Duration::from_secs(60),
            request: Duration::from_secs(30),
            cancel_grace: Duration::from_millis(250),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    InvalidPath,
    Spawn,
    Transport,
    Identity,
    InvalidRequest,
    Cancelled,
    StartupTimeout,
    RequestTimeout,
    Exited,
    InvalidCandidate,
    Cleanup,
    Rejected(FailureKind),
    Closed,
}
#[derive(Debug, thiserror::Error)]
#[error("Local text worker failed ({reason:?}); cleanup failed: {cleanup_failed}")]
pub struct Error {
    pub reason: Failure,
    pub cleanup_failed: bool,
}
impl From<Failure> for Error {
    fn from(reason: Failure) -> Self {
        Self {
            reason,
            cleanup_failed: reason == Failure::Cleanup,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Termination {
    pub status: ExitStatus,
    pub forced: bool,
    pub cleanup_complete: bool,
}

pub struct Worker {
    child: Option<Child>,
    pid: u32,
    scratch: Option<scratch::Scratch>,
    scratch_path: PathBuf,
    expected: ExpectedIdentity,
    limits: Limits,
    inbox: Receiver<Response>,
    outbox: Option<SyncSender<Request>>,
    fault: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    termination: Option<Termination>,
    last_request: u64,
}

fn real_path(path: &Path, directory: bool) -> bool {
    path.is_absolute()
        && std::fs::symlink_metadata(path).is_ok_and(|m| {
            !m.file_type().is_symlink() && if directory { m.is_dir() } else { m.is_file() }
        })
}
impl Worker {
    pub fn spawn(spec: WorkerSpec, limits: Limits, cancel: &Cancellation) -> Result<Self, Error> {
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled.into());
        }
        if !real_path(&spec.executable, false)
            || !real_path(&spec.model_directory, true)
            || !real_path(&spec.runtime_directory, true)
        {
            return Err(Failure::InvalidPath.into());
        }
        let scratch =
            scratch::Scratch::create(&spec.runtime_directory).map_err(|_| Failure::Spawn)?;
        let mut child = Command::new(&spec.executable)
            .arg(&spec.model_directory)
            .arg(spec.identity.generation.to_string())
            .arg(scratch.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| Failure::Spawn)?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (outbox, requests) = mpsc::sync_channel(1);
        let (responses, inbox) = mpsc::sync_channel(1);
        let mut worker = Self {
            pid: child.id(),
            child: Some(child),
            scratch_path: scratch.path().to_path_buf(),
            scratch: Some(scratch),
            expected: spec.identity,
            limits,
            inbox,
            outbox: Some(outbox),
            fault: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
            termination: None,
            last_request: 0,
        };
        let pipes = stdin.zip(stdout).zip(stderr).ok_or(Failure::Transport);
        let ((stdin, stdout), stderr) = match pipes {
            Ok(pipes) => pipes,
            Err(reason) => return Err(worker.abort(reason)),
        };
        if let Err(reason) = worker.start_io(stdin, stdout, stderr, requests, responses) {
            return Err(worker.abort(reason));
        }
        let ready = worker.receive(
            Instant::now()
                .checked_add(limits.startup)
                .ok_or(Failure::InvalidRequest)?,
            cancel,
            Failure::StartupTimeout,
        );
        match ready {
            Ok(Response::Ready {
                generation,
                model_manifest_sha256,
                prompt_version,
                policy_version,
            }) if generation == worker.expected.generation
                && model_manifest_sha256 == worker.expected.model_manifest_sha256
                && prompt_version == worker.expected.prompt_version
                && policy_version == worker.expected.policy_version =>
            {
                Ok(worker)
            }
            Ok(Response::Failed { job: None, code }) => Err(worker.abort(Failure::Rejected(code))),
            Ok(_) => Err(worker.abort(Failure::Identity)),
            Err(reason) => Err(worker.abort(reason)),
        }
    }

    fn start_io(
        &mut self,
        mut stdin: impl Write + Send + 'static,
        mut stdout: impl Read + Send + 'static,
        mut stderr: impl Read + Send + 'static,
        requests: Receiver<Request>,
        responses: SyncSender<Response>,
    ) -> Result<(), Failure> {
        let fault = self.fault.clone();
        self.threads.push(
            thread::Builder::new()
                .name("text-worker-input".into())
                .spawn(move || {
                    while let Ok(request) = requests.recv() {
                        if write_request(&mut stdin, &request).is_err() {
                            fault.store(true, Ordering::Release);
                            break;
                        }
                    }
                })
                .map_err(|_| Failure::Transport)?,
        );
        let fault = self.fault.clone();
        self.threads.push(
            thread::Builder::new()
                .name("text-worker-output".into())
                .spawn(move || {
                    loop {
                        match read_response(&mut stdout) {
                            Ok(Some(response)) => {
                                if responses.try_send(response).is_err() {
                                    fault.store(true, Ordering::Release);
                                    break;
                                }
                            }
                            _ => {
                                fault.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
                })
                .map_err(|_| Failure::Transport)?,
        );
        let fault = self.fault.clone();
        self.threads.push(
            thread::Builder::new()
                .name("text-worker-errors".into())
                .spawn(move || {
                    // Drain into fixed storage; model text or arbitrary stderr is never retained.
                    let mut buffer = [0u8; 4096];
                    let mut total = 0usize;
                    loop {
                        match stderr.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(count) => {
                                total += count;
                                if total > 16 * 1024 {
                                    fault.store(true, Ordering::Release);
                                    break;
                                }
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                            Err(_) => {
                                fault.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
                })
                .map_err(|_| Failure::Transport)?,
        );
        Ok(())
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn scratch_directory(&self) -> &Path {
        &self.scratch_path
    }

    fn send(&self, request: Request) -> Result<(), Failure> {
        self.outbox
            .as_ref()
            .ok_or(Failure::Closed)?
            .try_send(request)
            .map_err(|_| Failure::Transport)
    }
    fn receive(
        &mut self,
        deadline: Instant,
        cancel: &Cancellation,
        timeout: Failure,
    ) -> Result<Response, Failure> {
        loop {
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(timeout);
            }
            if self.fault.load(Ordering::Acquire) {
                return Err(Failure::Transport);
            }
            if self
                .child
                .as_mut()
                .ok_or(Failure::Closed)?
                .try_wait()
                .map_err(|_| Failure::Transport)?
                .is_some()
            {
                return Err(Failure::Exited);
            }
            match self.inbox.recv_timeout(
                Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
            ) {
                Ok(response) => return Ok(response),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Failure::Transport),
            }
        }
    }

    pub fn run(
        &mut self,
        job: JobKey,
        original: &OriginalTranscript,
        language: Language,
        cancel: &Cancellation,
    ) -> Result<AcceptedTranscript, Error> {
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled.into());
        }
        if self.child.is_none() {
            return Err(Failure::Closed.into());
        }
        if job.generation != self.expected.generation || job.request.get() <= self.last_request {
            return Err(Failure::InvalidRequest.into());
        }
        self.last_request = job.request.get();
        let result = self
            .send(Request::Run {
                job,
                text: original.as_str().into(),
                language,
            })
            .and_then(|()| {
                self.receive(
                    Instant::now()
                        .checked_add(self.limits.request)
                        .ok_or(Failure::InvalidRequest)?,
                    cancel,
                    Failure::RequestTimeout,
                )
            });
        let response = match result {
            Ok(response) => response,
            Err(reason) => return Err(self.abort(reason)),
        };
        if cancel.is_cancelled() {
            return Err(self.abort(Failure::Cancelled));
        }
        match response {
            Response::Completed {
                job: received,
                edits,
            } if job == received => validate(original, &edits, language.into())
                .map_err(|_| self.abort(Failure::InvalidCandidate)),
            Response::Failed {
                job: Some(received),
                code,
            } if job == received => Err(self.abort(Failure::Rejected(code))),
            Response::Cancelled { job: received } if job == received => {
                Err(self.abort(Failure::Cancelled))
            }
            _ => Err(self.abort(Failure::Identity)),
        }
    }

    fn abort(&mut self, reason: Failure) -> Error {
        Error {
            reason,
            cleanup_failed: self.close().is_err(),
        }
    }
    pub fn close(&mut self) -> Result<Termination, Error> {
        if let Some(termination) = self.termination {
            return if termination.cleanup_complete {
                Ok(termination)
            } else {
                Err(Failure::Cleanup.into())
            };
        }
        let requested = self.send(Request::Shutdown {}).is_ok();
        self.terminate(if requested {
            self.limits.cancel_grace
        } else {
            Duration::ZERO
        })
        .map_err(Error::from)
    }
    fn terminate(&mut self, grace: Duration) -> Result<Termination, Failure> {
        drop(self.outbox.take()); // Closing the writer channel also closes stdin after pending writes.
        let deadline = Instant::now().checked_add(grace).ok_or(Failure::Cleanup)?;
        let child = self.child.as_mut().ok_or(Failure::Closed)?;
        let mut status = child.try_wait().map_err(|_| Failure::Cleanup)?;
        while status.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
            status = child.try_wait().map_err(|_| Failure::Cleanup)?;
        }
        let forced = status.is_none();
        let status = if let Some(status) = status {
            status
        } else {
            child.kill().map_err(|_| Failure::Cleanup)?;
            child.wait().map_err(|_| Failure::Cleanup)?
        };
        drop(self.child.take());
        let mut complete = true;
        for handle in self.threads.drain(..) {
            if handle.join().is_err() {
                complete = false;
            }
        }
        if let Some(scratch) = self.scratch.take()
            && scratch.close().is_err()
        {
            complete = false;
        }
        let termination = Termination {
            status,
            forced,
            cleanup_complete: complete,
        };
        self.termination = Some(termination);
        if complete {
            Ok(termination)
        } else {
            Err(Failure::Cleanup)
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if self.child.is_some() && self.terminate(Duration::ZERO).is_err() {
            // Never remove a snapshot while a child might still map it.
            if let Some(scratch) = self.scratch.take() {
                drop(scratch.keep());
            }
            eprintln!("Local text worker cleanup failed");
        }
    }
}
