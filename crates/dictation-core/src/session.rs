//! Session identity and ordering. Worker/OS effects are executed by the host.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Starting,
    Recording,
    Finalizing,
    Delivering,
    Cancelling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Outcome {
    Local,
    Empty,
    Sent,
    Held(String),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Capture(u64),
    StopAudio(u64),
    Transcribe(u64),
    Deliver(u64),
    Revoke(u64),
}

pub enum Fact {
    AudioStarted(u64),
    AudioReleased(u64),
    Transcript { id: u64, text: String },
    Delivered { id: u64, result: DeliveryResult },
    Failed { id: u64, message: String },
}

pub enum DeliveryResult {
    Sent,
    Held(String),
    Failed(String),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("A dictation is already active")]
    Busy,
    #[error("The microphone, model or required permissions are not ready")]
    NotReady,
    #[error("Session identifiers exhausted; restart the application")]
    IdentifiersExhausted,
    #[error("The paste command has already been sent")]
    DeliveryCommitted,
}

#[derive(Default)]
pub struct Machine {
    phase: Phase,
    next_id: u64,
    session_id: Option<u64>,
    stop_pending: bool,
    released: bool,
    delivery: bool,
    last_text: Option<String>,
    outcome: Option<Outcome>,
    last_held_session: Option<u64>,
}

impl Machine {
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn session_id(&self) -> Option<u64> {
        self.session_id
    }
    /// Most recent held result, retained across starts and transcript clearing.
    pub fn last_held_session(&self) -> Option<u64> {
        self.last_held_session
    }
    pub fn clear_text(&mut self) {
        self.last_text = None;
    }
    pub fn last_text(&self) -> Option<&str> {
        self.last_text.as_deref()
    }
    pub fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    pub fn start(&mut self, ready: bool, delivery: bool) -> Result<Vec<Effect>, SessionError> {
        if self.phase != Phase::Idle {
            return Err(SessionError::Busy);
        }
        if !ready {
            return Err(SessionError::NotReady);
        }
        if self.next_id >= u64::MAX >> 2 {
            return Err(SessionError::IdentifiersExhausted);
        }
        self.next_id += 1;
        self.session_id = Some(self.next_id);
        self.stop_pending = false;
        self.released = false;
        self.delivery = delivery;
        self.outcome = None;
        self.phase = Phase::Starting;
        Ok(vec![Effect::Capture(self.next_id)])
    }

    pub fn stop(&mut self, id: u64) -> Result<Vec<Effect>, SessionError> {
        if self.session_id != Some(id) {
            return Ok(Vec::new());
        }
        match self.phase {
            Phase::Starting => {
                self.stop_pending = true;
                Ok(Vec::new())
            }
            Phase::Recording => {
                self.phase = Phase::Finalizing;
                Ok(vec![Effect::StopAudio(id)])
            }
            _ => Ok(Vec::new()),
        }
    }

    /// The host atomically revokes the native delivery token before this call.
    /// If its commit already won, cancellation cannot retract the sent event.
    pub fn cancel(
        &mut self,
        id: u64,
        delivery_committed: bool,
    ) -> Result<Vec<Effect>, SessionError> {
        if self.session_id != Some(id) || self.phase == Phase::Cancelling {
            return Ok(Vec::new());
        }
        if self.phase == Phase::Delivering && delivery_committed {
            return Err(SessionError::DeliveryCommitted);
        }
        self.outcome = Some(Outcome::Cancelled);
        if self.released {
            self.finish(Outcome::Cancelled);
            Ok(vec![Effect::Revoke(id)])
        } else {
            self.phase = Phase::Cancelling;
            Ok(vec![Effect::Revoke(id), Effect::StopAudio(id)])
        }
    }

    pub fn accept(&mut self, fact: Fact) -> Result<Vec<Effect>, SessionError> {
        let id = match &fact {
            Fact::AudioStarted(id) | Fact::AudioReleased(id) => *id,
            Fact::Transcript { id, .. } | Fact::Delivered { id, .. } | Fact::Failed { id, .. } => {
                *id
            }
        };
        if self.session_id != Some(id) {
            return Ok(Vec::new());
        }
        match fact {
            Fact::Failed { message, .. } => {
                self.outcome = Some(Outcome::Failed(message));
                if self.released {
                    let outcome = self.outcome.take().unwrap_or(Outcome::Cancelled);
                    self.finish(outcome);
                    Ok(vec![Effect::Revoke(id)])
                } else {
                    self.phase = Phase::Cancelling;
                    Ok(vec![Effect::Revoke(id), Effect::StopAudio(id)])
                }
            }
            Fact::Delivered { result, .. } if self.phase == Phase::Delivering => {
                self.finish(match result {
                    DeliveryResult::Sent => Outcome::Sent,
                    DeliveryResult::Held(reason) => Outcome::Held(reason),
                    DeliveryResult::Failed(error) => Outcome::Failed(error),
                });
                Ok(Vec::new())
            }
            Fact::AudioStarted(_) if self.phase == Phase::Cancelling => {
                Ok(vec![Effect::StopAudio(id)])
            }
            Fact::AudioReleased(_) if self.phase == Phase::Cancelling => {
                self.released = true;
                let outcome = self.outcome.take().unwrap_or(Outcome::Cancelled);
                self.finish(outcome);
                Ok(Vec::new())
            }
            Fact::AudioStarted(_) if self.phase == Phase::Starting => {
                self.phase = Phase::Recording;
                if self.stop_pending {
                    self.stop(id)
                } else {
                    Ok(Vec::new())
                }
            }
            Fact::AudioReleased(_) if self.phase == Phase::Finalizing && !self.released => {
                self.released = true;
                Ok(vec![Effect::Transcribe(id)])
            }
            Fact::Transcript { text, .. } if self.phase == Phase::Finalizing && self.released => {
                if text.is_empty() {
                    self.finish(Outcome::Empty);
                    Ok(Vec::new())
                } else {
                    self.last_text = Some(text);
                    if self.delivery {
                        self.phase = Phase::Delivering;
                        Ok(vec![Effect::Deliver(id)])
                    } else {
                        self.finish(Outcome::Local);
                        Ok(Vec::new())
                    }
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    fn finish(&mut self, outcome: Outcome) {
        if matches!(outcome, Outcome::Held(_)) {
            self.last_held_session = self.session_id;
        }
        self.phase = Phase::Idle;
        self.session_id = None;
        self.outcome = Some(outcome);
    }
}
