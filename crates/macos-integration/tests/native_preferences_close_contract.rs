//! Own AppKit windows only. No foreign AX, permissions, text, audio or model.
#![allow(
    unsafe_code,
    reason = "Exercises public close action on a retained own native window"
)]
use macos_integration::native_preferences::{
    NativePreferences, PreferenceChoice, PreferencesDraft, PreferencesPermission, PreferencesState,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSTabView, NSTextView, NSWindow, NSWindowButton,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    // SAFETY: designated initializer on the main thread for our own retained window.
    let main_window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(100., 100.), NSSize::new(640., 400.)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Rust owns this window throughout, including after close.
    unsafe {
        main_window.setReleasedWhenClosed(false);
    }
    main_window.setTitle(&NSString::from_str("Own close-contract main"));
    let mut prefs = NativePreferences::new(|_| {})?;
    prefs.update(PreferencesState {
        current: PreferencesDraft {
            device: None,
            language: "german".into(),
            key: 'D',
            option: true,
            command: false,
            control: false,
            shift: false,
        },
        devices: vec![],
        languages: vec![PreferenceChoice {
            value: "german".into(),
            label: "Deutsch".into(),
            enabled: true,
        }],
        key_labels: ('A'..='Z').map(|key| (key, key.to_string())).collect(),
        busy: false,
        permissions_busy: false,
        microphone: PreferencesPermission::Granted,
        accessibility: PreferencesPermission::Granted,
        model_summary: "Controlled local state".into(),
        model_details: String::new(),
        error: None,
    })?;
    let pref_window = app
        .windows()
        .iter()
        .find(|window| window.title().to_string() == "Einstellungen")
        .ok_or("preferences window missing")?;
    let full_keyboard_access = app.isFullKeyboardAccessEnabled();
    main_window.makeKeyAndOrderFront(None);
    assert!(main_window.isVisible());
    for iteration in 1..=3 {
        prefs.show()?;
        assert!(pref_window.isVisible());
        // The public show lifecycle supplies its promised keyboard entry point.
        // No test-side responder assignment precedes this initial/reopen check.
        assert!(
            pref_window
                .firstResponder()
                .is_some_and(|r| r.downcast_ref::<NSTabView>().is_some()),
            "show/reopen did not establish the configured initial tab responder"
        );
        assert_eq!(app.isFullKeyboardAccessEnabled(), full_keyboard_access);
        assert!(main_window.isVisible());
        let close = pref_window
            .standardWindowButton(NSWindowButton::CloseButton)
            .ok_or("close button missing")?;
        // SAFETY: same public NSButton action as our own native traffic-light click.
        unsafe {
            close.performClick(None);
        }
        println!(
            "iteration={iteration}; preferencesVisible={}; mainVisible={}; mainInOwnWindows={}",
            pref_window.isVisible(),
            main_window.isVisible(),
            app.windows().iter().any(|w| w == main_window)
        );
        assert!(
            !pref_window.isVisible(),
            "own close button did not close preferences"
        );
        assert!(
            main_window.isVisible(),
            "closing preferences hid the main window"
        );
        assert!(app.windows().iter().any(|window| window == main_window));
    }
    prefs.show()?;
    // Simulate an already focused owned editing control for the preservation case,
    // separately from the initial/reopen assertions above.
    let editor = NSTextView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(80., 24.)),
    );
    pref_window
        .contentView()
        .ok_or("own content missing")?
        .addSubview(&editor);
    assert!(pref_window.makeFirstResponder(Some(&editor)));
    let preserved = pref_window
        .firstResponder()
        .ok_or("owned control focus missing")?;
    prefs.show()?;
    assert_eq!(
        pref_window.firstResponder().as_deref(),
        Some(&*preserved),
        "show replaced existing control focus"
    );
    prefs.close()?;
    prefs.show()?;
    assert_eq!(
        pref_window.firstResponder().as_deref(),
        Some(&*preserved),
        "reopen replaced existing control focus"
    );
    assert_eq!(app.isFullKeyboardAccessEnabled(), full_keyboard_access);
    prefs.close()?;
    assert!(!pref_window.isVisible());
    assert!(main_window.isVisible(), "preferences API close hid main");
    prefs.detach()?;
    assert!(main_window.isVisible(), "preferences detach hid main");
    main_window.close();
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
