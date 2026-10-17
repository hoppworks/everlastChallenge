//! Main-thread half of native delivery. Each stage requires a fresh AX receipt.
use crate::{
    clipboard::{ClipboardError, ClipboardLease, PreparedClipboard, RestoreOutcome},
    delivery::{CommitError, DeliveryToken},
    keyboard::{KeyboardError, PasteKey, modifiers_held},
    target::{
        TargetApproval, TargetError, accessibility_trusted, frontmost_process, secure_input_active,
    },
};
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use objc2::MainThreadMarker;

#[derive(Debug, thiserror::Error)]
pub enum PasteError {
    #[error(transparent)]
    Clipboard(#[from] ClipboardError),
    #[error(transparent)]
    Keyboard(#[from] KeyboardError),
    #[error(transparent)]
    Target(#[from] TargetError),
    #[error(transparent)]
    Commit(#[from] CommitError),
    #[error("Physical modifier keys are still held")]
    ModifiersHeld,
    #[error("Keyboard layout changed")]
    LayoutChanged,
    #[error("Keyboard events could not be constructed")]
    EventConstruction,
    #[error("{cause}; clipboard cleanup also failed: {cleanup}")]
    Cleanup {
        cause: Box<PasteError>,
        cleanup: ClipboardError,
    },
}

pub struct PreparedPaste {
    key: PasteKey,
    down: CGEvent,
    up: CGEvent,
}
pub struct ReadyPaste {
    events: PreparedPaste,
    clipboard: ClipboardLease,
    token: DeliveryToken,
}

/// This confirms only event dispatch. It contains no claim about target text.
pub struct SentPaste {
    clipboard: ClipboardLease,
}

impl PreparedPaste {
    /// Creates both events before any clipboard mutation, without posting them.
    pub fn new(key: PasteKey, _main_thread: MainThreadMarker) -> Result<Self, PasteError> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| PasteError::EventConstruction)?;
        let down = CGEvent::new_keyboard_event(source.clone(), key.key_code(), true)
            .map_err(|_| PasteError::EventConstruction)?;
        let up = CGEvent::new_keyboard_event(source, key.key_code(), false)
            .map_err(|_| PasteError::EventConstruction)?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagCommand);
        Ok(Self { key, down, up })
    }

    fn preflight(
        &self,
        approval: TargetApproval,
        token: &DeliveryToken,
        main: MainThreadMarker,
    ) -> Result<(), PasteError> {
        if !token.is_pending() {
            return Err(CommitError::Cancelled.into());
        }
        if !self.key.is_current(main)? {
            return Err(PasteError::LayoutChanged);
        }
        if modifiers_held() {
            return Err(PasteError::ModifiersHeld);
        }
        if !accessibility_trusted() {
            return Err(TargetError::Permission.into());
        }
        if secure_input_active() {
            return Err(TargetError::SecureInput.into());
        }
        approval.validate(token.session(), frontmost_process(main)?)?;
        Ok(())
    }

    pub fn install(
        self,
        clipboard: PreparedClipboard,
        approval: TargetApproval,
        token: &DeliveryToken,
        main: MainThreadMarker,
    ) -> Result<ReadyPaste, PasteError> {
        if !clipboard.belongs_to(token) {
            return Err(CommitError::Cancelled.into());
        }
        self.preflight(approval, token, main)?;
        let clipboard = clipboard.install(main)?;
        Ok(ReadyPaste {
            events: self,
            clipboard,
            token: token.clone(),
        })
    }
}

impl ReadyPaste {
    /// Explicitly undo an installed but unsent clipboard lease on cancellation.
    pub fn discard(self, main: MainThreadMarker) -> Result<RestoreOutcome, ClipboardError> {
        self.clipboard.restore(main)
    }

    /// Call only with a new AX receipt acquired after clipboard installation.
    /// No fallback event or retry exists after the token's one-way commit.
    pub fn send(
        self,
        approval: TargetApproval,
        main: MainThreadMarker,
    ) -> Result<SentPaste, PasteError> {
        let Self {
            events,
            clipboard,
            token,
        } = self;
        let ready = (|| {
            events.preflight(approval, &token, main)?;
            if !clipboard.is_current() {
                return Err(ClipboardError::Changed.into());
            }
            token.try_commit()?;
            Ok::<_, PasteError>(())
        })();
        if let Err(cause) = ready {
            return match clipboard.restore(main) {
                Ok(_) => Err(cause),
                Err(cleanup) => Err(PasteError::Cleanup {
                    cause: Box::new(cause),
                    cleanup,
                }),
            };
        }
        events.down.post(CGEventTapLocation::HID);
        events.up.post(CGEventTapLocation::HID);
        Ok(SentPaste { clipboard })
    }
}

impl SentPaste {
    /// Schedule outside the send step; neither delay nor restore proves receipt.
    pub fn restore(self, main: MainThreadMarker) -> Result<RestoreOutcome, ClipboardError> {
        self.clipboard.restore(main)
    }
}
