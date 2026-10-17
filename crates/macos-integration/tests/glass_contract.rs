//! Runs AppKit contracts on main without showing or activating any window.
#![allow(
    unsafe_code,
    reason = "This native integration test constructs genuine AppKit windows and a lifetime-bound raw handle."
)]
use macos_integration::glass::{GlassAttachment, GlassError};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSGlassEffectView, NSGlassEffectViewStyle, NSTextView,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use std::ptr::NonNull;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
struct Owner(Retained<NSWindow>);
impl HasWindowHandle for Owner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let view = self.0.contentView().ok_or(HandleError::Unavailable)?;
        let raw = AppKitWindowHandle::new(NonNull::from(&*view).cast());
        // SAFETY: this owner's retained window owns its current content view;
        // the handle borrows self and no view is replaced during this borrow.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(raw)) })
    }
}

struct TrackedOwner {
    inner: Owner,
    dropped: Arc<AtomicBool>,
}
impl HasWindowHandle for TrackedOwner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}
impl Drop for TrackedOwner {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}

struct ViewOwner(Retained<NSView>);
impl HasWindowHandle for ViewOwner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = AppKitWindowHandle::new(NonNull::from(&*self.0).cast());
        // SAFETY: this owner retains the concrete NSView for the whole borrow.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(raw)) })
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("not on main thread")?;
    let app = NSApplication::sharedApplication(main);
    let active = app.isActive();
    // SAFETY: valid finite dimensions and documented style/backing values; an
    // NSApplication exists. No ordering or activation method is invoked.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(400., 300.)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Rust Retained owns destruction, avoiding AppKit's close-time release.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    let original = NSView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(400., 300.)),
    );
    window.setContentView(Some(&original));
    let unattached = NSView::new(main);
    assert!(matches!(
        GlassAttachment::attach(ViewOwner(unattached)),
        Err(GlassError::Detached)
    ));
    let focus = NSTextView::new(main);
    original.addSubview(&focus);
    assert!(matches!(
        GlassAttachment::attach(ViewOwner(focus.clone().into_super().into_super())),
        Err(GlassError::Changed)
    ));
    assert!(window.makeFirstResponder(Some(&focus)));
    let first_before = window
        .firstResponder()
        .ok_or("missing initial first responder")?;
    let owner = Owner(window.clone());
    let mut attachment = GlassAttachment::attach(owner)?;
    let first_after = window
        .firstResponder()
        .ok_or("missing attached first responder")?;
    assert!(std::ptr::eq(&*first_before, &*first_after));
    let glass = window
        .contentView()
        .ok_or("no content")?
        .downcast::<NSGlassEffectView>()
        .map_err(|_| "no glass installed")?;
    assert_eq!(glass.style(), NSGlassEffectViewStyle::Regular);
    assert_eq!(glass.contentView().as_deref(), Some(original.as_ref()));
    assert!(original.isDescendantOf(&glass));
    assert!(matches!(
        GlassAttachment::attach(Owner(window.clone())),
        Err(GlassError::Busy)
    ));
    window.setContentSize(NSSize::new(640., 480.));
    glass.layoutSubtreeIfNeeded();
    assert_eq!(glass.bounds().size, NSSize::new(640., 480.));
    assert_eq!(original.frame().size, NSSize::new(640., 480.));
    let second_focus = NSTextView::new(main);
    original.addSubview(&second_focus);
    assert!(window.makeFirstResponder(Some(&second_focus)));
    let immediate_before_detach = window.firstResponder().ok_or("missing changed responder")?;
    assert!(!std::ptr::eq(&*first_before, &*immediate_before_detach));
    attachment.detach()?;
    let first_after_detach = window
        .firstResponder()
        .ok_or("missing restored first responder")?;
    assert!(std::ptr::eq(
        &*immediate_before_detach,
        &*first_after_detach
    ));
    assert_eq!(window.contentView().as_deref(), Some(original.as_ref()));
    assert_eq!(original.frame().size, NSSize::new(640., 480.));
    attachment.detach()?;
    drop(attachment);
    for _ in 0..3 {
        let next = GlassAttachment::attach(Owner(window.clone()))?;
        drop(next);
        assert_eq!(window.contentView().as_deref(), Some(original.as_ref()));
        assert_eq!(original.frame().size, NSSize::new(640., 480.));
    }

    let mut attachment = GlassAttachment::attach(Owner(window.clone()))?;
    let replacement = NSView::initWithFrame(
        main.alloc(),
        window.contentView().ok_or("missing content")?.bounds(),
    );
    window.setContentView(Some(&replacement));
    replacement.addSubview(&original);
    assert_eq!(attachment.detach(), Err(GlassError::Changed));
    assert!(matches!(
        GlassAttachment::attach(Owner(window.clone())),
        Err(GlassError::Busy)
    ));
    drop(attachment);
    assert_eq!(window.contentView().as_deref(), Some(replacement.as_ref()));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut tracked = GlassAttachment::attach(TrackedOwner {
        inner: Owner(window.clone()),
        dropped: dropped.clone(),
    })?;
    assert!(!dropped.load(Ordering::Relaxed));
    tracked.detach()?;
    assert!(!dropped.load(Ordering::Relaxed));
    drop(tracked);
    assert!(dropped.load(Ordering::Relaxed));
    assert!(original.isDescendantOf(&replacement));
    let recovered = GlassAttachment::attach(Owner(window.clone()))?;
    drop(recovered);
    assert_eq!(window.contentView().as_deref(), Some(replacement.as_ref()));
    assert!(original.isDescendantOf(&replacement));

    let mut changed_content = GlassAttachment::attach(Owner(window.clone()))?;
    let changed_glass = window
        .contentView()
        .ok_or("missing glass")?
        .downcast::<NSGlassEffectView>()
        .map_err(|_| "missing glass type")?;
    let foreign_content = NSView::initWithFrame(main.alloc(), changed_glass.bounds());
    changed_glass.setContentView(Some(&foreign_content));
    assert_eq!(changed_content.detach(), Err(GlassError::Changed));
    drop(changed_content);
    assert_eq!(
        changed_glass.contentView().as_deref(),
        Some(foreign_content.as_ref())
    );
    assert_eq!(
        window.contentView().as_deref(),
        Some(changed_glass.as_ref())
    );
    window.setContentView(Some(&replacement));
    let recovered = GlassAttachment::attach(Owner(window.clone()))?;
    drop(recovered);
    assert_eq!(window.contentView().as_deref(), Some(replacement.as_ref()));
    assert!(!window.isVisible());
    assert_eq!(app.isActive(), active);
    window.close();
    println!(
        "glass-contract: attach/Regular/content identity/resize/detach/reattach/foreign replacement passed; window remained invisible"
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
