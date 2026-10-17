//! Session-bound cancellation competes atomically with the one-way send commit.
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

const PENDING: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTED: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    AlreadyCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    #[error("Delivery was cancelled")]
    Cancelled,
    #[error("Delivery was already committed")]
    AlreadyCommitted,
}

#[derive(Clone)]
pub struct DeliveryToken {
    session: NonZeroU64,
    state: Arc<AtomicU8>,
}

impl DeliveryToken {
    pub fn new(session: NonZeroU64) -> Self {
        Self {
            session,
            state: Arc::new(AtomicU8::new(PENDING)),
        }
    }

    pub fn session(&self) -> NonZeroU64 {
        self.session
    }
    pub fn is_pending(&self) -> bool {
        self.state.load(Ordering::Acquire) == PENDING
    }

    pub(crate) fn same_work(&self, other: &Self) -> bool {
        self.session == other.session && Arc::ptr_eq(&self.state, &other.state)
    }

    /// Invoke only after final native checks and event construction. Success
    /// authorizes exactly one send attempt; failure never authorizes a retry.
    pub fn try_commit(&self) -> Result<(), CommitError> {
        match self
            .state
            .compare_exchange(PENDING, COMMITTED, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(CANCELLED) => Err(CommitError::Cancelled),
            Err(_) => Err(CommitError::AlreadyCommitted),
        }
    }

    /// Does not claim to recall a keyboard event after the send commit.
    pub fn cancel(&self) -> CancelOutcome {
        match self
            .state
            .compare_exchange(PENDING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
        {
            Err(COMMITTED) => CancelOutcome::AlreadyCommitted,
            _ => CancelOutcome::Cancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_with_the_same_session_cannot_revive_cancelled_work()
    -> Result<(), Box<dyn std::error::Error>> {
        let session = NonZeroU64::new(7).ok_or("invalid fixture id")?;
        let original = DeliveryToken::new(session);
        let installed = original.clone();
        original.cancel();
        let replacement = DeliveryToken::new(session);
        assert!(!installed.same_work(&replacement));
        assert!(installed.same_work(&original));
        assert_eq!(installed.try_commit(), Err(CommitError::Cancelled));
        assert!(replacement.is_pending());
        Ok(())
    }

    #[test]
    fn cancel_before_send_prevents_all_clones_from_committing()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(NonZeroU64::new(7).ok_or("invalid fixture id")?);
        let clone = token.clone();
        assert_eq!(token.cancel(), CancelOutcome::Cancelled);
        assert_eq!(clone.try_commit(), Err(CommitError::Cancelled));
        assert!(!clone.is_pending());
        assert_eq!(clone.session().get(), 7);
        Ok(())
    }

    #[test]
    fn committed_send_is_exactly_once_and_cannot_be_recalled()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(NonZeroU64::new(7).ok_or("invalid fixture id")?);
        let clone = token.clone();
        token.try_commit()?;
        assert_eq!(clone.try_commit(), Err(CommitError::AlreadyCommitted));
        assert_eq!(clone.cancel(), CancelOutcome::AlreadyCommitted);
        assert!(!token.is_pending());
        Ok(())
    }

    #[test]
    fn racing_cancel_and_commit_choose_exactly_one_irreversible_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(NonZeroU64::new(9).ok_or("invalid fixture id")?);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let cancel_token = token.clone();
        let cancel_barrier = Arc::clone(&barrier);
        let cancel = std::thread::spawn(move || {
            cancel_barrier.wait();
            cancel_token.cancel()
        });
        barrier.wait();
        let committed = token.try_commit();
        let cancelled = cancel.join().map_err(|_| "cancel thread failed")?;
        assert!(matches!(
            (committed, cancelled),
            (Ok(()), CancelOutcome::AlreadyCommitted)
                | (Err(CommitError::Cancelled), CancelOutcome::Cancelled)
        ));
        assert!(!token.is_pending());
        Ok(())
    }
}
