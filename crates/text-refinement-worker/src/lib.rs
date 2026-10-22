#![forbid(unsafe_code)]
//! One inference owner with bounded input and session-scoped cancellation.

pub mod manifest;
pub mod punctuation;
pub mod selection;
pub mod single_pass;
mod word_grammar;

#[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
pub mod model;

use dictation_core::refinement::StagedEdits;
use std::{
    io::{Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use text_refinement_protocol::{
    FailureKind, JobKey, Language, Request, Response, read_request, write_response,
};

pub trait Generator {
    fn generate(
        &mut self,
        text: &str,
        language: Language,
        cancelled: &AtomicBool,
    ) -> Result<StagedEdits, FailureKind>;
    fn finish(self) -> Result<(), FailureKind>;
}

pub struct Identity {
    pub generation: NonZeroU64,
    pub model_manifest_sha256: String,
    pub prompt_version: u16,
    pub policy_version: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("Worker transport failed")]
    Transport(#[from] text_refinement_protocol::ProtocolError),
    #[error("Worker output backpressure exceeded its bounded queue")]
    Backpressure,
    #[error("Worker synchronization failed")]
    Synchronization,
    #[error("Worker input thread failed")]
    Thread,
    #[error("Worker model could not be loaded")]
    Load,
    #[error("Worker model cleanup failed")]
    Cleanup,
    #[error("Worker operation and model cleanup failed")]
    OperationAndCleanup,
}

struct Work {
    job: JobKey,
    language: Language,
    text: String,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    ready: bool,
    stop: bool,
    occupied: Option<(JobKey, Arc<AtomicBool>)>,
    pending: Option<Work>,
    last_request: u64,
    stopping: Arc<AtomicBool>,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

fn stop(shared: &Shared) -> Result<(), WorkerError> {
    let mut state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
    state.stop = true;
    state.stopping.store(true, Ordering::Release);
    if let Some((_, cancelled)) = &state.occupied {
        cancelled.store(true, Ordering::Release);
    }
    shared.1.notify_one();
    Ok(())
}

// A stalled stdout must never prevent control input from revoking a job.
// One frame may be in the writer and one in this bounded queue.
fn send(output: &SyncSender<Response>, response: Response) -> Result<(), WorkerError> {
    output
        .try_send(response)
        .map_err(|_| WorkerError::Backpressure)
}

fn input_loop(
    mut input: impl Read,
    output: SyncSender<Response>,
    shared: Shared,
    generation: NonZeroU64,
    disconnect: &dyn Fn(i32),
) -> Result<(), WorkerError> {
    loop {
        let request = match read_request(&mut input) {
            Ok(Some(request)) => request,
            Ok(None) => {
                stop(&shared)?;
                disconnect(0);
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        match request {
            Request::Shutdown {} => {
                stop(&shared)?;
                return Ok(());
            }
            Request::Cancel { job } => {
                let state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
                if let Some((active, cancelled)) = &state.occupied
                    && *active == job
                {
                    cancelled.store(true, Ordering::Release);
                }
            }
            Request::Run {
                job,
                language,
                text,
            } => {
                let rejection = {
                    let mut state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
                    if state.stop
                        || job.generation != generation
                        || job.request.get() <= state.last_request
                    {
                        Some(FailureKind::InvalidRequest)
                    } else if !state.ready {
                        Some(FailureKind::NotReady)
                    } else if state.occupied.is_some() {
                        Some(FailureKind::Busy)
                    } else {
                        let cancelled = Arc::new(AtomicBool::new(false));
                        state.occupied = Some((job, cancelled.clone()));
                        state.last_request = job.request.get();
                        state.pending = Some(Work {
                            job,
                            language,
                            text,
                            cancelled,
                        });
                        shared.1.notify_one();
                        None
                    }
                };
                if let Some(code) = rejection {
                    send(
                        &output,
                        Response::Failed {
                            job: Some(job),
                            code,
                        },
                    )?;
                }
            }
        }
    }
}

/// Runs inside a dedicated child process. The disconnect callback must terminate
/// that process in production: EOF must also stop a blocked native GPU operation.
/// A normal Shutdown instead lets the inference owner release its model.
pub fn serve<R, W, G>(
    input: R,
    output: W,
    identity: Identity,
    idle_timeout: Duration,
    load: impl FnOnce(&AtomicBool) -> Result<G, FailureKind>,
    disconnect: Arc<dyn Fn(i32) + Send + Sync>,
) -> Result<(), WorkerError>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
    G: Generator,
{
    let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
    let (output_tx, output_rx) = mpsc::sync_channel::<Response>(1);
    let write_shared = shared.clone();
    let write_disconnect = disconnect.clone();
    let writer = thread::Builder::new()
        .name("refinement-output".into())
        .spawn(move || {
            let mut output = output;
            while let Ok(response) = output_rx.recv() {
                if write_response(&mut output, &response).is_err() {
                    let stopped = stop(&write_shared);
                    write_disconnect(1);
                    return stopped;
                }
            }
            Ok(())
        })
        .map_err(|_| WorkerError::Thread)?;
    let output = output_tx;
    let read_shared = shared.clone();
    let read_output = output.clone();
    let generation = identity.generation;
    let reader = thread::Builder::new()
        .name("refinement-input".into())
        .spawn(move || {
            let result = input_loop(
                input,
                read_output,
                read_shared.clone(),
                generation,
                &*disconnect,
            );
            if result.is_err() {
                let stopped = stop(&read_shared);
                disconnect(1);
                stopped?;
            }
            result
        })
        .map_err(|_| WorkerError::Thread)?;
    let stopping = shared
        .0
        .lock()
        .map_err(|_| WorkerError::Synchronization)?
        .stopping
        .clone();
    let model = load(&stopping);
    let mut generator = match model {
        Ok(model) => model,
        Err(_) if stopping.load(Ordering::Acquire) => {
            drop(output);
            drop(writer);
            return reader.join().map_err(|_| WorkerError::Thread)?;
        }
        Err(_) => {
            let sent = send(
                &output,
                Response::Failed {
                    job: None,
                    code: FailureKind::ModelLoad,
                },
            );
            let stopped = stop(&shared);
            drop(reader); // The dedicated process exits; stdin may still be blocked.
            sent?;
            stopped?;
            return Err(WorkerError::Load);
        }
    };
    let served = (|| {
        {
            let mut state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
            if state.stop {
                return Ok(true);
            }
            send(
                &output,
                Response::Ready {
                    generation,
                    model_manifest_sha256: identity.model_manifest_sha256,
                    prompt_version: identity.prompt_version,
                    policy_version: identity.policy_version,
                },
            )?;
            state.ready = true;
        }
        loop {
            let deadline = Instant::now() + idle_timeout;
            let work = {
                let mut state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
                while state.pending.is_none() && !state.stop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Ok(false);
                    }
                    state = shared
                        .1
                        .wait_timeout(state, remaining)
                        .map_err(|_| WorkerError::Synchronization)?
                        .0;
                }
                if state.stop {
                    return Ok(true);
                }
                state.pending.take().ok_or(WorkerError::Synchronization)?
            };
            let generated = if work.cancelled.load(Ordering::Acquire) {
                Ok(StagedEdits::default())
            } else {
                generator.generate(&work.text, work.language, &work.cancelled)
            };
            let mut state = shared.0.lock().map_err(|_| WorkerError::Synchronization)?;
            if state.stop {
                return Ok(true);
            }
            let response = if work.cancelled.load(Ordering::Acquire) {
                Response::Cancelled { job: work.job }
            } else {
                match generated {
                    Ok(proposal) => Response::Completed {
                        job: work.job,
                        proposal,
                    },
                    Err(code) => Response::Failed {
                        job: Some(work.job),
                        code,
                    },
                }
            };
            send(&output, response)?;
            if state
                .occupied
                .as_ref()
                .is_some_and(|(job, _)| *job == work.job)
            {
                state.occupied = None;
            }
            if state.stop {
                return Ok(true);
            }
        }
    })();
    let finished = generator.finish().map_err(|_| WorkerError::Cleanup);
    // A requested shutdown/EOF comes from the input thread, which is returning.
    // On idle expiry or transport failure stdin can remain blocked. Detach only
    // as the dedicated process exits; never hold up its bounded termination.
    let joined = if matches!(served, Ok(true)) {
        reader
            .join()
            .map_err(|_| WorkerError::Thread)
            .and_then(|result| result)
    } else {
        drop(reader);
        Ok(())
    };
    let stopped = stop(&shared);
    drop(output);
    // Pipe writes can remain blocked when the parent stops reading. The child
    // process owns this thread and exits after model cleanup; never join a
    // blocked writer or delay its host-enforced termination deadline.
    if writer.is_finished() {
        writer.join().map_err(|_| WorkerError::Thread)??;
    }
    match (served, finished, joined, stopped) {
        (Err(_), Err(_), _, _) => Err(WorkerError::OperationAndCleanup),
        (Err(error), _, _, _)
        | (_, Err(error), _, _)
        | (_, _, Err(error), _)
        | (_, _, _, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
