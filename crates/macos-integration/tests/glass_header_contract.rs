//! Genuine AppKit contract, confined to an invisible, never activated window.
#![allow(
    unsafe_code,
    reason = "Constructs owned native test objects and lifetime-bound handles"
)]
use macos_integration::glass_header::{
    GlassHeader, HeaderAction, HeaderError, HeaderPrimary, HeaderState,
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSButton, NSEvent, NSEventModifierFlags, NSEventType,
    NSGlassEffectView, NSGlassEffectViewStyle, NSLayoutAttribute, NSTextField, NSTextView,
    NSTitlebarAccessoryViewController, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use std::{
    num::NonZeroU64,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone)]
struct Owner(Retained<NSWindow>);
impl HasWindowHandle for Owner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let view = self.0.contentView().ok_or(HandleError::Unavailable)?;
        // SAFETY: the retained window owns this view throughout the handle borrow.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(
                NonNull::from(&*view).cast(),
            )))
        })
    }
}
struct TrackedOwner {
    owner: Owner,
    dropped: Arc<AtomicBool>,
}
impl HasWindowHandle for TrackedOwner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.owner.window_handle()
    }
}
impl Drop for TrackedOwner {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}
fn click(button: &NSButton) {
    unsafe {
        button.performClick(None);
    }
}
fn button(view: &NSView, title: &str) -> Result<Retained<NSButton>, &'static str> {
    view.subviews()
        .iter()
        .filter_map(|view| view.downcast::<NSButton>().ok())
        .find(|button| button.title().to_string() == title)
        .ok_or("native button missing")
}
fn key(
    window: &NSWindow,
    characters: &str,
    modifiers: NSEventModifierFlags,
) -> Result<Retained<NSEvent>, &'static str> {
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0., 0.), modifiers, 0., window.windowNumber(), None,
        &NSString::from_str(characters), &NSString::from_str(characters), false, 0,
    ).ok_or("owned native key event")
}
fn panic_callback(_: HeaderAction) {
    // Exercise unwinding at the callback boundary without a production panic
    // shortcut or the global panic hook's expected test-only noise.
    std::panic::resume_unwind(Box::new("intentional callback test"));
}
fn toolbar_fits(content: &NSView, width: f64) -> Result<(), &'static str> {
    let status = content
        .subviews()
        .iter()
        .find_map(|view| view.downcast::<NSTextField>().ok())
        .ok_or("status")?;
    let mut edge = status.frame().origin.x + status.frame().size.width;
    assert!(
        status.frame().size.width >= 160.0,
        "status needs useful space at minimum width"
    );
    for title in ["Aufnehmen", "Stoppen", "Abbrechen", "Einstellungen"] {
        let Ok(control) = button(content, title) else {
            continue;
        };
        if control.isHidden() {
            continue;
        }
        let frame = control.frame();
        assert!(
            edge + 8.0 <= frame.origin.x,
            "toolbar controls must not collide"
        );
        assert_eq!(
            frame.origin.y + frame.size.height / 2.0,
            32.0,
            "one control row"
        );
        edge = frame.origin.x + frame.size.width;
    }
    assert!(
        edge <= width - 14.0,
        "controls must fit inside the trailing margin"
    );
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("not main")?;
    let app = NSApplication::sharedApplication(main);
    let active = app.isActive();
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(560., 600.)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    let root = window.contentView().ok_or("root")?;
    let first = NSTextView::new(main);
    root.addSubview(&first);
    assert!(window.makeFirstResponder(Some(&first)));
    let responder = window.firstResponder().ok_or("responder")?;
    let foreign = NSTitlebarAccessoryViewController::new(main);
    foreign.setView(&NSView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(560., 20.)),
    ));
    window.addTitlebarAccessoryViewController(&foreign);
    let (send, recv) = mpsc::channel();
    let mut header = GlassHeader::attach(Owner(window.clone()), move |action| {
        assert!(send.send(action).is_ok());
    })?;
    assert!(matches!(
        GlassHeader::attach(Owner(window.clone()), |_| {}),
        Err(HeaderError::Busy)
    ));
    assert_eq!(window.contentView().as_deref(), Some(root.as_ref()));
    assert_eq!(window.firstResponder().as_deref(), Some(responder.as_ref()));
    let accessory = window
        .titlebarAccessoryViewControllers()
        .lastObject()
        .ok_or("accessory")?;
    assert_eq!(accessory.layoutAttribute(), NSLayoutAttribute::Bottom);
    let glass = accessory
        .view()
        .downcast::<NSGlassEffectView>()
        .map_err(|_| "glass")?;
    assert_eq!(glass.style(), NSGlassEffectViewStyle::Regular);
    let content = glass.contentView().ok_or("content")?;
    glass.layoutSubtreeIfNeeded();
    assert_eq!(glass.frame().size.width, 560.);
    assert_eq!(glass.frame().size.height, 64.);
    assert_eq!(content.subviews().len(), 4);
    let primary = button(&content, "Aufnehmen")?;
    let cancel = button(&content, "Abbrechen")?;
    let settings = button(&content, "Einstellungen")?;
    for control in [&primary, &cancel, &settings] {
        click(control);
    }
    assert!(recv.try_recv().is_err());
    header.update(HeaderState {
        status: "Bereit".into(),
        primary: Some(HeaderPrimary::Start),
        cancel: None,
        settings_enabled: true,
    })?;
    toolbar_fits(&content, 560.)?;
    let record_key = key(&window, "r", NSEventModifierFlags::Command)?;
    assert!(
        window.performKeyEquivalent(&record_key),
        "owned window must dispatch its recording shortcut"
    );
    assert_eq!(recv.try_recv()?, HeaderAction::Start);
    let settings_key = key(&window, ",", NSEventModifierFlags::Command)?;
    assert!(window.performKeyEquivalent(&settings_key));
    assert_eq!(recv.try_recv()?, HeaderAction::Settings);
    let plain_r = key(&window, "r", NSEventModifierFlags::empty())?;
    window.performKeyEquivalent(&plain_r);
    assert!(recv.try_recv().is_err());
    click(&primary);
    click(&settings);
    assert_eq!(recv.try_recv()?, HeaderAction::Start);
    assert_eq!(recv.try_recv()?, HeaderAction::Settings);
    for number in [17, 18] {
        let id = NonZeroU64::new(number).ok_or("id")?;
        header.update(HeaderState {
            status: "Aufnahme läuft".into(),
            primary: Some(HeaderPrimary::Stop(id)),
            cancel: Some(id),
            settings_enabled: false,
        })?;
        toolbar_fits(&content, 560.)?;
        window.performKeyEquivalent(&settings_key);
        assert!(
            recv.try_recv().is_err(),
            "disabled Settings must not dispatch"
        );
        assert!(window.performKeyEquivalent(&record_key));
        assert_eq!(recv.try_recv()?, HeaderAction::Stop(id));
        let cancel_key = key(&window, "\u{1b}", NSEventModifierFlags::empty())?;
        assert!(window.performKeyEquivalent(&cancel_key));
        assert_eq!(recv.try_recv()?, HeaderAction::Cancel(id));
        click(&primary);
        click(&cancel);
        click(&settings);
        assert_eq!(recv.try_recv()?, HeaderAction::Stop(id));
        assert_eq!(recv.try_recv()?, HeaderAction::Cancel(id));
        assert!(recv.try_recv().is_err());
    }
    for status in [String::new(), "x".repeat(513), "invalid\0status".into()] {
        assert_eq!(
            header.update(HeaderState {
                status,
                ..HeaderState::default()
            }),
            Err(HeaderError::State)
        );
    }
    assert_eq!(
        header.update(HeaderState {
            status: "Bad ids".into(),
            primary: Some(HeaderPrimary::Stop(NonZeroU64::new(1).ok_or("id")?)),
            cancel: NonZeroU64::new(2),
            settings_enabled: false
        }),
        Err(HeaderError::State)
    );
    for width in [760., 1000., 560.] {
        window.setContentSize(NSSize::new(width, 720.));
        glass.layoutSubtreeIfNeeded();
        assert_eq!(glass.frame().size.width, width);
        assert_eq!(content.frame().size.width, width);
        toolbar_fits(&content, width)?;
    }
    header.update(HeaderState {
        status: "Modell wird geprüft".into(),
        primary: None,
        cancel: None,
        settings_enabled: true,
    })?;
    toolbar_fits(&content, 560.)?;
    assert!(primary.isHidden() && cancel.isHidden());
    window.performKeyEquivalent(&record_key);
    let hidden_cancel_key = key(&window, "\u{1b}", NSEventModifierFlags::empty())?;
    window.performKeyEquivalent(&hidden_cancel_key);
    assert!(
        recv.try_recv().is_err(),
        "hidden recording actions must not dispatch"
    );
    click(&settings);
    assert_eq!(recv.try_recv()?, HeaderAction::Settings);
    let second = NSTextView::new(main);
    root.addSubview(&second);
    assert!(window.makeFirstResponder(Some(&second)));
    let before_detach = window.firstResponder().ok_or("second responder")?;
    header.detach()?;
    header.detach()?;
    assert_eq!(
        window.firstResponder().as_deref(),
        Some(before_detach.as_ref())
    );
    assert_eq!(window.titlebarAccessoryViewControllers().len(), 1);
    assert_eq!(
        window
            .titlebarAccessoryViewControllers()
            .firstObject()
            .as_deref(),
        Some(foreign.as_ref())
    );
    for control in [&primary, &cancel, &settings] {
        click(control);
    }
    assert!(recv.try_recv().is_err());
    drop(header);
    for _ in 0..3 {
        drop(GlassHeader::attach(Owner(window.clone()), |_| {})?);
        assert_eq!(window.titlebarAccessoryViewControllers().len(), 1);
    }
    let mut changed = GlassHeader::attach(Owner(window.clone()), |_| {})?;
    let owned = window
        .titlebarAccessoryViewControllers()
        .lastObject()
        .ok_or("owned")?;
    owned.removeFromParentViewController();
    assert_eq!(
        changed.update(HeaderState::default()),
        Err(HeaderError::Changed)
    );
    drop(changed);
    drop(GlassHeader::attach(Owner(window.clone()), |_| {})?);
    assert_eq!(window.titlebarAccessoryViewControllers().len(), 1);
    let dropped = Arc::new(AtomicBool::new(false));
    let mut tracked = GlassHeader::attach(
        TrackedOwner {
            owner: Owner(window.clone()),
            dropped: dropped.clone(),
        },
        |_| {},
    )?;
    tracked.detach()?;
    assert!(!dropped.load(Ordering::Relaxed));
    drop(tracked);
    assert!(dropped.load(Ordering::Relaxed));

    // A Rust panic is contained at the native callback boundary; later actions
    // and updates are rejected instead of crossing ObjC's unwinding boundary.
    let mut panicking = GlassHeader::attach(Owner(window.clone()), panic_callback)?;
    panicking.update(HeaderState {
        status: "Bereit".into(),
        primary: Some(HeaderPrimary::Start),
        ..HeaderState::default()
    })?;
    let panic_accessory = window
        .titlebarAccessoryViewControllers()
        .lastObject()
        .ok_or("panic accessory")?;
    let panic_glass = panic_accessory
        .view()
        .downcast::<NSGlassEffectView>()
        .map_err(|_| "panic glass")?;
    let panic_content = panic_glass.contentView().ok_or("panic content")?;
    click(&*button(&panic_content, "Aufnehmen")?);
    assert_eq!(
        panicking.update(HeaderState::default()),
        Err(HeaderError::Inactive)
    );
    panicking.detach()?;
    assert_eq!(window.titlebarAccessoryViewControllers().len(), 1);
    assert!(!window.isVisible());
    assert_eq!(app.isActive(), active);
    window.close();
    println!(
        "glass-header-contract: native actions/session ids, disabled controls, full width/resize, focus, detach/foreign accessory and reattach passed; never visible"
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
