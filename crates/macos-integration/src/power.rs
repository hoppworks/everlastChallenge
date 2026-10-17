//! Main-thread-owned subscriptions to public workspace power notifications.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    WillSleep,
    DidWake,
    WillPowerOff,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PowerError {
    #[error("Power observers must be owned by the native main thread")]
    MainThread,
}

use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillPowerOffNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
use std::{
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct DeliveryGate {
    active: AtomicBool,
    receive: Box<dyn Fn(PowerEvent) + Send + Sync>,
}

impl DeliveryGate {
    fn new(receive: impl Fn(PowerEvent) + Send + Sync + 'static) -> Self {
        Self {
            active: AtomicBool::new(true),
            receive: Box::new(receive),
        }
    }

    fn emit(&self, event: PowerEvent) {
        if self.active.load(Ordering::Acquire)
            && catch_unwind(AssertUnwindSafe(|| (self.receive)(event))).is_err()
        {
            self.disable();
            eprintln!("Power event callback failed; observer disabled");
        }
    }

    fn disable(&self) {
        self.active.store(false, Ordering::Release);
    }
}

/// Own this value on the main thread until the native application run loop ends.
/// Callbacks run on the main operation queue and must only enqueue bounded work;
/// do not join workers or perform blocking I/O inside them.
///
/// ```compile_fail
/// use macos_integration::power::PowerObserver;
/// fn requires_send<T: Send>() {}
/// requires_send::<PowerObserver>();
/// ```
///
/// ```compile_fail
/// use macos_integration::power::PowerObserver;
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<PowerObserver>();
/// ```
pub struct PowerObserver {
    center: Retained<NSNotificationCenter>,
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    gate: Arc<DeliveryGate>,
    _main_thread: PhantomData<Rc<()>>,
}

impl PowerObserver {
    pub fn new(receive: impl Fn(PowerEvent) + Send + Sync + 'static) -> Result<Self, PowerError> {
        let _main = MainThreadMarker::new().ok_or(PowerError::MainThread)?;
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let queue = NSOperationQueue::mainQueue();
        let gate = Arc::new(DeliveryGate::new(receive));
        let mut observers = Vec::with_capacity(3);
        // SAFETY: these are Apple's immutable, framework-owned notification
        // constants, not invented notification names or workspace mutations.
        let events = unsafe {
            [
                (NSWorkspaceWillSleepNotification, PowerEvent::WillSleep),
                (NSWorkspaceDidWakeNotification, PowerEvent::DidWake),
                (
                    NSWorkspaceWillPowerOffNotification,
                    PowerEvent::WillPowerOff,
                ),
            ]
        };
        for (name, event) in events {
            let callback_gate = gate.clone();
            let callback = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                callback_gate.emit(event);
            });
            // SAFETY: nil sender intentionally receives workspace-wide events.
            // Apple's main queue serializes delivery with this main-thread-only
            // owner's removal. The copied block owns only Send + Sync Rust data,
            // never dereferences the notification, and catches callback panics.
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&queue),
                    &callback,
                )
            };
            observers.push(observer);
        }
        Ok(Self {
            center,
            observers,
            gate,
            _main_thread: PhantomData,
        })
    }

    /// Idempotent. Close admission before unregistering all three tokens, so
    /// already-queued native callbacks become inert after removal.
    pub fn remove(&mut self) {
        self.gate.disable();
        for observer in self.observers.drain(..) {
            let observer: &ProtocolObject<dyn NSObjectProtocol> = &observer;
            // SAFETY: the exact opaque token returned by registration is passed
            // back to the same notification center; its retain spans the call.
            unsafe {
                self.center.removeObserver(observer.as_ref());
            }
        }
    }
}

impl Drop for PowerObserver {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribing_from_a_worker_is_rejected_before_observing_the_workspace()
    -> Result<(), Box<dyn std::error::Error>> {
        let error = std::thread::spawn(|| PowerObserver::new(|_| {}).err())
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(error, Some(PowerError::MainThread));
        Ok(())
    }

    #[test]
    fn disabling_delivery_rejects_already_queued_and_repeated_power_events()
    -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(3);
        let gate = DeliveryGate::new(move |event| assert!(sender.try_send(event).is_ok()));
        gate.emit(PowerEvent::WillSleep);
        gate.emit(PowerEvent::DidWake);
        assert_eq!(receiver.try_recv()?, PowerEvent::WillSleep);
        assert_eq!(receiver.try_recv()?, PowerEvent::DidWake);
        gate.disable();
        gate.disable();
        gate.emit(PowerEvent::WillPowerOff);
        gate.emit(PowerEvent::DidWake);
        assert!(matches!(
            receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        Ok(())
    }

    #[test]
    fn queued_callback_copies_share_the_same_removal_gate() -> Result<(), Box<dyn std::error::Error>>
    {
        let (sender, receiver) = std::sync::mpsc::sync_channel(3);
        let gate = Arc::new(DeliveryGate::new(move |event| {
            assert!(sender.try_send(event).is_ok())
        }));
        let queued = gate.clone();
        queued.emit(PowerEvent::WillPowerOff);
        assert_eq!(receiver.try_recv()?, PowerEvent::WillPowerOff);
        gate.disable();
        queued.emit(PowerEvent::WillSleep);
        assert!(matches!(
            receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        Ok(())
    }

    #[test]
    fn a_panicking_client_is_disabled_without_unwinding_into_objc() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let captured = calls.clone();
        let gate = DeliveryGate::new(move |_| {
            captured.fetch_add(1, Ordering::Relaxed);
            std::panic::resume_unwind(Box::new("controlled client failure"));
        });
        gate.emit(PowerEvent::WillSleep);
        gate.emit(PowerEvent::DidWake);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
