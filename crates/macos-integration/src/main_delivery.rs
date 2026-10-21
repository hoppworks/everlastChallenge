//! AppKit ownership for the delivery stages. Only commands and scalar replies cross threads.
use crate::{
    clipboard::{ClipboardError, PreparedClipboard, RestoreOutcome, Snapshot},
    delivery::DeliveryToken,
    keyboard::{PasteKey, modifiers_held},
    paste::{PasteError, PreparedPaste, ReadyPaste, SentPaste},
    target::{
        ProcessIdentity, TargetApproval, TargetError, accessibility_trusted, frontmost_process,
        secure_input_active,
    },
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSPasteboard;
use std::{cell::RefCell, num::NonZeroU64, time::Instant};

pub enum Command {
    Environment,
    Prepare {
        session: NonZeroU64,
        name: String,
        backup: Snapshot,
        text: String,
        token: DeliveryToken,
    },
    Install {
        session: NonZeroU64,
        approval: TargetApproval,
    },
    Send {
        session: NonZeroU64,
        approval: TargetApproval,
    },
    Cleanup {
        session: NonZeroU64,
    },
}

pub struct Environment {
    pub process: ProcessIdentity,
    pub modifiers_held: bool,
    pub accessibility_trusted: bool,
    pub secure_input: bool,
    pub pasteboard_name: String,
}

pub enum Reply {
    Environment(Environment),
    Prepared,
    Installed,
    Sent,
    Cleaned(Option<RestoreOutcome>),
    /// The caller must wait off the main thread and retry; the native lease is retained.
    CleanupDeferred(Instant),
}

#[derive(Debug, thiserror::Error)]
pub enum MainError {
    #[error("Native delivery requires the application main thread")]
    WrongThread,
    #[error("Native delivery is already occupied")]
    Busy,
    #[error("Native delivery stage or session does not match")]
    Stale,
    #[error("Native delivery has been cancelled")]
    Cancelled,
    #[error(transparent)]
    Clipboard(#[from] ClipboardError),
    #[error(transparent)]
    Paste(#[from] PasteError),
    #[error(transparent)]
    Target(#[from] TargetError),
}

enum Stage {
    Prepared {
        events: PreparedPaste,
        clipboard: PreparedClipboard,
        token: DeliveryToken,
    },
    Installed(ReadyPaste),
    Sent(SentPaste),
}
struct Entry {
    session: NonZeroU64,
    stage: Stage,
}
thread_local! {
    static CURRENT: RefCell<Option<Entry>> = const { RefCell::new(None) };
}

fn after_read_window(
    restore_after: Option<Instant>,
    now: Instant,
    restore: impl FnOnce() -> Result<Reply, MainError>,
) -> Result<Reply, MainError> {
    if let Some(deadline) = restore_after
        && now < deadline
    {
        return Ok(Reply::CleanupDeferred(deadline));
    }
    restore()
}

/// Call from an application-main-thread closure. No AppKit owner leaves this function.
/// A stale cleanup cannot remove a newer session. There is never an implicit send or retry.
pub fn dispatch(command: Command) -> Result<Reply, MainError> {
    let main = MainThreadMarker::new().ok_or(MainError::WrongThread)?;
    if matches!(command, Command::Environment) {
        return Ok(Reply::Environment(Environment {
            process: frontmost_process(main)?,
            modifiers_held: modifiers_held(),
            accessibility_trusted: accessibility_trusted(),
            secure_input: secure_input_active(),
            pasteboard_name: NSPasteboard::generalPasteboard().name().to_string(),
        }));
    }
    CURRENT.with(|current| {
        let mut current = current.try_borrow_mut().map_err(|_| MainError::Busy)?;
        match command {
            Command::Environment => Err(MainError::Stale),
            Command::Prepare { session, name, backup, text, token } => {
                if current.is_some() {
                    return Err(MainError::Busy);
                }
                if token.session() != session || !token.is_pending() {
                    return Err(MainError::Cancelled);
                }
                let key = PasteKey::resolve(main).map_err(PasteError::from)?;
                let events = PreparedPaste::new(key, main)?;
                let clipboard = PreparedClipboard::new(&name, backup, &text, &token, main)?;
                if !token.is_pending() {
                    return Err(MainError::Cancelled);
                }
                *current = Some(Entry { session, stage: Stage::Prepared { events, clipboard, token } });
                Ok(Reply::Prepared)
            }
            Command::Install { session, approval } => {
                if !matches!(current.as_ref(), Some(Entry { session: id, stage: Stage::Prepared { .. } }) if *id == session) {
                    return Err(MainError::Stale);
                }
                let entry = current.take().ok_or(MainError::Stale)?;
                let Stage::Prepared { events, clipboard, token } = entry.stage else {
                    return Err(MainError::Stale);
                };
                let ready = events.install(clipboard, approval, &token, main)?;
                *current = Some(Entry { session, stage: Stage::Installed(ready) });
                Ok(Reply::Installed)
            }
            Command::Send { session, approval } => {
                if !matches!(current.as_ref(), Some(Entry { session: id, stage: Stage::Installed(_) }) if *id == session) {
                    return Err(MainError::Stale);
                }
                let entry = current.take().ok_or(MainError::Stale)?;
                let Stage::Installed(ready) = entry.stage else {
                    return Err(MainError::Stale);
                };
                let sent = ready.send(approval, main)?;
                *current = Some(Entry { session, stage: Stage::Sent(sent) });
                Ok(Reply::Sent)
            }
            Command::Cleanup { session } => {
                if current.as_ref().is_none_or(|entry| entry.session != session) {
                    return Ok(Reply::Cleaned(None));
                }
                let restore_after = match current.as_ref().map(|entry| &entry.stage) {
                    Some(Stage::Sent(sent)) => Some(sent.restore_after()),
                    _ => None,
                };
                after_read_window(restore_after, Instant::now(), || {
                    let entry = current.take().ok_or(MainError::Stale)?;
                    let restored = match entry.stage {
                        Stage::Prepared { .. } => None,
                        Stage::Installed(ready) => Some(ready.discard(main)?),
                        Stage::Sent(sent) => Some(sent.restore(main)?),
                    };
                    Ok(Reply::Cleaned(restored))
                })
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sent_read_window_defers_restoration_until_deadline_without_consuming_lease()
    -> Result<(), MainError> {
        let posted = Instant::now();
        let deadline = posted + std::time::Duration::from_millis(500);
        let consumed = std::cell::Cell::new(false);
        for now in [posted, deadline - std::time::Duration::from_nanos(1)] {
            let reply = after_read_window(Some(deadline), now, || {
                consumed.set(true);
                Ok(Reply::Cleaned(None))
            })?;
            assert!(matches!(reply, Reply::CleanupDeferred(value) if value == deadline));
            assert!(!consumed.get());
        }
        assert!(matches!(
            after_read_window(Some(deadline), deadline, || {
                consumed.set(true);
                Ok(Reply::Cleaned(None))
            })?,
            Reply::Cleaned(_)
        ));
        assert!(consumed.get());
        assert!(matches!(
            after_read_window(None, posted, || Ok(Reply::Cleaned(None)))?,
            Reply::Cleaned(_)
        ));
        Ok(())
    }

    #[test]
    fn background_dispatch_cannot_access_appkit_state() -> Result<(), Box<dyn std::error::Error>> {
        let result = std::thread::spawn(|| dispatch(Command::Environment).map(|_| ()))
            .join()
            .map_err(|_| "worker panicked")?;
        assert!(matches!(result, Err(MainError::WrongThread)));
        Ok(())
    }
}
