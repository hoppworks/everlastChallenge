//! Asynchronous, single-slot ownership for the local text worker.
use super::{Cancellation, Error, Failure, Limits, Worker, WorkerSpec};
use dictation_core::refinement::{AcceptedTranscript, OriginalTranscript};
use std::{
    num::NonZeroU64,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use text_refinement_protocol::{JobKey, Language};

pub struct Job {
    pub session: NonZeroU64,
    pub original: OriginalTranscript,
    pub language: Language,
    pub cancel: Cancellation,
}
pub struct Completed {
    pub session: NonZeroU64,
    pub original: OriginalTranscript,
    pub result: Result<AcceptedTranscript, Error>,
    cancel: Cancellation,
}
#[derive(Debug, thiserror::Error)]
pub enum OwnerError {
    #[error("Text refinement is occupied")]
    Busy,
    #[error("Text refinement is closed")]
    Closed,
    #[error("Text refinement job is invalid or replayed")]
    InvalidJob,
    #[error("Text refinement synchronization failed")]
    Synchronization,
    #[error("Text refinement thread failed")]
    Thread,
    #[error(transparent)]
    Worker(#[from] Error),
}
#[derive(Default)]
struct State {
    closing: bool,
    unload: bool,
    pending: Option<Job>,
    active_cancel: Option<Cancellation>,
    completed: Option<Completed>,
    last_session: u64,
}
impl State {
    fn revoke(&self) {
        if let Some(cancel) = &self.active_cancel {
            cancel.cancel();
        }
        if let Some(completed) = &self.completed {
            completed.cancel.cancel();
        }
    }
}
type Shared = Arc<(Mutex<State>, Condvar)>;

pub struct Owner {
    shared: Shared,
    thread: Option<JoinHandle<Result<(), OwnerError>>>,
}

// Fail closed even when the service unwinds. Teardown may recover a poisoned
// lock solely to revoke work and close admission; normal operations never do.
struct Exit(Shared);
impl Drop for Exit {
    fn drop(&mut self) {
        let mut state = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
        state.closing = true;
        state.revoke();
        self.0.1.notify_all();
    }
}
impl Owner {
    /// Creates a service thread. Model loading is deferred until a submitted job.
    pub fn spawn(spec: WorkerSpec, limits: Limits, idle: Duration) -> Result<Self, OwnerError> {
        if idle.is_zero() || Instant::now().checked_add(idle).is_none() {
            return Err(OwnerError::InvalidJob);
        }
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let service = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("text-refinement-owner".into())
            .spawn(move || {
                let _exit = Exit(Arc::clone(&service));
                serve(service, spec, limits, idle)
            })
            .map_err(|_| OwnerError::Thread)?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    /// One occupied slot includes active work and an unconsumed completion.
    pub fn submit(&self, job: Job) -> Result<(), OwnerError> {
        let mut state = self
            .shared
            .0
            .lock()
            .map_err(|_| OwnerError::Synchronization)?;
        if state.closing {
            return Err(OwnerError::Closed);
        }
        if state.active_cancel.is_some() || state.completed.is_some() {
            return Err(OwnerError::Busy);
        }
        if job.session.get() <= state.last_session {
            return Err(OwnerError::InvalidJob);
        }
        if job.cancel.is_cancelled() {
            return Err(Error::from(Failure::Cancelled).into());
        }
        state.last_session = job.session.get();
        state.active_cancel = Some(job.cancel.clone());
        state.pending = Some(job);
        self.shared.1.notify_one();
        Ok(())
    }
    pub fn take_result(&self) -> Result<Option<Completed>, OwnerError> {
        let mut state = self
            .shared
            .0
            .lock()
            .map_err(|_| OwnerError::Synchronization)?;
        // Fatal exit revokes even a stored success. Report the service failure
        // before examining its token, otherwise that revocation looks like a
        // user cancellation. Shutdown/join retains the concrete failure cause.
        if state.closing {
            return Err(OwnerError::Closed);
        }
        if let Some(mut completed) = state.completed.take() {
            if completed.cancel.is_cancelled() && completed.result.is_ok() {
                completed.result = Err(Failure::Cancelled.into());
                state.unload = true;
                self.shared.1.notify_one();
            }
            return Ok(Some(completed));
        }
        Ok(None)
    }
    pub fn shutdown(mut self) -> Result<(), OwnerError> {
        self.stop()
    }
    pub fn unload(&self) -> Result<(), OwnerError> {
        let mut state = self
            .shared
            .0
            .lock()
            .map_err(|_| OwnerError::Synchronization)?;
        if state.closing {
            return Err(OwnerError::Closed);
        }
        state.revoke();
        state.unload = true;
        self.shared.1.notify_one();
        Ok(())
    }
    fn stop(&mut self) -> Result<(), OwnerError> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let poisoned = match self.shared.0.lock() {
            Ok(mut state) => {
                state.closing = true;
                state.revoke();
                false
            }
            Err(error) => {
                let mut state = error.into_inner();
                state.closing = true;
                state.revoke();
                true
            }
        };
        self.shared.1.notify_all();
        thread.join().map_err(|_| OwnerError::Thread)??;
        if poisoned {
            Err(OwnerError::Synchronization)
        } else {
            Ok(())
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Local text refinement teardown failed: {error}");
        }
    }
}

fn close(worker: &mut Option<Worker>) -> Result<(), Error> {
    if let Some(mut owned) = worker.take() {
        owned.close()?;
    }
    Ok(())
}

fn reap_exited(worker: &mut Option<Worker>) -> Result<(), Error> {
    let status = match worker.as_mut() {
        Some(worker) => worker
            .child
            .as_mut()
            .ok_or(Failure::Closed)?
            .try_wait()
            .map_err(|_| Error::from(Failure::Transport))?,
        None => None,
    };
    if let Some(status) = status {
        let cleanup = close(worker);
        if !status.success() {
            return Err(Error {
                reason: Failure::Exited,
                cleanup_failed: cleanup.is_err(),
            });
        }
        cleanup?;
    }
    Ok(())
}

fn serve(
    shared: Shared,
    spec: WorkerSpec,
    limits: Limits,
    idle: Duration,
) -> Result<(), OwnerError> {
    let mut worker = None;
    let mut generation = Some(spec.identity.generation);
    let mut idle_deadline: Option<Instant> = None;
    loop {
        let mut state = shared.0.lock().map_err(|_| OwnerError::Synchronization)?;
        while !state.closing && !state.unload && state.pending.is_none() {
            if let Some(deadline) = idle_deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                state = shared
                    .1
                    .wait_timeout(state, remaining.min(Duration::from_millis(100)))
                    .map_err(|_| OwnerError::Synchronization)?
                    .0;
            } else {
                state = shared
                    .1
                    .wait(state)
                    .map_err(|_| OwnerError::Synchronization)?;
            }
            if !state.closing && !state.unload && state.pending.is_none() {
                drop(state);
                reap_exited(&mut worker)?;
                if worker.is_none() {
                    idle_deadline = None;
                }
                state = shared.0.lock().map_err(|_| OwnerError::Synchronization)?;
            }
        }
        if state.closing {
            drop(state);
            close(&mut worker)?;
            return Ok(());
        }
        if state.unload || idle_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            state.unload = false;
            drop(state);
            close(&mut worker)?;
            idle_deadline = None;
            continue;
        }
        let job = state.pending.take().ok_or(OwnerError::Synchronization)?;
        drop(state);
        let mut result = run(&mut worker, &mut generation, &spec, limits, &job);
        if job.cancel.is_cancelled() && result.is_ok() {
            result = Err(Failure::Cancelled.into());
        }
        if let Err(error) = &mut result {
            error.cleanup_failed |= close(&mut worker).is_err();
        }
        idle_deadline = if worker.is_some() {
            Some(
                Instant::now()
                    .checked_add(idle)
                    .ok_or(OwnerError::InvalidJob)?,
            )
        } else {
            None
        };
        let mut state = shared.0.lock().map_err(|_| OwnerError::Synchronization)?;
        state.active_cancel = None;
        if state.closing {
            if let Err(error) = result
                && error.cleanup_failed
            {
                return Err(error.into());
            }
        } else {
            state.completed = Some(Completed {
                session: job.session,
                original: job.original,
                result,
                cancel: job.cancel,
            });
        }
    }
}

