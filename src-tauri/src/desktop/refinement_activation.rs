//! One explicit setup request may enable refinement after its own completion.
use crate::{refinement_setup::Snapshot, setup::Phase};
use std::sync::Arc;

pub(super) type Ticket = Arc<()>;

#[derive(Default)]
pub(super) struct Activation {
    requested_after: Option<(u64, Ticket)>,
}
impl Activation {
    pub fn request(&mut self, revision: u64) {
        self.requested_after = Some((revision, Arc::new(())));
    }
    pub fn cancel(&mut self) {
        self.requested_after = None;
    }
    pub fn ticket(&self) -> Option<Ticket> {
        self.requested_after
            .as_ref()
            .map(|(_, ticket)| Arc::clone(ticket))
    }
    pub fn claim(&mut self, ticket: &Ticket) -> bool {
        let matches = self
            .requested_after
            .as_ref()
            .is_some_and(|(_, current)| Arc::ptr_eq(current, ticket));
        if matches {
            self.cancel();
        }
        matches
    }
    pub fn ready(&mut self, setup: Option<&Snapshot>, busy: bool) -> bool {
        let Some((requested_after, _)) = self.requested_after.as_ref() else {
            return false;
        };
        let Some(setup) = setup else {
            self.cancel();
            return false;
        };
        // While a request is queued/in flight, the published snapshot can
        // still describe the previous attempt's terminal state.
        if setup.revision <= *requested_after || busy {
            return false;
        }
        if matches!(
            setup.phase,
            Phase::Cancelled | Phase::Cancelling | Phase::Failed | Phase::Missing
        ) {
            self.cancel();
            return false;
        }
        setup.phase == Phase::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(revision: u64, phase: Phase) -> Snapshot {
        Snapshot {
            revision,
            phase,
            received: 0,
            total: 10,
            failure: None,
        }
    }
    #[test]
    fn loss_of_the_setup_service_revokes_activation() {
        let mut activation = Activation::default();
        activation.request(4);
        assert!(!activation.ready(None, false));
        assert!(activation.ticket().is_none());
        assert!(!activation.ready(Some(&snapshot(6, Phase::Ready)), false));
    }
    #[test]
    fn an_old_ticket_cannot_claim_a_new_request() -> Result<(), &'static str> {
        let mut activation = Activation::default();
        activation.request(4);
        let old = activation.ticket().ok_or("requested ticket")?;
        activation.cancel();
        activation.request(4);
        let current = activation.ticket().ok_or("new ticket")?;
        assert!(!activation.claim(&old));
        assert!(activation.claim(&current));
        assert!(!activation.claim(&current));
        Ok(())
    }
    #[test]
    fn queued_retry_ignores_a_previous_terminal_snapshot() {
        let mut activation = Activation::default();
        activation.request(4);
        assert!(!activation.ready(Some(&snapshot(5, Phase::Failed)), true));
        assert!(!activation.ready(Some(&snapshot(6, Phase::Checking)), true));
        assert!(activation.ready(Some(&snapshot(7, Phase::Ready)), false));
    }
    #[test]
    fn only_the_requested_completion_can_enable_refinement() {
        let mut activation = Activation::default();
        assert!(!activation.ready(Some(&snapshot(4, Phase::Ready)), false));
        activation.request(4);
        assert!(!activation.ready(Some(&snapshot(4, Phase::Ready)), false));
        assert!(!activation.ready(Some(&snapshot(5, Phase::Checking)), false));
        assert!(!activation.ready(Some(&snapshot(6, Phase::Ready)), true));
        assert!(activation.ready(Some(&snapshot(6, Phase::Ready)), false));
        activation.cancel();
        assert!(!activation.ready(Some(&snapshot(6, Phase::Ready)), false));
    }
    #[test]
    fn cancellation_and_failure_do_not_enable_later_completions() {
        for terminal in [
            Phase::Cancelled,
            Phase::Cancelling,
            Phase::Failed,
            Phase::Missing,
        ] {
            let mut activation = Activation::default();
            activation.request(4);
            assert!(!activation.ready(Some(&snapshot(5, terminal)), false));
            assert!(!activation.ready(Some(&snapshot(6, Phase::Ready)), false));
        }
        let mut activation = Activation::default();
        activation.request(4);
        activation.cancel();
        assert!(!activation.ready(Some(&snapshot(6, Phase::Ready)), false));
    }
}
