//! One serial owner for AX references. Only scalar identities and receipts cross.
use super::{ProcessIdentity, Target, TargetApproval, TargetError};
use crate::delivery::DeliveryToken;
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const CAPACITY: usize = 8;

enum Request {
    Capture {
        session: NonZeroU64,
        process: ProcessIdentity,
        token: DeliveryToken,
    },
    Approve {
        session: NonZeroU64,
        request: NonZeroU64,
        frontmost: ProcessIdentity,
    },
}

pub enum TargetEvent {
    Captured {
        session: NonZeroU64,
        result: Result<(), TargetError>,
    },
    Approved {
        session: NonZeroU64,
        request: NonZeroU64,
        result: Result<TargetApproval, TargetError>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("Native target request queue is busy")]
    Busy,
    #[error("Native target worker stopped")]
    Stopped,
    #[error("Native target worker could not be started or joined")]
    Thread,
    #[error("Native target replies were not consumed")]
    Backpressure,
    #[error("Native target request has a cancelled or mismatched token")]
    Token,
}

trait Backend: 'static {
    fn capture(&mut self, session: NonZeroU64, process: ProcessIdentity)
    -> Result<(), TargetError>;
    fn approve(&mut self, frontmost: ProcessIdentity) -> Result<TargetApproval, TargetError>;
}

#[derive(Default)]
struct NativeBackend {
    target: Option<Target>,
}
impl Backend for NativeBackend {
    fn capture(
        &mut self,
        session: NonZeroU64,
        process: ProcessIdentity,
    ) -> Result<(), TargetError> {
        self.target = None;
        self.target = Some(Target::capture(session, process)?);
        Ok(())
    }
    fn approve(&mut self, frontmost: ProcessIdentity) -> Result<TargetApproval, TargetError> {
        self.target
            .as_ref()
            .ok_or(TargetError::Changed)?
            .approve(frontmost)
    }
}

struct Entry {
    session: NonZeroU64,
    token: DeliveryToken,
    failed: Option<TargetError>,
}

pub struct TargetWorker {
    requests: SyncSender<Request>,
    events: Receiver<TargetEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), WorkerError>>>,
}

impl TargetWorker {
    pub fn start() -> Result<Self, WorkerError> {
        Self::with_backend(NativeBackend::default)
    }

