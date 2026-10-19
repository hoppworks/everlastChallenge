//! Route AppKit termination requests through the application's orderly exit.
//! No delegate replacement or method swizzling is performed.

use block2::RcBlock;
use objc2::{
    MainThreadMarker, class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Imp, Sel},
    sel,
};
use objc2_foundation::NSOperationQueue;
use std::{
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TerminationError {
    #[error("Termination routing must be installed on the native main thread")]
    MainThread,
    #[error("The native application delegate is unavailable")]
    MissingDelegate,
    #[error("A termination router is already installed")]
    AlreadyInstalled,
    #[error("The native delegate already implements applicationShouldTerminate:")]
    ExistingImplementation,
    #[error("The public Objective-C runtime could not add termination routing")]
    Registration,
}

struct Gate {
    active: AtomicBool,
    requested: AtomicBool,
    callback: Box<dyn Fn() + Send + Sync>,
}
impl Gate {
    fn claim(&self) -> bool {
        self.active.load(Ordering::Acquire) && !self.requested.swap(true, Ordering::AcqRel)
    }
    fn deliver(&self) {
        if self.active.load(Ordering::Acquire)
            && catch_unwind(AssertUnwindSafe(|| (self.callback)())).is_err()
        {
            // Keep cancellation in force rather than allow AppKit's unsafe
            // process-exit path after a failed application callback.
            eprintln!("Orderly termination callback failed");
        }
    }
}
#[derive(Clone)]
struct Registration {
    delegate: usize,
    gate: Arc<Gate>,
}
static REGISTRATION: Mutex<Option<Registration>> = Mutex::new(None);

/// Main-thread owner, retained until the application's orderly Exit event.
/// Install after Tauri builds its delegate, before entering its run loop. The
/// callback must enqueue exit (e.g. AppHandle::exit), never join workers here.
///
/// Only a missing selector is added. An existing foreign implementation is an
/// error. The added method remains on that Objective-C class for process life;
/// without this exact active delegate registration it returns TerminateNow,
/// AppKit's default. Hold the owner through worker cleanup. This does not qualify
/// system logout/power-off behavior or prevent forced process termination.
pub struct TerminationHook {
    _delegate: Retained<AnyObject>,
    gate: Arc<Gate>,
    _main_thread: PhantomData<Rc<()>>,
}
impl TerminationHook {
    pub fn install(callback: impl Fn() + Send + Sync + 'static) -> Result<Self, TerminationError> {
        let _main = MainThreadMarker::new().ok_or(TerminationError::MainThread)?;
        // SAFETY: public NSApplication singleton/delegate getters, on the main
        // thread. Retained keeps the existing framework-owned delegate alive;
        // this code never sets or replaces it.
        let delegate: Option<Retained<AnyObject>> = unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            msg_send![&*app, delegate]
        };
        let delegate = delegate.ok_or(TerminationError::MissingDelegate)?;
        let mut registration = REGISTRATION
            .lock()
            .map_err(|_| TerminationError::Registration)?;
        if registration.is_some() {
            return Err(TerminationError::AlreadyInstalled);
        }
        let class = delegate.class();
        let selector = sel!(applicationShouldTerminate:);
        let implementation = implementation();
        if let Some(method) = class.instance_method(selector) {
            if !std::ptr::fn_addr_eq(method.implementation(), implementation) {
                return Err(TerminationError::ExistingImplementation);
            }
        } else {
            // SAFETY: only add an absent instance selector to the actual delegate
            // class. No existing/inherited method is replaced. The ABI is Apple's
            // NSUInteger applicationShouldTerminate:(NSApplication *), Q@:@ on
            // supported 64-bit macOS; the callback uses usize and object refs.
            let added = unsafe {
                objc2::ffi::class_addMethod(
                    class as *const _ as *mut _,
                    selector,
                    implementation,
                    c"Q@:@".as_ptr(),
                )
            };
            if !added.as_bool() {
                return Err(TerminationError::Registration);
            }
        }
        let gate = Arc::new(Gate {
            active: AtomicBool::new(true),
            requested: AtomicBool::new(false),
            callback: Box::new(callback),
        });
        *registration = Some(Registration {
            delegate: (&*delegate as *const AnyObject) as usize,
            gate: Arc::clone(&gate),
        });
        Ok(Self {
            _delegate: delegate,
            gate,
            _main_thread: PhantomData,
        })
    }
}
impl Drop for TerminationHook {
    fn drop(&mut self) {
        self.gate.active.store(false, Ordering::Release);
        if let Ok(mut registration) = REGISTRATION.lock()
            && registration
                .as_ref()
                .is_some_and(|value| Arc::ptr_eq(&value.gate, &self.gate))
        {
            *registration = None;
        }
    }
}
fn implementation() -> Imp {
    // SAFETY: Objective-C stores IMP as an erased function pointer. The only
    // selector using this IMP has the exact Q@:@ signature declared above.
    unsafe {
        std::mem::transmute(
            should_terminate as unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> usize,
        )
    }
}
unsafe extern "C-unwind" fn should_terminate(this: &AnyObject, _: Sel, _: &AnyObject) -> usize {
    let gate = match REGISTRATION.lock() {
        Ok(registration) => registration
            .as_ref()
            .filter(|value| value.delegate == (this as *const AnyObject) as usize)
            .map(|value| Arc::clone(&value.gate)),
        Err(_) => return 0,
    };
    let Some(gate) = gate else {
        return 1;
    };
    if gate.claim() {
        let block = RcBlock::new(move || gate.deliver());
        // SAFETY: this block owns its Arc data, catches Rust callback unwinding,
        // and is copied by the queue. Main-queue execution defers application
        // exit until after the synchronous AppKit termination decision returns.
        unsafe {
            NSOperationQueue::mainQueue().addOperationWithBlock(&block);
        }
    }
    0 // NSTerminateCancel: only the queued orderly application exit may proceed.
}
