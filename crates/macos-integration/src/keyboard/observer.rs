//! Main-thread-owned public TIS distributed notification subscription.
use super::*;
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send, rc::Retained, sel};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationSuspensionBehavior, NSObject,
    NSString,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    static kTISNotifySelectedKeyboardInputSourceChanged: CFStringRef;
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ObserverError {
    #[error("Keyboard layout observers must be owned by the main thread")]
    MainThread,
}
struct Gate {
    active: Cell<bool>,
    callback: Box<dyn Fn()>,
}
impl Gate {
    fn emit(&self) {
        if self.active.get() && catch_unwind(AssertUnwindSafe(|| (self.callback)())).is_err() {
            self.active.set(false);
            eprintln!("Keyboard layout callback failed; observer disabled");
        }
    }
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<Gate>]
    struct LayoutTarget;
    impl LayoutTarget {
        #[unsafe(method(layoutChanged:))]
        fn changed(&self, _notification: &NSNotification) {
            let _keep_alive = self.retain();
            self.ivars().emit();
        }
    }
);
impl LayoutTarget {
    fn new(main: MainThreadMarker, gate: Rc<Gate>) -> Retained<Self> {
        let object = Self::alloc(main).set_ivars(gate);
        // SAFETY: initialize a fresh NSObject subclass with initialized Rust ivars.
        unsafe { msg_send![super(object), init] }
    }
}
/// Own on main until explicit removal before the application event loop exits.
/// Notifications are invalidation hints: re-read the current layout, never trust
/// payloads. Delivery remains enabled while inactive; foreground sync is additional.
/// Callbacks must be bounded and must not join workers or perform blocking I/O.
/// ```compile_fail
/// fn send<T: Send>() {}
/// send::<macos_integration::keyboard::LayoutObserver>();
/// ```
/// ```compile_fail
/// fn sync<T: Sync>() {}
/// sync::<macos_integration::keyboard::LayoutObserver>();
/// ```
pub struct LayoutObserver {
    center: Retained<NSDistributedNotificationCenter>,
    target: Retained<LayoutTarget>,
    name: Retained<NSString>,
    gate: Rc<Gate>,
    removed: bool,
}
impl LayoutObserver {
    pub fn new(callback: impl Fn() + 'static) -> Result<Self, ObserverError> {
        let main = MainThreadMarker::new().ok_or(ObserverError::MainThread)?;
        let gate = Rc::new(Gate {
            active: Cell::new(true),
            callback: Box::new(callback),
        });
        let target = LayoutTarget::new(main, Rc::clone(&gate));
        let center = NSDistributedNotificationCenter::defaultCenter();
        // SAFETY: framework-owned immutable CFString constant; Get retain gives
        // a valid owned reference while converting the notification name.
        let name =
            unsafe { CFString::wrap_under_get_rule(kTISNotifySelectedKeyboardInputSourceChanged) };
        let name = NSString::from_str(&name.to_string());
        // SAFETY: retained receiver implements this exact notification selector;
        // distributed delivery uses the main run loop. No notification posting,
        // input-source selection, permissions, or global suspension changes.
        // SDK NSDistributedNotificationCenter.h: DeliverImmediately applies only
        // to this registration and restores the prior suspension state.
        unsafe {
            center.addObserver_selector_name_object_suspensionBehavior(
                &target,
                sel!(layoutChanged:),
                Some(&name),
                None,
                NSNotificationSuspensionBehavior::DeliverImmediately,
            );
        }
        Ok(Self {
            center,
            target,
            name,
            gate,
            removed: false,
        })
    }
    /// Close admission before removal. Repeated removal and queued callbacks are inert.
    pub fn remove(&mut self) {
        self.gate.active.set(false);
        if !self.removed {
            // SAFETY: exact still-retained observer/name registered on this center;
            // unregister before releasing target, as required by Foundation.
            unsafe {
                self.center
                    .removeObserver_name_object(&self.target, Some(&self.name), None);
            }
            self.removed = true;
        }
    }
}
impl Drop for LayoutObserver {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_callback_after_removal_is_inert_and_panic_disables_delivery() {
        let count = Rc::new(Cell::new(0));
        let calls = Rc::clone(&count);
        let gate = Rc::new(Gate {
            active: Cell::new(true),
            callback: Box::new(move || calls.set(calls.get() + 1)),
        });
        let queued = Rc::clone(&gate);
        gate.emit();
        gate.active.set(false);
        queued.emit();
        queued.emit();
        assert_eq!(count.get(), 1);
        let calls = Rc::clone(&count);
        let panic_gate = Gate {
            active: Cell::new(true),
            callback: Box::new(move || {
                calls.set(calls.get() + 1);
                std::panic::resume_unwind(Box::new("controlled callback failure"));
            }),
        };
        panic_gate.emit();
        panic_gate.emit();
        assert_eq!(count.get(), 2);
    }
    #[test]
    fn worker_registration_is_rejected_before_native_registration() -> Result<(), String> {
        let result = std::thread::spawn(|| LayoutObserver::new(|| {}).err())
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(result, Some(ObserverError::MainThread));
        Ok(())
    }
}