fn run(
    worker: &mut Option<Worker>,
    generation: &mut Option<NonZeroU64>,
    spec: &WorkerSpec,
    limits: Limits,
    job: &Job,
) -> Result<AcceptedTranscript, Error> {
    if job.cancel.is_cancelled() {
        return Err(Failure::Cancelled.into());
    }
    reap_exited(worker)?;
    if worker.is_none() {
        let current = generation.ok_or(Failure::InvalidRequest)?;
        *generation = current.get().checked_add(1).and_then(NonZeroU64::new);
        let mut spec = spec.clone();
        spec.identity.generation = current;
        *worker = Some(Worker::spawn(spec, limits, &job.cancel)?);
    }
    let worker = worker.as_mut().ok_or(Failure::Closed)?;
    worker.run(
        JobKey {
            session: job.session,
            generation: worker.expected.generation,
            request: job.session,
        },
        &job.original,
        job.language,
        &job.cancel,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatal_close_precedes_an_unconsumed_completion() -> Result<(), Box<dyn std::error::Error>> {
        let original = OriginalTranscript::new("Hello.".into())?;
        let accepted =
            dictation_core::refinement::validate(&original, &[], Language::English.into())?;
        let cancel = Cancellation::new();
        let shared = Arc::new((
            Mutex::new(State {
                completed: Some(Completed {
                    session: NonZeroU64::new(1).ok_or("zero session")?,
                    original,
                    result: Ok(accepted),
                    cancel: cancel.clone(),
                }),
                ..State::default()
            }),
            Condvar::new(),
        ));
        let owner = Owner {
            shared: Arc::clone(&shared),
            thread: None,
        };
        // Exercise the actual fatal-exit guard after a result was stored. This
        // ordering is not established by a child's pipe-write acknowledgement.
        drop(Exit(shared));
        assert!(cancel.is_cancelled());
        assert!(matches!(owner.take_result(), Err(OwnerError::Closed)));
        Ok(())
    }
}
