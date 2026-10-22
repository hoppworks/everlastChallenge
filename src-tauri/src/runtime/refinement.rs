//! Runtime seam for the asynchronous local text owner; no GPU work on the actor.
use super::*;
use crate::refinement_service::{self, owner};
use dictation_core::refinement::{AcceptedTranscript, OriginalTranscript};
pub(super) const MEMORY_PRESSURE: &str =
    "Textbereinigung wegen Speicherdruck abgebrochen. Das Original bleibt erhalten.";

pub(super) struct Completed {
    pub session: u64,
    pub original: OriginalTranscript,
    pub result: Result<AcceptedTranscript, refinement_service::Error>,
}
pub(super) trait Service: Send {
    fn submit(&self, job: owner::Job) -> Result<(), String>;
    fn poll(&self) -> Result<Option<Completed>, String>;
    fn unload(&self) -> Result<(), String>;
    fn shutdown(self: Box<Self>) -> Result<(), String>;
}
impl Service for owner::Owner {
    fn submit(&self, job: owner::Job) -> Result<(), String> {
        owner::Owner::submit(self, job).map_err(|e| e.to_string())
    }
    fn poll(&self) -> Result<Option<Completed>, String> {
        self.take_result()
            .map(|result| {
                result.map(|done| Completed {
                    session: done.session.get(),
                    original: done.original,
                    result: done.result,
                })
            })
            .map_err(|e| e.to_string())
    }
    fn unload(&self) -> Result<(), String> {
        owner::Owner::unload(self).map_err(|e| e.to_string())
    }
    fn shutdown(self: Box<Self>) -> Result<(), String> {
        owner::Owner::shutdown(*self).map_err(|e| e.to_string())
    }
}

impl<A: AudioDriver> Actor<A> {
    pub(super) fn hold_for_memory_pressure(&mut self) -> Result<(), RuntimeError> {
        let actions = {
            let mut shared = self
                .shared
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            match shared.pressure_hold.take() {
                Some(session) => {
                    shared.refinement_active = None;
                    self.controller
                        .refinement_unavailable(session, MEMORY_PRESSURE.into())
                        .map_err(service_error)?
                }
                None => vec![],
            }
        };
        self.execute(actions)
    }
    pub(super) fn poll_refinement(&mut self) -> Result<(), RuntimeError> {
        self.hold_for_memory_pressure()?;
        let unload = {
            let mut shared = self
                .shared
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            std::mem::take(&mut shared.refinement_unload_requested)
        };
        if unload
            && let Some(service) = &self.refinement
            && let Err(error) = service.unload()
        {
            self.retire_refinement(error)?;
        }
        let suspended = self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?
            .suspended;
        if suspended != self.refinement_suspended {
            self.refinement_suspended = suspended;
            if suspended
                && let Some(service) = &self.refinement
                && let Err(error) = service.unload()
            {
                self.retire_refinement(error)?;
            }
        }
        let result = self.refinement.as_ref().map(|service| service.poll());
        match result {
            Some(Ok(Some(done))) => {
                let actions = self
                    .accept(Event::RefinementFinished {
                        session: done.session,
                        original: done.original,
                        result: done.result,
                    })
                    .map_err(service_error)?;
                self.execute(actions)?;
            }
            Some(Err(message)) => self.retire_refinement(message)?,
            _ => {}
        }
        Ok(())
    }
    fn retire_refinement(&mut self, mut message: String) -> Result<(), RuntimeError> {
        if let Some(session) = self.controller.snapshot().session {
            let actions = {
                let shared = Arc::clone(&self.shared);
                let _admission = shared.lock().map_err(|_| RuntimeError::Synchronization)?;
                self.controller
                    .refinement_unavailable(session, message.clone())
                    .map_err(service_error)?
            };
            self.execute(actions)?;
        }
        if let Some(service) = self.refinement.take() {
            let cleanup = catch_unwind(AssertUnwindSafe(|| service.shutdown()))
                .unwrap_or_else(|_| Err("Text refinement cleanup panicked".into()));
            if let Err(error) = cleanup {
                message.push_str(&format!("; cleanup: {error}"));
            }
        }
        self.error = Some(message);
        Ok(())
    }
}
