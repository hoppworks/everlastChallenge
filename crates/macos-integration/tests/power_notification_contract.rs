//! Process-local workspace notifications: no machine sleep, windows or microphone.
#![allow(
    unsafe_code,
    reason = "Posts public immutable notification names only to this test process's workspace center"
)]
use macos_integration::power::{PowerEvent, PowerObserver};
use objc2::{MainThreadMarker, rc::autoreleasepool};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillPowerOffNotification,
    NSWorkspaceWillSleepNotification,
};
use std::sync::{Arc, Mutex};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("requires executable main thread")?;
    let events = Arc::new(Mutex::new(Vec::new()));
    let received = events.clone();
    let mut observer = PowerObserver::new(move |event| {
        // A regression to background delivery must fail even if the enum is right.
        let on_main = MainThreadMarker::new().is_some();
        if let Ok(mut events) = received.lock() {
            events.push((event, on_main));
        }
    })?;
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let _main_lifetime = main;
    // SAFETY: named notifications are immutable public framework constants.
    // NSNotificationCenter posts locally; this does not request OS sleep/shutdown.
    let names = unsafe {
        [
            NSWorkspaceWillSleepNotification,
            NSWorkspaceDidWakeNotification,
            NSWorkspaceWillPowerOffNotification,
        ]
    };
    for name in names {
        unsafe { center.postNotificationName_object(name, None) };
    }
    let expected = vec![
        (PowerEvent::WillSleep, true),
        (PowerEvent::DidWake, true),
        (PowerEvent::WillPowerOff, true),
    ];
    assert_eq!(
        *events.lock().map_err(|_| "poisoned observation")?,
        expected
    );
    observer.remove();
    observer.remove();
    for name in names {
        unsafe { center.postNotificationName_object(name, None) };
    }
    assert_eq!(
        *events.lock().map_err(|_| "poisoned observation")?,
        expected,
        "removed native observers must receive no further events"
    );
    drop(observer);
    unsafe { center.postNotificationName_object(names[0], None) };
    assert_eq!(
        *events.lock().map_err(|_| "poisoned observation")?,
        expected
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    autoreleasepool(|_| run())?;
    println!(
        "Process-local native power registration, event mapping and removal passed; no OS sleep performed"
    );
    Ok(())
}
