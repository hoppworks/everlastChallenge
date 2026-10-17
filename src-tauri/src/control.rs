//! Bounded requests and independent idempotent session-bound priority control.

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MailboxError {
    #[error("The request queue is full")]
    Full,
    #[error("The controller is closed")]
    Closed,
    #[error("Invalid session identifier")]
    InvalidSession,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Control<T> {
    Request(T),
    Stop(u64),
    Cancel(u64),
    Shutdown,
}

struct Priority {
    // Packed monotone session ID plus Stop=1/Cancel=2. A stale sender cannot
    // overwrite newer pending work; Cancel wins over Stop for the same ID.
    command: AtomicU64,
    shutdown: AtomicBool,
}

pub struct ControlTx<T> {
    requests: Sender<T>,
    wake: Sender<()>,
    priority: Arc<Priority>,
}

impl<T> Clone for ControlTx<T> {
    fn clone(&self) -> Self {
        Self {
            requests: self.requests.clone(),
            wake: self.wake.clone(),
            priority: Arc::clone(&self.priority),
        }
    }
}

pub struct ControlRx<T> {
    requests: Receiver<T>,
    wake: Receiver<()>,
    priority: Arc<Priority>,
    #[cfg(test)]
    before_wait: Option<Box<dyn Fn() + Send>>,
}

/// Capacity has a minimum of one; wakeups occupy a separate single slot.
pub fn mailbox<T>(capacity: usize) -> (ControlTx<T>, ControlRx<T>) {
    let (requests, incoming) = bounded(capacity.max(1));
    let (wake, awakened) = bounded(1);
    let priority = Arc::new(Priority {
        command: AtomicU64::new(0),
        shutdown: AtomicBool::new(false),
    });
    (
        ControlTx {
            requests,
            wake,
            priority: Arc::clone(&priority),
        },
        ControlRx {
            requests: incoming,
            wake: awakened,
            priority,
            #[cfg(test)]
            before_wait: None,
        },
    )
}

impl<T> ControlTx<T> {
    pub fn request(&self, request: T) -> Result<(), MailboxError> {
        if self.priority.shutdown.load(Ordering::Acquire) {
            return Err(MailboxError::Closed);
        }
        self.requests
            .try_send(request)
            .map_err(|error| match error {
                TrySendError::Full(_) => MailboxError::Full,
                TrySendError::Disconnected(_) => MailboxError::Closed,
            })?;
        self.notify()
    }

    pub fn stop(&self, session: u64) -> Result<(), MailboxError> {
        self.priority(session, 1)
    }
    pub fn cancel(&self, session: u64) -> Result<(), MailboxError> {
        self.priority(session, 2)
    }

    fn priority(&self, session: u64, kind: u64) -> Result<(), MailboxError> {
        if self.priority.shutdown.load(Ordering::Acquire) {
            return Err(MailboxError::Closed);
        }
        if session == 0 || session > (u64::MAX >> 2) {
            return Err(MailboxError::InvalidSession);
        }
        self.priority
            .command
            .fetch_max((session << 2) | kind, Ordering::AcqRel);
        self.notify()
    }

    fn notify(&self) -> Result<(), MailboxError> {
        match self.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(()),
            Err(TrySendError::Disconnected(())) => Err(MailboxError::Closed),
        }
    }

    pub fn shutdown(&self) -> Result<(), MailboxError> {
        self.priority.shutdown.store(true, Ordering::Release);
        self.notify()
    }
}

impl<T> ControlRx<T> {
    pub fn poll(&self) -> Option<Control<T>> {
        if self.priority.shutdown.load(Ordering::Acquire) {
            return Some(Control::Shutdown);
        }
        let pending = self.priority.command.swap(0, Ordering::AcqRel);
        if pending != 0 {
            return Some(if pending & 3 == 2 {
                Control::Cancel(pending >> 2)
            } else {
                Control::Stop(pending >> 2)
            });
        }
        self.requests.try_recv().ok().map(Control::Request)
    }

    /// Wait only on the controller thread. GUI/callback senders never block.
    pub fn wait(&self, timeout: Duration) -> Option<Control<T>> {
        if let Some(control) = self.poll() {
            return Some(control);
        }
        // Test-only rendezvous after the initial poll: production has no hook.
        #[cfg(test)]
        if let Some(before_wait) = &self.before_wait {
            before_wait();
        }
        match self.wake.recv_timeout(timeout) {
            Ok(())
            | Err(crossbeam_channel::RecvTimeoutError::Timeout)
            | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => self.poll(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_cancel_and_shutdown_survive_a_saturated_request_queue()
    -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mailbox(1);
        tx.request("start")?;
        assert!(tx.request("overflow").is_err());
        tx.stop(1)?;
        tx.cancel(1)?;
        assert_eq!(rx.poll(), Some(Control::Cancel(1)));
        assert_eq!(rx.poll(), Some(Control::Request("start")));
        tx.request("next")?;
        tx.shutdown()?;
        assert_eq!(rx.poll(), Some(Control::Shutdown));
        Ok(())
    }

    #[test]
    fn stale_priorities_cannot_replace_newer_pending_control() -> Result<(), MailboxError> {
        let (tx, rx) = mailbox::<()>(1);
        tx.cancel(2)?;
        tx.stop(1)?;
        tx.stop(2)?;
        assert_eq!(rx.wait(Duration::from_millis(1)), Some(Control::Cancel(2)));
        assert_eq!(rx.poll(), None);
        assert_eq!(tx.stop(0), Err(MailboxError::InvalidSession));
        assert_eq!(tx.cancel(u64::MAX), Err(MailboxError::InvalidSession));
        tx.shutdown()?;
        assert_eq!(tx.request(()), Err(MailboxError::Closed));
        assert_eq!(tx.stop(2), Err(MailboxError::Closed));
        Ok(())
    }

    #[test]
    fn priority_wakes_a_waiting_receiver() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, mut rx) = mailbox::<()>(1);
        let (waiting, ready) = crossbeam_channel::bounded(0);
        rx.before_wait = Some(Box::new(move || assert!(waiting.send(()).is_ok())));
        let worker = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let control = rx.wait(Duration::from_secs(5));
            (control, started.elapsed())
        });
        ready.recv_timeout(Duration::from_secs(1))?;
        tx.stop(1)?;
        let (control, elapsed) = worker.join().map_err(|_| "test worker panicked")?;
        assert_eq!(control, Some(Control::Stop(1)));
        assert!(
            elapsed < Duration::from_secs(1),
            "priority was only noticed after waiting: {elapsed:?}"
        );
        Ok(())
    }
}
