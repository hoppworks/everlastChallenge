//! Public AppKit termination contract in disposable children; no window, audio, or model.
#![allow(
    unsafe_code,
    reason = "Owns test delegates and invokes the public termination decision without terminating"
)]
use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use macos_integration::termination::{TerminationError, TerminationHook};
use objc2::{
    class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ClassBuilder, Sel},
    sel,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
extern "C-unwind" fn preserved(_: &AnyObject, _: Sel) -> usize {
    42
}
fn run(control: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = ClassBuilder::new(c"EverlastTerminationContract", class!(NSObject))
        .ok_or("class exists")?;
    // SAFETY: a separate owned test class with an accurately encoded scalar method.
    unsafe {
        builder.add_method(
            sel!(retainedContractValue),
            preserved as extern "C-unwind" fn(_, _) -> _,
        );
    }
    let class = builder.register();
    // SAFETY: create a normal NSObject subclass and access public NSApplication
    // only on this executable's main thread. No window is shown or activated.
    let (app, delegate): (Retained<AnyObject>, Retained<AnyObject>) = unsafe {
        (
            msg_send![class!(NSApplication), sharedApplication],
            msg_send![class, new],
        )
    };
    let original: Option<Retained<AnyObject>> = unsafe { msg_send![&*app, delegate] };
    unsafe {
        let _: () = msg_send![&*app, setDelegate: &*delegate];
    }
    if control {
        unsafe {
            let _: () = msg_send![&*app, terminate: Option::<&AnyObject>::None];
        }
        return Err("unhooked terminate unexpectedly returned".into());
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::clone(&calls);
    let hook = TerminationHook::install(move || {
        received.fetch_add(1, Ordering::SeqCst);
    })?;
    let reply: usize = unsafe { msg_send![&*delegate, applicationShouldTerminate: &*app] };
    assert_eq!(reply, 0, "AppKit must cancel immediate termination");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "callback must be deferred");
    // Exercise AppKit itself after delegate assignment preceded method addition.
    // The parent requires a nonzero completion sentinel: unexpected process exit
    // here cannot masquerade as a passing child.
    unsafe {
        let _: () = msg_send![&*app, terminate: Option::<&AnyObject>::None];
    }
    let other: Retained<AnyObject> = unsafe { msg_send![class, new] };
    let unrelated: usize = unsafe { msg_send![&*other, applicationShouldTerminate: &*app] };
    assert_eq!(
        unrelated, 1,
        "other delegate instances retain AppKit default"
    );
    let second: usize = unsafe { msg_send![&*delegate, applicationShouldTerminate: &*app] };
    assert_eq!(second, 0);
    let unchanged: usize = unsafe { msg_send![&*delegate, retainedContractValue] };
    assert_eq!(unchanged, 42);
    assert!(matches!(
        TerminationHook::install(|| {}),
        Err(TerminationError::AlreadyInstalled)
    ));
    let deadline = Instant::now() + Duration::from_secs(2);
    while calls.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        // SAFETY: Apple's default run-loop mode constant is immutable.
        unsafe {
            CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(10), false);
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1, "requests must coalesce");
    drop(hook);
    let inactive: usize = unsafe { msg_send![&*delegate, applicationShouldTerminate: &*app] };
    assert_eq!(inactive, 1, "inactive hook preserves AppKit default");
    // A panic cannot unwind across the Objective-C callback boundary or permit
    // native immediate termination. A failed callback remains coalesced.
    let panic_entered = Arc::new(AtomicBool::new(false));
    let entered = Arc::clone(&panic_entered);
    let panic_hook = TerminationHook::install(move || {
        entered.store(true, Ordering::SeqCst);
        std::panic::resume_unwind(Box::new("controlled callback test"))
    })?;
    let reply: usize = unsafe { msg_send![&*delegate, applicationShouldTerminate: &*app] };
    assert_eq!(reply, 0);
    unsafe {
        CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(20), false);
    }
    assert!(
        panic_entered.load(Ordering::SeqCst),
        "panic callback must actually run"
    );
    drop(panic_hook);
    let discarded = Arc::clone(&calls);
    let dropped_hook = TerminationHook::install(move || {
        discarded.fetch_add(100, Ordering::SeqCst);
    })?;
    let reply: usize = unsafe { msg_send![&*delegate, applicationShouldTerminate: &*app] };
    assert_eq!(reply, 0);
    drop(dropped_hook);
    unsafe {
        CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(20), false);
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "queued callback must be inert after owner removal"
    );
    let mut foreign = ClassBuilder::new(c"EverlastForeignTerminationContract", class!(NSObject))
        .ok_or("class exists")?;
    unsafe {
        foreign.add_method(
            sel!(applicationShouldTerminate:),
            foreign_reply as extern "C-unwind" fn(_, _, _) -> _,
        );
    }
    let foreign = foreign.register();
    let foreign: Retained<AnyObject> = unsafe { msg_send![foreign, new] };
    unsafe {
        let _: () = msg_send![&*app, setDelegate: &*foreign];
    }
    assert!(matches!(
        TerminationHook::install(|| {}),
        Err(TerminationError::ExistingImplementation)
    ));
    let untouched: usize = unsafe { msg_send![&*foreign, applicationShouldTerminate: &*app] };
    assert_eq!(untouched, 2, "foreign selector must never be overwritten");
    let wrong_thread = std::thread::spawn(|| {
        matches!(
            TerminationHook::install(|| {}),
            Err(TerminationError::MainThread)
        )
    });
    assert!(wrong_thread.join().map_err(|_| "thread failed")?);
    unsafe {
        let _: () = msg_send![&*app, setDelegate: original.as_deref()];
    }
    Ok(())
}
extern "C-unwind" fn foreign_reply(_: &AnyObject, _: Sel, _: &AnyObject) -> usize {
    2
}
fn main() {
    let control = std::env::args().any(|arg| arg == "--native-control");
    if control || std::env::args().any(|arg| arg == "--native-child") {
        if let Err(error) = run(control) {
            eprintln!("termination contract: {error}");
            std::process::exit(1);
        }
        println!("termination-contract-complete");
        std::process::exit(23);
    }
    let result = std::env::current_exe().and_then(|binary| {
        let control = std::process::Command::new(&binary)
            .arg("--native-control")
            .output()?;
        if control.status.code() != Some(0) || !control.stdout.is_empty() {
            return Err(std::io::Error::other(
                "unhooked AppKit termination control failed",
            ));
        }
        std::process::Command::new(binary)
            .arg("--native-child")
            .output()
    });
    match result {
        Ok(output)
            if output.status.code() == Some(23)
                && output.stdout == b"termination-contract-complete\n" =>
        {
            println!(
                "native termination contract passed (unhooked exit0 control; hooked terminate survived)"
            );
        }
        Ok(output) => {
            eprintln!(
                "native termination child failed: {:?}; {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("native termination child failed: {error}");
            std::process::exit(1);
        }
    }
}
