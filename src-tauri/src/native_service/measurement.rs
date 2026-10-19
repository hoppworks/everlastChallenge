//! Bounded, content-free observations of the actual native handoff boundaries.
//! Only compiled for explicit measurement builds; observations never authorize work.
use super::*;
use serde_json::{Value, json};
use std::sync::OnceLock;

const MAX_RECORDS: usize = 32;

#[derive(Clone, Copy)]
enum Stage {
    Send,
    Cleanup,
}
struct Span {
    lower: u64,
    upper: u64,
    success: bool,
    committed: bool,
    restore: Option<&'static str>,
}
struct Record {
    session: u64,
    token: DeliveryToken,
    ready: Instant,
    matched: bool,
    send: Option<Span>,
    cleanup: Option<Span>,
}
struct Ticket {
    session: u64,
    token: DeliveryToken,
    stage: Stage,
}
#[derive(Default)]
struct Ledger {
    records: Vec<Record>,
    invalid: bool,
    overflow: bool,
}
impl Ledger {
    fn text_ready(&mut self, session: u64, token: &DeliveryToken, at: Instant) {
        if session != token.session().get()
            || !token.is_pending()
            || self.records.iter().any(|r| r.session == session)
        {
            self.invalid = true;
            return;
        }
        if self.records.len() == MAX_RECORDS {
            self.overflow = true;
            self.invalid = true;
            return;
        }
        self.records.push(Record {
            session,
            token: token.clone(),
            ready: at,
            matched: false,
            send: None,
            cleanup: None,
        });
    }
    fn validate_delivery(&mut self, session: NonZeroU64, token: &DeliveryToken) {
        if let Some(r) = self.records.iter_mut().find(|r| r.session == session.get()) {
            if !r.token.same_work(token) || r.matched || !token.is_pending() {
                self.invalid = true;
            } else {
                r.matched = true;
            }
        }
    }
    fn ticket(
        &mut self,
        stage: Stage,
        session: NonZeroU64,
        token: Option<&DeliveryToken>,
    ) -> Option<Ticket> {
        let r = self.records.iter().find(|r| r.session == session.get())?;
        if !r.matched {
            return None;
        }
        if matches!(stage, Stage::Send)
            && !token.is_some_and(|t| r.token.same_work(t) && t.is_pending())
        {
            return None;
        }
        Some(Ticket {
            session: r.session,
            token: r.token.clone(),
            stage,
        })
    }
    fn complete(
        &mut self,
        ticket: Ticket,
        before: Instant,
        after: Instant,
        success: bool,
        restore: Option<&'static str>,
    ) {
        let Some(r) = self
            .records
            .iter_mut()
            .find(|r| r.session == ticket.session && r.token.same_work(&ticket.token))
        else {
            self.invalid = true;
            return;
        };
        let Some(lower) = before
            .checked_duration_since(r.ready)
            .and_then(|d| u64::try_from(d.as_nanos()).ok())
        else {
            self.invalid = true;
            return;
        };
        let Some(upper) = after
            .checked_duration_since(r.ready)
            .and_then(|d| u64::try_from(d.as_nanos()).ok())
        else {
            self.invalid = true;
            return;
        };
        if upper < lower
            || (matches!(ticket.stage, Stage::Send) && success && !ticket.token.is_committed())
        {
            self.invalid = true;
            return;
        }
        if matches!(ticket.stage, Stage::Cleanup)
            && r.send.as_ref().is_some_and(|s| lower < s.upper)
        {
            self.invalid = true;
            return;
        }
        let target = match ticket.stage {
            Stage::Send => &mut r.send,
            Stage::Cleanup => &mut r.cleanup,
        };
        if target.is_some() {
            self.invalid = true;
            return;
        }
        *target = Some(Span {
            lower,
            upper,
            success,
            committed: ticket.token.is_committed(),
            restore,
        });
    }
    fn snapshot(&self) -> Value {
        json!({"schemaVersion":1,"kind":"native-handoff-timing-v1","maximumRecords":MAX_RECORDS,
          "qualification":false,"exactEventPostInstant":false,"invalid":self.invalid,"overflow":self.overflow,
          "records":self.records.iter().map(|r| json!({"session":r.session,"textReadyObserved":true,"deliveryTokenMatched":r.matched,
            "sendLowerNanos":r.send.as_ref().map(|s|s.lower),"sendUpperNanos":r.send.as_ref().map(|s|s.upper),
            "sendSucceeded":r.send.as_ref().map(|s|s.success),"sendCommitted":r.send.as_ref().map(|s|s.committed),
            "clipboardCompletionLowerNanos":r.cleanup.as_ref().map(|s|s.lower),"clipboardCompletionUpperNanos":r.cleanup.as_ref().map(|s|s.upper),
            "cleanupSucceeded":r.cleanup.as_ref().map(|s|s.success),"restoreOutcome":r.cleanup.as_ref().and_then(|s|s.restore)
          })).collect::<Vec<_>>()})
    }
}
static LEDGER: OnceLock<Mutex<Ledger>> = OnceLock::new();
static LOST: AtomicBool = AtomicBool::new(false);
fn with_ledger<T>(action: impl FnOnce(&mut Ledger) -> T) -> Option<T> {
    match LEDGER
        .get_or_init(|| Mutex::new(Ledger::default()))
        .try_lock()
    {
        Ok(mut ledger) => Some(action(&mut ledger)),
        Err(_) => {
            LOST.store(true, Ordering::Release);
            None
        }
    }
}
/// Actual controller adoption boundary; no transcript content crosses this API.
pub(crate) fn text_ready(session: u64, token: &DeliveryToken, at: Instant) {
    with_ledger(|l| l.text_ready(session, token, at));
}
pub(super) fn validate_delivery(session: NonZeroU64, token: &DeliveryToken) {
    with_ledger(|l| l.validate_delivery(session, token));
}
/// Brackets the real main-thread dispatch call, not its asynchronous caller's reply.
pub(super) fn dispatch(
    command: Command,
    token: Option<&DeliveryToken>,
    operation: impl FnOnce(Command) -> Result<Reply, NativeError>,
) -> Result<Reply, NativeError> {
    let ticket = match &command {
        Command::Send { session, .. } => {
            with_ledger(|l| l.ticket(Stage::Send, *session, token)).flatten()
        }
        Command::Cleanup { session } => {
            with_ledger(|l| l.ticket(Stage::Cleanup, *session, token)).flatten()
        }
        _ => None,
    };
    let before = Instant::now();
    let result = operation(command);
    let after = Instant::now();
    if let Some(ticket) = ticket {
        let (success, restore) = match (&ticket.stage, &result) {
            (Stage::Send, Ok(Reply::Sent)) => (true, None),
            (Stage::Cleanup, Ok(Reply::Cleaned(outcome))) => (
                true,
                Some(match outcome {
                    Some(macos_integration::clipboard::RestoreOutcome::Restored) => "restored",
                    Some(macos_integration::clipboard::RestoreOutcome::OwnershipChanged) => {
                        "ownership-changed"
                    }
                    None => "no-lease",
                }),
            ),
            _ => (false, None),
        };
        with_ledger(|l| l.complete(ticket, before, after, success, restore));
    }
    result
}
/// Bounded readonly projection. Absence, invalidity and unknown completion remain explicit.
pub fn snapshot() -> Value {
    let mut result = with_ledger(|l| l.snapshot()).unwrap_or_else(
        || json!({"schemaVersion":1,"kind":"native-handoff-timing-v1","invalid":true,"records":[]}),
    );
    result["observerContentionOrPoison"] = json!(LOST.load(Ordering::Acquire));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> NonZeroU64 {
        NonZeroU64::MIN
    }

    #[test]
    fn exact_work_records_separate_dispatch_and_restore_bounds()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(id());
        let ready = Instant::now();
        let mut ledger = Ledger::default();
        ledger.text_ready(id().get(), &token, ready);
        ledger.validate_delivery(id(), &token);
        let ticket = ledger
            .ticket(Stage::Send, id(), Some(&token))
            .ok_or("bound send")?;
        token.try_commit()?;
        ledger.complete(
            ticket,
            ready + Duration::from_millis(2),
            ready + Duration::from_millis(3),
            true,
            None,
        );
        let ticket = ledger
            .ticket(Stage::Cleanup, id(), None)
            .ok_or("bound cleanup")?;
        ledger.complete(
            ticket,
            ready + Duration::from_millis(503),
            ready + Duration::from_millis(505),
            true,
            Some("restored"),
        );
        let report = ledger.snapshot();
        assert_eq!(report["records"][0]["sendLowerNanos"], 2_000_000);
        assert_eq!(report["records"][0]["sendUpperNanos"], 3_000_000);
        assert_eq!(
            report["records"][0]["clipboardCompletionLowerNanos"],
            503_000_000_u64
        );
        assert_eq!(
            report["records"][0]["clipboardCompletionUpperNanos"],
            505_000_000_u64
        );
        assert_eq!(report["records"][0]["sendCommitted"], true);
        assert_eq!(report["records"][0]["cleanupSucceeded"], true);
        assert_eq!(report["records"][0]["restoreOutcome"], "restored");
        assert_eq!(report["invalid"], false);
        Ok(())
    }

    #[test]
    fn foreign_token_or_missing_ready_never_yields_dispatch_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(id());
        let mut ledger = Ledger::default();
        assert!(ledger.ticket(Stage::Send, id(), Some(&token)).is_none());
        ledger.text_ready(id().get(), &token, Instant::now());
        ledger.validate_delivery(id(), &DeliveryToken::new(id()));
        assert!(ledger.ticket(Stage::Send, id(), Some(&token)).is_none());
        assert_eq!(ledger.snapshot()["invalid"], true);
        assert!(ledger.snapshot()["records"][0]["sendSucceeded"].is_null());
        Ok(())
    }

    #[test]
    fn cancelled_before_send_and_failed_cleanup_cannot_look_successful()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(id());
        let ready = Instant::now();
        let mut ledger = Ledger::default();
        ledger.text_ready(id().get(), &token, ready);
        ledger.validate_delivery(id(), &token);
        token.cancel();
        assert!(ledger.ticket(Stage::Send, id(), Some(&token)).is_none());
        let ticket = ledger
            .ticket(Stage::Cleanup, id(), None)
            .ok_or("cleanup still observed")?;
        ledger.complete(ticket, ready, ready, false, None);
        let report = ledger.snapshot();
        assert!(report["records"][0]["sendSucceeded"].is_null());
        assert_eq!(report["records"][0]["cleanupSucceeded"], false);
        assert!(report["records"][0]["restoreOutcome"].is_null());
        Ok(())
    }

    #[test]
    fn duplicate_and_backwards_observations_are_invalid_not_overwritten()
    -> Result<(), Box<dyn std::error::Error>> {
        let token = DeliveryToken::new(id());
        let ready = Instant::now();
        let mut ledger = Ledger::default();
        ledger.text_ready(id().get(), &token, ready);
        ledger.validate_delivery(id(), &token);
        let ticket = ledger
            .ticket(Stage::Send, id(), Some(&token))
            .ok_or("send")?;
        token.try_commit()?;
        ledger.complete(
            ticket,
            ready + Duration::from_millis(2),
            ready + Duration::from_millis(1),
            true,
            None,
        );
        assert_eq!(ledger.snapshot()["invalid"], true);
        assert!(ledger.snapshot()["records"][0]["sendUpperNanos"].is_null());
        ledger.text_ready(id().get(), &token, ready);
        assert_eq!(ledger.records.len(), 1);
        Ok(())
    }
    #[test]
    fn bounded_records_never_evict_or_accept_duplicate_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut ledger = Ledger::default();
        let at = Instant::now();
        for session in 1..=MAX_RECORDS as u64 + 1 {
            let token = DeliveryToken::new(NonZeroU64::new(session).ok_or("id")?);
            ledger.text_ready(session, &token, at);
        }
        assert_eq!(ledger.records.len(), MAX_RECORDS);
        assert!(ledger.overflow && ledger.invalid);
        assert_eq!(ledger.records[0].session, 1);
        Ok(())
    }

    #[test]
    fn dispatch_wrapper_calls_once_and_returns_the_original_failure_without_content_export()
    -> Result<(), Box<dyn std::error::Error>> {
        let session = NonZeroU64::new(90001).ok_or("id")?;
        let token = DeliveryToken::new(session);
        text_ready(session.get(), &token, Instant::now());
        validate_delivery(session, &token);
        let mut calls = 0;
        let result = dispatch(Command::Cleanup { session }, None, |command| {
            assert!(matches!(command, Command::Cleanup { session: id } if id == session));
            calls += 1;
            Err("private test error must not be exported".into())
        });
        assert_eq!(calls, 1);
        assert!(
            matches!(result, Err(ref message) if message == "private test error must not be exported")
        );
        let report = snapshot();
        let record = report["records"]
            .as_array()
            .ok_or("records")?
            .iter()
            .find(|r| r["session"] == session.get())
            .ok_or("record")?;
        assert_eq!(record["cleanupSucceeded"], false);
        assert!(record["sendSucceeded"].is_null());
        assert!(!report.to_string().contains("private test error"));
        assert_eq!(report["qualification"], false);
        assert_eq!(report["exactEventPostInstant"], false);
        Ok(())
    }
}
