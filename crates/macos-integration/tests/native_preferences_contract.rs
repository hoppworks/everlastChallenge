//! Genuine owned AppKit objects; never shows or activates a window.
#![allow(
    unsafe_code,
    reason = "Invokes actions on owned controls with correctly typed targets"
)]
use macos_integration::native_preferences::{
    NativePreferences, PreferenceChoice, PreferencesAction, PreferencesDraft, PreferencesError,
    PreferencesPermission, PreferencesState,
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSButton, NSPopUpButton, NSTabView, NSTextField, NSView,
    NSWindowDidBecomeKeyNotification,
};
use objc2_foundation::NSNotificationCenter;
use std::sync::mpsc;
fn draft() -> PreferencesDraft {
    PreferencesDraft {
        device: None,
        language: "german".into(),
        key: 'D',
        option: true,
        command: false,
        control: false,
        shift: false,
    }
}
fn state() -> PreferencesState {
    PreferencesState {
        current: draft(),
        devices: vec![PreferenceChoice {
            value: "USB".into(),
            label: "USB Mikrofon".into(),
            enabled: true,
        }],
        languages: vec![PreferenceChoice {
            value: "german".into(),
            label: "Deutsch".into(),
            enabled: true,
        }],
        key_labels: ('A'..='Z').map(|key| (key, key.to_string())).collect(),
        busy: false,
        permissions_busy: false,
        microphone: PreferencesPermission::Granted,
        accessibility: PreferencesPermission::Denied,
        model_summary: "Bereit".into(),
        model_details: "Lokale Erkennung".into(),
        error: None,
    }
}
fn descendants<T: objc2::DowncastTarget + 'static>(view: &NSView) -> Vec<Retained<T>> {
    let mut found = Vec::new();
    for child in view.subviews() {
        if let Ok(item) = child.clone().downcast::<T>() {
            found.push(item);
        }
        found.extend(descendants::<T>(&child));
    }
    found
}
fn button(root: &NSView, title: &str) -> Result<Retained<NSButton>, Box<dyn std::error::Error>> {
    descendants::<NSButton>(root)
        .into_iter()
        .find(|button| button.title().to_string() == title)
        .ok_or_else(|| format!("missing button {title}").into())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread")?;
    let app = NSApplication::sharedApplication(main);
    let active = app.isActive();
    let (send, recv) = mpsc::channel();
    let mut prefs = NativePreferences::new(move |action| {
        if send.send(action).is_err() {
            eprintln!("contract receiver ended");
        }
    })?;
    prefs.update(state())?;
    let window = app
        .windows()
        .iter()
        .find(|window| window.title().to_string() == "Einstellungen")
        .ok_or("window")?;
    assert!(!window.isVisible());
    let root = window.contentView().ok_or("root")?;
    let tabs = descendants::<NSTabView>(&root).pop().ok_or("native tabs")?;
    let labels: Vec<_> = tabs
        .tabViewItems()
        .iter()
        .map(|item| item.label().to_string())
        .collect();
    assert_eq!(labels, ["Aufnahme", "Berechtigungen", "Modell"]);
    assert_eq!(
        tabs.selectedTabViewItem()
            .ok_or("selected")?
            .label()
            .to_string(),
        "Berechtigungen"
    );
    for item in tabs.tabViewItems() {
        let page = item.view(main).ok_or("tab page")?;
        for control in page.subviews() {
            let frame = control.frame();
            assert!(frame.origin.x >= 0. && frame.origin.y >= 0.);
            assert!(frame.origin.x + frame.size.width <= page.bounds().size.width);
            assert!(
                frame.origin.y + frame.size.height <= page.bounds().size.height,
                "control outside tab content"
            );
        }
    }
    let model_page = tabs
        .tabViewItems()
        .objectAtIndex(2)
        .view(main)
        .ok_or("model page")?;
    let details = descendants::<NSTextField>(&model_page)
        .into_iter()
        .find(|field| field.stringValue().to_string() == "Lokale Erkennung")
        .ok_or("model details")?;
    assert!(
        details.isHidden(),
        "technical model details start collapsed"
    );
    let disclosure = descendants::<NSButton>(&model_page)
        .into_iter()
        .find(|button| {
            button
                .accessibilityLabel()
                .is_some_and(|label| label.to_string() == "Technische Details")
        })
        .ok_or("native disclosure")?;
    tabs.selectTabViewItemAtIndex(2);
    unsafe {
        disclosure.performClick(None);
    }
    assert!(!details.isHidden());
    tabs.selectTabViewItemAtIndex(0);
    let mut telemetry = state();
    telemetry.model_summary = "Modell bereit".into();
    prefs.update(telemetry)?;
    tabs.selectTabViewItemAtIndex(2);
    assert!(
        !details.isHidden(),
        "telemetry and tab switches preserve disclosure choice"
    );
    unsafe {
        disclosure.performClick(None);
    }
    assert!(details.isHidden());
    let error = root
        .subviews()
        .iter()
        .find_map(|view| view.downcast::<NSTextField>().ok())
        .ok_or("error label")?;
    assert!(error.isHidden(), "empty error label must not appear");
    let mut failure = state();
    failure.error = Some("Testfehler".into());
    prefs.update(failure)?;
    assert!(!error.isHidden());
    prefs.update(state())?;
    assert!(error.isHidden());
    tabs.selectTabViewItemAtIndex(1);
    let center = NSNotificationCenter::defaultCenter();
    // Controlled notification boundary on our owned invisible window: not an
    // assertion that a real foreground/focus transition was exercised.
    unsafe {
        center.postNotificationName_object(NSWindowDidBecomeKeyNotification, Some(&window));
    }
    assert_eq!(recv.try_recv()?, PreferencesAction::RefreshPermissions);
    let request = button(&root, "Freigabe anfragen")?;
    unsafe { request.performClick(None) };
    assert_eq!(recv.try_recv()?, PreferencesAction::RequestAX);
    tabs.selectTabViewItemAtIndex(0);
    let device = descendants::<NSPopUpButton>(&root)
        .into_iter()
        .find(|popup| popup.numberOfItems() == 2)
        .ok_or("device popup")?;
    device.selectItemAtIndex(1);
    let mut next = state();
    next.model_summary = "Wird geprüft".into();
    prefs.update(next.clone())?;
    tabs.selectTabViewItemAtIndex(2);
    tabs.selectTabViewItemAtIndex(0);
    let save = button(&root, "Speichern")?;
    unsafe { save.performClick(None) };
    let mut expected = draft();
    expected.device = Some("USB".into());
    assert_eq!(recv.try_recv()?, PreferencesAction::Save(expected));
    let mut invalid = state();
    invalid.devices[0].label = "x".repeat(1025);
    assert_eq!(prefs.update(invalid), Err(PreferencesError::State));
    assert_eq!(
        device.indexOfSelectedItem(),
        1,
        "invalid updates preserve the draft"
    );
    next.current.key = 'G';
    prefs.update(next.clone())?;
    assert_eq!(
        device.indexOfSelectedItem(),
        0,
        "changed backend settings replace the old draft"
    );
    next.busy = true;
    next.permissions_busy = false;
    prefs.update(next)?;
    assert!(!save.isEnabled());
    unsafe { request.performClick(None) };
    assert_eq!(recv.try_recv()?, PreferencesAction::RequestAX);
    prefs.close()?;
    assert!(!window.isVisible());
    prefs.detach()?;
    prefs.detach()?;
    unsafe { request.performClick(None) };
    unsafe { save.performClick(None) };
    unsafe {
        center.postNotificationName_object(NSWindowDidBecomeKeyNotification, Some(&window));
    }
    assert!(recv.try_recv().is_err());
    assert_eq!(prefs.show(), Err(PreferencesError::Inactive));
    assert_eq!(app.isActive(), active);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let captured = calls.clone();
    let mut failing = NativePreferences::new(move |_| {
        captured.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::panic::resume_unwind(Box::new("controlled preferences callback panic"));
    })?;
    failing.update(state())?;
    let failing_window = app
        .windows()
        .iter()
        .find(|candidate| {
            candidate.title().to_string() == "Einstellungen"
                && !std::ptr::eq(&**candidate, &*window)
        })
        .ok_or("second window")?;
    let failing_root = failing_window.contentView().ok_or("second root")?;
    let failing_request = button(&failing_root, "Freigabe anfragen")?;
    unsafe {
        failing_request.performClick(None);
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!failing_request.isEnabled());
    assert_eq!(failing.update(state()), Err(PreferencesError::Inactive));
    failing.detach()?;
    unsafe {
        center.postNotificationName_object(NSWindowDidBecomeKeyNotification, Some(&failing_window));
    }
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "panic still permits full observer teardown"
    );
    assert_eq!(app.isActive(), active);
    assert_eq!(
        std::thread::spawn(|| NativePreferences::new(|_| {}).err())
            .join()
            .map_err(|_| "join")?,
        Some(PreferencesError::MainThread)
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    run()
}