    fn with_backend<B: Backend>(
        factory: impl FnOnce() -> B + Send + 'static,
    ) -> Result<Self, WorkerError> {
        let (requests, receive) = mpsc::sync_channel(CAPACITY);
        let (replies, events) = mpsc::sync_channel(CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("native-target".into())
            .spawn(move || {
                // Backend is constructed here, so its !Send AX references never
                // leave this thread, including when replaced or dropped.
                let mut backend = factory();
                let mut current: Option<Entry> = None;
                let mut highest_capture = None;
                while !stopping.load(Ordering::Acquire) {
                    let request = match receive.recv_timeout(Duration::from_millis(20)) {
                        Ok(request) => request,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                    };
                    let event = match request {
                        Request::Capture {
                            session,
                            process,
                            token,
                        } => {
                            let result = if highest_capture.is_some_and(|seen| seen >= session) {
                                Err(TargetError::Changed)
                            } else {
                                // A cancelled first attempt still consumes its
                                // session ID; a fresh token cannot recapture it.
                                highest_capture = Some(session);
                                let result = if token.is_pending() {
                                    backend.capture(session, process)
                                } else {
                                    Err(TargetError::Cancelled)
                                };
                                current = Some(Entry {
                                    session,
                                    token: token.clone(),
                                    failed: result.as_ref().err().cloned(),
                                });
                                result
                            };
                            let result = if token.is_pending() {
                                result
                            } else {
                                Err(TargetError::Cancelled)
                            };
                            TargetEvent::Captured { session, result }
                        }
                        Request::Approve {
                            session,
                            request,
                            frontmost,
                        } => {
                            let result = match current.as_ref() {
                                Some(entry)
                                    if entry.session == session && entry.token.is_pending() =>
                                {
                                    let result = match &entry.failed {
                                        Some(error) => Err(error.clone()),
                                        None => backend.approve(frontmost),
                                    };
                                    if entry.token.is_pending() {
                                        result
                                    } else {
                                        Err(TargetError::Cancelled)
                                    }
                                }
                                _ => Err(TargetError::Cancelled),
                            };
                            TargetEvent::Approved {
                                session,
                                request,
                                result,
                            }
                        }
                    };
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    match replies.try_send(event) {
                        Ok(()) => {}
                        Err(TrySendError::Full(_)) => return Err(WorkerError::Backpressure),
                        Err(TrySendError::Disconnected(_)) => return Ok(()),
                    }
                }
                Ok(())
            })
            .map_err(|_| WorkerError::Thread)?;
        Ok(Self {
            requests,
            events,
            stop,
            worker: Some(worker),
        })
    }

    pub fn capture(
        &self,
        session: NonZeroU64,
        process: ProcessIdentity,
        token: DeliveryToken,
    ) -> Result<(), WorkerError> {
        if token.session() != session || !token.is_pending() {
            return Err(WorkerError::Token);
        }
        self.submit(Request::Capture {
            session,
            process,
            token,
        })
    }

    pub fn approve(
        &self,
        session: NonZeroU64,
        request: NonZeroU64,
        frontmost: ProcessIdentity,
    ) -> Result<(), WorkerError> {
        self.submit(Request::Approve {
            session,
            request,
            frontmost,
        })
    }

    fn submit(&self, request: Request) -> Result<(), WorkerError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(WorkerError::Stopped);
        }
        self.requests
            .try_send(request)
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkerError::Busy,
                TrySendError::Disconnected(_) => WorkerError::Stopped,
            })
    }

    pub fn next_event(&self) -> Result<Option<TargetEvent>, WorkerError> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(WorkerError::Stopped),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Join only outside the GUI thread. Individual AX messages have timeouts.
    pub fn shutdown(mut self) -> Result<(), WorkerError> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .ok_or(WorkerError::Thread)?
            .join()
            .map_err(|_| WorkerError::Thread)?
    }
}

