//! Error lifetime at the host acknowledgement boundary, independent of UI transport.
use std::sync::Arc;

#[derive(Default)]
pub(super) struct ActionErrors {
    generation: Arc<()>,
    transient: Option<String>,
    persistent: Option<String>,
}

impl ActionErrors {
    // Identity avoids counter wraparound. At most the bounded pending requests
    // retain old identities; these allocations never occur in the audio callback.
    pub(super) fn begin(&mut self) -> Arc<()> {
        self.generation = Arc::new(());
        Arc::clone(&self.generation)
    }

    pub(super) fn complete(&mut self, ticket: &Arc<()>, result: Result<(), String>) {
        if Arc::ptr_eq(ticket, &self.generation) {
            self.transient = result.err();
        }
    }

    pub(super) fn report(&mut self, message: String) {
        let ticket = self.begin();
        self.complete(&ticket, Err(message));
    }

    pub(super) fn clear_transient(&mut self) {
        let ticket = self.begin();
        self.complete(&ticket, Ok(()));
    }

    pub(super) fn persistent(&mut self, message: Option<String>) {
        self.persistent = message;
    }

    pub(super) fn message(&self) -> Option<String> {
        self.persistent.clone().or_else(|| self.transient.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_success_removes_previous_failure() {
        let mut errors = ActionErrors::default();
        let before_ready = errors.begin();
        errors.complete(&before_ready, Err("not ready".into()));
        assert_eq!(errors.message().as_deref(), Some("not ready"));
        let ready = errors.begin();
        errors.complete(&ready, Ok(()));
        assert_eq!(errors.message(), None);
    }

    #[test]
    fn late_success_and_failure_cannot_overwrite_newer_result() {
        let mut errors = ActionErrors::default();
        let old = errors.begin();
        let current = errors.begin();
        errors.complete(&current, Err("new failure".into()));
        errors.complete(&old, Ok(()));
        errors.complete(&old, Err("old failure".into()));
        assert_eq!(errors.message().as_deref(), Some("new failure"));
        errors.report("independent failure".into());
        errors.complete(&current, Ok(()));
        assert_eq!(errors.message().as_deref(), Some("independent failure"));
    }

    #[test]
    fn successful_actions_never_clear_a_persistent_registration_fault() {
        let mut errors = ActionErrors::default();
        errors.persistent(Some("registration unavailable".into()));
        let start = errors.begin();
        errors.complete(&start, Ok(()));
        errors.clear_transient();
        assert_eq!(
            errors.message().as_deref(),
            Some("registration unavailable")
        );
        errors.persistent(None);
        assert_eq!(errors.message(), None);
    }
}
