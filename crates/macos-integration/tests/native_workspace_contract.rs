//! Real invisible AppKit workspace; never records, copies or activates an app.
#![allow(
    unsafe_code,
    reason = "Owns invisible native contract views and borrows a lifetime-bound AppKit handle"
)]
use macos_integration::glass_header::{GlassHeader, HeaderState};
use macos_integration::native_workspace::{
    DownloadReadout, NativeWorkspace, RecordingReadout, WorkspaceAction, WorkspaceError,
    WorkspaceState,
};
use objc2::{AnyThread, MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSApplication, NSBackingStoreType, NSBitmapImageRep,
    NSBorderType, NSButton, NSColor, NSDeviceRGBColorSpace, NSEvent, NSEventModifierFlags,
    NSEventType, NSGraphicsContext, NSRectFill, NSScrollView, NSTextField, NSTextView, NSView,
    NSWindow, NSWindowButton, NSWindowStyleMask,
};
use objc2_foundation::{NSAffineTransform, NSPoint, NSRange, NSRect, NSSize, NSString};
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use std::{ptr::NonNull, sync::mpsc};
#[derive(Clone)]
struct Owner {
    _window: Retained<NSWindow>,
    view: Retained<NSView>,
}
impl HasWindowHandle for Owner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: Owner retains the exact view throughout the handle borrow.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(
                NonNull::from(&*self.view).cast(),
            )))
        })
    }
}
fn button(view: &NSView, title: &str) -> Result<Retained<NSButton>, &'static str> {
    view.subviews()
        .iter()
        .filter_map(|value| value.downcast::<NSButton>().ok())
        .find(|value| value.title().to_string() == title)
        .ok_or("button missing")
}
fn click(value: &NSButton) {
    unsafe {
        value.performClick(None);
    }
}
// Exercise the actual drawRect override with an oversized dirty rectangle in an
// offscreen bitmap. No window is displayed and no view hierarchy is captured.
fn background_is_contained(body: &NSView) -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: null planes asks AppKit to own a bounded 64x64 RGBA allocation;
    // the public device RGB constant outlives this initialized bitmap.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), 64, 64, 8, 4,
            true, false, NSDeviceRGBColorSpace, 256, 32,
        )
    }.ok_or("offscreen bitmap")?;
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)
        .ok_or("offscreen graphics context")?;
    let previous = NSGraphicsContext::currentContext();
    NSGraphicsContext::setCurrentContext(Some(&context));
    context.saveGraphicsState();
    NSColor::magentaColor().setFill();
    NSRectFill(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0)));
    let bounds = body.bounds();
    let transform = NSAffineTransform::new();
    transform.translateXBy_yBy(16.0, 16.0);
    transform.scaleXBy_yBy(32.0 / bounds.size.width, 32.0 / bounds.size.height);
    transform.concat();
    body.drawRect(NSRect::new(
        NSPoint::new(-bounds.size.width, -bounds.size.height),
        NSSize::new(bounds.size.width * 3.0, bounds.size.height * 3.0),
    ));
    context.restoreGraphicsState();
    NSGraphicsContext::setCurrentContext(previous.as_deref());
    let is_sentinel = |x, y| -> Result<bool, Box<dyn std::error::Error>> {
        let color = bitmap.colorAtX_y(x, y).ok_or("bitmap color")?;
        Ok(color.redComponent() > 0.95
            && color.greenComponent() < 0.05
            && color.blueComponent() > 0.95)
    };
    for (x, y) in [(1, 1), (62, 62), (1, 32), (32, 1)] {
        assert!(
            is_sentinel(x, y)?,
            "workspace painter overwrote surrounding sentinel at {x},{y}"
        );
    }
    assert!(
        !is_sentinel(32, 32)?,
        "workspace background was not painted"
    );
    assert!(
        body.clipsToBounds(),
        "AppKit must clip workspace subview drawing"
    );
    Ok(())
}
fn full_size_header_contract() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main")?;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(760.0, 720.0)),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    let original = window.contentView().ok_or("original root")?;
    let parent = unsafe { original.superview() }.ok_or("root parent")?;
    let owner = Owner {
        _window: window.clone(),
        view: original.clone(),
    };
    let mut header = GlassHeader::attach(owner.clone(), |_| {})?;
    header.update(HeaderState {
        status: "Bereit".into(),
        ..HeaderState::default()
    })?;
    let mut workspace = NativeWorkspace::attach(owner, |_| {})?;
    workspace.update(WorkspaceState {
        status: "Bereit".into(),
        ..WorkspaceState::default()
    })?;
    let body = parent
        .subviews()
        .iter()
        .find(|view| {
            view.subviews()
                .iter()
                .any(|child| child.downcast::<NSScrollView>().is_ok())
        })
        .ok_or("native body")?;
    background_is_contained(&body)?;
    let status = body
        .subviews()
        .iter()
        .filter_map(|view| view.downcast::<NSTextField>().ok())
        .find(|view| view.stringValue().to_string() == "Bereit")
        .ok_or("status")?;
    for (width, height) in [(560.0, 600.0), (760.0, 720.0), (560.0, 640.0)] {
        window.setContentSize(NSSize::new(width, height));
        window.layoutIfNeeded();
        parent.layoutSubtreeIfNeeded();
        let unobscured = window.contentLayoutRect();
        let actual = body.convertRect_toView(body.bounds(), None);
        assert!(
            (actual.origin.y + actual.size.height - (unobscured.origin.y + unobscured.size.height))
                .abs()
                < 0.5,
            "workspace must stop below titlebar/accessory, not use full-size root frame: actual={actual:?}, unobscured={unobscured:?}"
        );
        assert!(
            (actual.origin.x - unobscured.origin.x).abs() < 0.5
                && (actual.size.width - unobscured.size.width).abs() < 0.5
                && (actual.origin.y - unobscured.origin.y).abs() < 0.5
                && (actual.size.height - unobscured.size.height).abs() < 0.5
        );
        let status_rect = status.convertRect_toView(status.bounds(), None);
        let accessory = window
            .titlebarAccessoryViewControllers()
            .lastObject()
            .ok_or("glass accessory")?
            .view();
        let glass_rect = accessory.convertRect_toView(accessory.bounds(), None);
        assert!(
            status_rect.origin.y + status_rect.size.height <= glass_rect.origin.y,
            "status must be below the actual glass header"
        );
        let close = window
            .standardWindowButton(NSWindowButton::CloseButton)
            .ok_or("traffic light")?;
        let traffic = close.convertRect_toView(close.bounds(), None);
        assert!(
            status_rect.origin.y + status_rect.size.height <= traffic.origin.y,
            "status must not overlap traffic lights"
        );
        assert_eq!(window.contentView().as_deref(), Some(original.as_ref()));
        assert!(original.isHidden());
    }
    workspace.detach()?;
    header.detach()?;
    assert!(!original.isHidden());
    assert!(!window.isVisible());
    window.close();
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread")?;
    let app = NSApplication::sharedApplication(main);
    let active = app.isActive();
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(700.0, 520.0)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    let root = window.contentView().ok_or("root")?;
    let original = NSView::initWithFrame(main.alloc(), root.bounds());
    original.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    root.addSubview(&original);
    let old_text = NSTextView::new(main);
    original.addSubview(&old_text);
    assert!(window.makeFirstResponder(Some(&old_text)));
    let before = window.firstResponder().ok_or("old responder")?;
    let owner = Owner {
        _window: window.clone(),
        view: original.clone(),
    };
    let (send, receive) = mpsc::channel();
    let mut workspace = NativeWorkspace::attach(owner.clone(), move |action| {
        assert!(send.send(action).is_ok());
    })?;
    assert!(
        original.isHidden(),
        "the retained webview must not remain visible"
    );
    assert_eq!(
        window.contentView().as_deref(),
        Some(root.as_ref()),
        "Tauri's content root must remain installed"
    );
    assert!(matches!(
        NativeWorkspace::attach(owner.clone(), |_| {}),
        Err(WorkspaceError::Busy)
    ));
    let body = root.subviews().lastObject().ok_or("native body")?;
    let scroll = body
        .subviews()
        .iter()
        .find_map(|value| value.downcast::<NSScrollView>().ok())
        .ok_or("scroll")?;
    let text = scroll
        .documentView()
        .ok_or("document")?
        .downcast::<NSTextView>()
        .map_err(|_| "text")?;
    assert!(!text.isEditable() && text.isSelectable());
    assert_eq!(scroll.borderType(), NSBorderType::BezelBorder);
    assert!(body.isOpaque());
    let copy = button(&body, "Kopieren")?;
    click(&copy);
    assert!(receive.try_recv().is_err(), "default state must be inert");
    let state = WorkspaceState {
        status: "Bereit".into(),
        transcript: "Erster Text".into(),
        hotkey: "⌥D".into(),
        can_copy: true,
        can_clear: true,
        can_settings: true,
        ..WorkspaceState::default()
    };
    workspace.update(state.clone())?;
    assert_eq!(text.string().to_string(), "Erster Text");
    text.setSelectedRange(NSRange::new(2, 3));
    workspace.update(WorkspaceState {
        recording: Some(RecordingReadout {
            elapsed_seconds: 61,
            level_permille: 250,
        }),
        ..state.clone()
    })?;
    assert_eq!(
        text.selectedRange(),
        NSRange::new(2, 3),
        "telemetry must preserve native selection"
    );
    let copy_key = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0., 0.), NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        0., window.windowNumber(), None, &NSString::from_str("C"), &NSString::from_str("C"), false, 0,
    ).ok_or("owned copy key event")?;
    assert!(
        window.performKeyEquivalent(&copy_key),
        "copy must be reachable through its native key equivalent"
    );
    assert_eq!(
        receive.try_recv()?,
        WorkspaceAction::Copy {
            text: "Erster Text".into()
        }
    );
    click(&copy);
    click(&*button(&body, "Löschen")?);
    click(&*button(&body, "Berechtigungen prüfen")?);
    // The callback can be handled after the view has advanced to a new result.
    workspace.update(WorkspaceState {
        transcript: "Replacement B".into(),
        ..state.clone()
    })?;
    assert_eq!(
        receive.try_recv()?,
        WorkspaceAction::Copy {
            text: "Erster Text".into()
        }
    );
    assert_eq!(
        receive.try_recv()?,
        WorkspaceAction::Clear {
            expected_text: "Erster Text".into()
        }
    );
    assert_eq!(receive.try_recv()?, WorkspaceAction::Settings);
    workspace.update(state.clone())?;
    assert!(
        body.subviews()
            .iter()
            .filter_map(|value| value.downcast::<NSTextField>().ok())
            .any(|value| value
                .stringValue()
                .to_string()
                .contains("⌥D startet und stoppt. Esc"))
    );
    let setup_info =
        "large-v3-turbo · 574 MB · Hugging Face. Einmal laden, danach lokal und offline diktieren.";
    let permission_info = "Mikrofon für Aufnahme, Bedienungshilfen für automatisches Einfügen. Lokales Kopieren bleibt möglich.";
    workspace.update(WorkspaceState {
        status: "Modell einrichten".into(),
        setup_info: Some(setup_info.into()),
        permission_info: Some(permission_info.into()),
        can_download: true,
        can_settings: true,
        ..WorkspaceState::default()
    })?;
    let find_label = |expected: &str| {
        body.subviews()
            .iter()
            .filter_map(|value| value.downcast::<NSTextField>().ok())
            .find(|value| value.stringValue().to_string() == expected)
    };
    assert!(
        find_label(setup_info).is_some(),
        "model, size, source and offline context must be visible before download"
    );
    assert!(
        find_label(permission_info).is_some(),
        "microphone and accessibility requirements must remain separate"
    );
    let model_label = find_label(setup_info).ok_or("setup info")?;
    let permission_label = find_label(permission_info).ok_or("permission info")?;
    assert!(!model_label.isHidden() && !permission_label.isHidden());
    assert_eq!(model_label.frame().size.height, 60.0);
    assert_eq!(permission_label.frame().size.height, 40.0);
    let download_button = button(&body, "Modell laden")?;
    assert!(
        permission_label.frame().origin.y + permission_label.frame().size.height
            <= download_button.frame().origin.y
    );
    for width in [560.0, 760.0] {
        window.setContentSize(NSSize::new(width, 520.0));
        root.layoutSubtreeIfNeeded();
        assert!(model_label.frame().origin.x + model_label.frame().size.width <= width);
        assert!(scroll.frame().origin.y + scroll.frame().size.height <= body.bounds().size.height);
    }
    workspace.update(WorkspaceState {
        status: "Download".into(),
        download: Some(DownloadReadout {
            received: 5,
            total: Some(10),
        }),
        can_cancel_setup: true,
        ..WorkspaceState::default()
    })?;
    assert!(
        model_label.isHidden() && permission_label.isHidden(),
        "absent context must release its layout space"
    );
    click(&*button(&body, "Einrichtung abbrechen")?);
    assert_eq!(receive.try_recv()?, WorkspaceAction::CancelSetup);
    workspace.update(WorkspaceState {
        status: "Fehlgeschlagen".into(),
        can_download: true,
        ..WorkspaceState::default()
    })?;
    click(&*button(&body, "Modell laden")?);
    assert_eq!(receive.try_recv()?, WorkspaceAction::Download);
    for width in [560.0, 760.0] {
        window.setContentSize(NSSize::new(width, 520.0));
        root.layoutSubtreeIfNeeded();
        let actual = body.convertRect_toView(body.bounds(), None);
        let expected = window.contentLayoutRect();
        assert!(
            (actual.size.width - expected.size.width).abs() < 0.5
                && (actual.size.height - expected.size.height).abs() < 0.5
        );
        assert!(scroll.frame().size.height >= 48.0);
        for child in body.subviews() {
            if !child.isHidden() {
                let frame = child.frame();
                assert!(
                    frame.origin.x >= 0.0 && frame.origin.x + frame.size.width <= width,
                    "native controls must fit"
                );
            }
        }
    }
    assert_eq!(
        workspace.update(WorkspaceState {
            transcript: "x".repeat(262145),
            ..state.clone()
        }),
        Err(WorkspaceError::State)
    );
    assert_eq!(
        workspace.update(WorkspaceState {
            setup_info: Some("x".repeat(2049)),
            ..WorkspaceState::default()
        }),
        Err(WorkspaceError::State)
    );
    assert_eq!(
        workspace.update(WorkspaceState {
            permission_info: Some("invalid\0info".into()),
            ..WorkspaceState::default()
        }),
        Err(WorkspaceError::State)
    );
    assert_eq!(text.string().to_string(), "");
    workspace.update(WorkspaceState {
        recovery: Some("Beschädigte Einstellungen".into()),
        ..WorkspaceState::default()
    })?;
    assert!(button(&body, "Einstellungen zurücksetzen …")?.isEnabled());
    assert!(!copy.isEnabled());
    // Do not invoke the genuine NSAlert: confirmation gates are tested in the
    // module without presenting UI. This executable never resets preferences.
    workspace.detach()?;
    assert!(!original.isHidden());
    assert_eq!(window.firstResponder().as_deref(), Some(before.as_ref()));
    assert_eq!(root.subviews().len(), 1);
    click(&copy);
    assert!(receive.try_recv().is_err());
    assert_eq!(workspace.update(state), Err(WorkspaceError::Inactive));
    drop(workspace);
    original.setHidden(true);
    drop(NativeWorkspace::attach(owner.clone(), |_| {})?);
    assert!(
        original.isHidden(),
        "preexisting visibility must be restored"
    );
    original.setHidden(false);
    let mut changed = NativeWorkspace::attach(owner.clone(), |_| {})?;
    let changed_body = root.subviews().lastObject().ok_or("changed body")?;
    changed_body.removeFromSuperview();
    assert_eq!(
        changed.update(WorkspaceState::default()),
        Err(WorkspaceError::Changed)
    );
    drop(changed);
    assert!(
        !original.isHidden(),
        "cleanup restores retained webview after foreign hierarchy change"
    );
    let mut panicking = NativeWorkspace::attach(owner, |_| {
        std::panic::resume_unwind(Box::new("controlled callback"))
    })?;
    panicking.update(WorkspaceState {
        transcript: "native test".into(),
        can_copy: true,
        ..WorkspaceState::default()
    })?;
    let panic_body = root.subviews().lastObject().ok_or("panic body")?;
    click(&*button(&panic_body, "Kopieren")?);
    assert_eq!(
        panicking.update(WorkspaceState::default()),
        Err(WorkspaceError::Inactive)
    );
    panicking.detach()?;
    assert!(!original.isHidden());
    assert!(!window.isVisible());
    assert_eq!(app.isActive(), active);
    window.close();
    println!("native workspace contract passed; no visible window/clipboard/audio");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| {
        full_size_header_contract()?;
        run()
    })
}