impl Drop for TargetWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::AtomicUsize, time::Instant};

    fn id(value: u64) -> Result<NonZeroU64, Box<dyn std::error::Error>> {
        NonZeroU64::new(value).ok_or_else(|| "invalid fixture id".into())
    }
    fn process() -> ProcessIdentity {
        ProcessIdentity {
            pid: 123,
            start_seconds: 4,
            start_microseconds: 5,
        }
    }

    fn event(worker: &TargetWorker) -> Result<TargetEvent, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(event) = worker.next_event()? {
                return Ok(event);
            }
            if Instant::now() >= deadline {
                return Err("missing worker response".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    struct ControlledBackend {
        entered: SyncSender<NonZeroU64>,
        release: Receiver<()>,
        session: Option<NonZeroU64>,
    }

    #[test]
    fn cancelled_queued_capture_cannot_be_retried_with_a_fresh_token()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = mpsc::sync_channel(2);
        let (release, receive) = mpsc::sync_channel(2);
        let worker = TargetWorker::with_backend(move || ControlledBackend {
            entered,
            release: receive,
            session: None,
        })?;
        worker.capture(id(1)?, process(), DeliveryToken::new(id(1)?))?;
        started.recv_timeout(Duration::from_secs(1))?;
        let cancelled = DeliveryToken::new(id(2)?);
        worker.capture(id(2)?, process(), cancelled.clone())?;
        cancelled.cancel();
        worker.capture(id(2)?, process(), DeliveryToken::new(id(2)?))?;
        release.send(())?;
        release.send(())?;
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Captured { result: Ok(()), .. }
        ));
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Captured {
                result: Err(TargetError::Cancelled),
                ..
            }
        ));
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Captured {
                result: Err(TargetError::Changed),
                ..
            }
        ));
        assert!(matches!(started.try_recv(), Err(TryRecvError::Empty)));
        worker.shutdown()?;
        Ok(())
    }
    impl Backend for ControlledBackend {
        fn capture(&mut self, session: NonZeroU64, _: ProcessIdentity) -> Result<(), TargetError> {
            self.entered.send(session).map_err(|_| TargetError::Value)?;
            self.release
                .recv_timeout(Duration::from_secs(1))
                .map_err(|_| TargetError::Value)?;
            self.session = Some(session);
            Ok(())
        }
        fn approve(&mut self, process: ProcessIdentity) -> Result<TargetApproval, TargetError> {
            Ok(TargetApproval {
                session: self.session.ok_or(TargetError::Value)?,
                process,
                checked_at: Instant::now(),
            })
        }
    }

    #[test]
    fn cancellation_discards_an_inflight_capture_and_new_session_receipts_keep_tags()
    -> Result<(), Box<dyn std::error::Error>> {
        let (entered, started) = mpsc::sync_channel(1);
        let (release, receive) = mpsc::sync_channel(1);
        let worker = TargetWorker::with_backend(move || ControlledBackend {
            entered,
            release: receive,
            session: None,
        })?;
        let first = DeliveryToken::new(id(1)?);
        worker.capture(id(1)?, process(), first.clone())?;
        assert_eq!(started.recv_timeout(Duration::from_secs(1))?, id(1)?);
        first.cancel();
        release.send(())?;
        assert!(
            matches!(event(&worker)?, TargetEvent::Captured { session, result: Err(TargetError::Cancelled) } if session == id(1)?)
        );
        let second = DeliveryToken::new(id(2)?);
        worker.capture(id(2)?, process(), second)?;
        started.recv_timeout(Duration::from_secs(1))?;
        release.send(())?;
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Captured { result: Ok(()), .. }
        ));
        worker.approve(id(2)?, id(7)?, process())?;
        match event(&worker)? {
            TargetEvent::Approved {
                session,
                request,
                result: Ok(receipt),
            } => {
                assert_eq!((session, request), (id(2)?, id(7)?));
                receipt.validate(id(2)?, process())?;
            }
            _ => return Err("wrong response".into()),
        }
        worker.approve(id(1)?, id(8)?, process())?;
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Approved {
                result: Err(TargetError::Cancelled),
                ..
            }
        ));
        worker.shutdown()?;
        Ok(())
    }

    #[test]
    fn blocked_ax_cannot_make_the_request_queue_unbounded() -> Result<(), Box<dyn std::error::Error>>
    {
        let (entered, started) = mpsc::sync_channel(1);
        let (release, receive) = mpsc::sync_channel(1);
        let worker = TargetWorker::with_backend(move || ControlledBackend {
            entered,
            release: receive,
            session: None,
        })?;
        worker.capture(id(1)?, process(), DeliveryToken::new(id(1)?))?;
        started.recv_timeout(Duration::from_secs(1))?;
        for value in 1..=8 {
            worker.approve(id(1)?, id(value)?, process())?;
        }
        assert!(matches!(
            worker.approve(id(1)?, id(9)?, process()),
            Err(WorkerError::Busy)
        ));
        worker.stop.store(true, Ordering::Release);
        release.send(())?;
        worker.shutdown()?;
        Ok(())
    }

    #[test]
    fn missing_start_target_never_retries_capture_during_later_approval()
    -> Result<(), Box<dyn std::error::Error>> {
        struct MissingBackend(Arc<AtomicUsize>);
        impl Backend for MissingBackend {
            fn capture(&mut self, _: NonZeroU64, _: ProcessIdentity) -> Result<(), TargetError> {
                Err(TargetError::Field(
                    super::super::HoldReason::UnconfirmedField,
                ))
            }
            fn approve(&mut self, _: ProcessIdentity) -> Result<TargetApproval, TargetError> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Err(TargetError::Value)
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let worker = TargetWorker::with_backend(move || MissingBackend(observed))?;
        worker.capture(id(1)?, process(), DeliveryToken::new(id(1)?))?;
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Captured {
                result: Err(TargetError::Field(_)),
                ..
            }
        ));
        worker.approve(id(1)?, id(1)?, process())?;
        assert!(matches!(
            event(&worker)?,
            TargetEvent::Approved {
                result: Err(TargetError::Field(_)),
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        worker.shutdown()?;
        Ok(())
    }
}
