//! Ephemeral native test recipient: no AX queries, files, network or text logging.
#![allow(
    unsafe_code,
    reason = "Public AppKit construction and standard target/action selectors on the main thread"
)]
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSBorderType, NSMenu, NSMenuItem, NSScrollView, NSTextField, NSTextView, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn close_last(&self, _app: &NSApplication) -> bool {
            true
        }
    }
);
fn label(main: MainThreadMarker, text: &str, frame: NSRect) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), main);
    label.setFrame(frame);
    label
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // SAFETY: NSObject's designated initializer on an allocated main-thread subclass.
    let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(main), init] };
    app.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*delegate)));
    let menu = NSMenu::new(main);
    let app_item = NSMenuItem::new(main);
    menu.addItem(&app_item);
    let app_menu = NSMenu::new(main);
    // SAFETY: standard AppKit responder-chain selectors; no custom injection or target.
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            main.alloc(),
            &NSString::from_str("Everlast Testziel beenden"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    app_menu.addItem(&quit);
    app_item.setSubmenu(Some(&app_menu));
    let edit_item = NSMenuItem::new(main);
    menu.addItem(&edit_item);
    let edit_menu = NSMenu::initWithTitle(main.alloc(), &NSString::from_str("Bearbeiten"));
    for (title, action, key) in [
        ("Ausschneiden", sel!(cut:), "x"),
        ("Kopieren", sel!(copy:), "c"),
        ("Einfügen", sel!(paste:), "v"),
        ("Alles auswählen", sel!(selectAll:), "a"),
    ] {
        // SAFETY: public standard editing selectors, with nil target so AppKit
        // dispatches to the focused text responder and validates availability.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                main.alloc(),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            )
        };
        edit_menu.addItem(&item);
    }
    edit_item.setSubmenu(Some(&edit_menu));
    let window_item = NSMenuItem::new(main);
    menu.addItem(&window_item);
    let window_menu = NSMenu::initWithTitle(main.alloc(), &NSString::from_str("Fenster"));
    // SAFETY: standard responder-chain window close action.
    let close = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            main.alloc(),
            &NSString::from_str("Fenster schließen"),
            Some(sel!(performClose:)),
            &NSString::from_str("w"),
        )
    };
    window_menu.addItem(&close);
    window_item.setSubmenu(Some(&window_menu));
    app.setMainMenu(Some(&menu));
    app.setWindowsMenu(Some(&window_menu));
    // SAFETY: designated NSWindow initializer, main-thread-only owned local window.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(640.0, 400.0)),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: retained window lives through run; AppKit must not release it on close.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    window.setTitle(&NSString::from_str("Everlast Testziel"));
    let body = NSView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(640.0, 400.0)),
    );
    body.addSubview(&label(
        main,
        "Isoliertes lokales Testziel – keine Speicherung oder Übertragung.",
        NSRect::new(NSPoint::new(20.0, 365.0), NSSize::new(600.0, 22.0)),
    ));
    body.addSubview(&label(
        main,
        "Feld anklicken, dann Everlasts Kürzel zum Starten und Stoppen verwenden.",
        NSRect::new(NSPoint::new(20.0, 338.0), NSSize::new(600.0, 22.0)),
    ));
    body.addSubview(&label(
        main,
        "Testfeld (mehrzeilig)",
        NSRect::new(NSPoint::new(20.0, 305.0), NSSize::new(600.0, 22.0)),
    ));
    let scroll = NSScrollView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(20.0, 105.0), NSSize::new(600.0, 195.0)),
    );
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setHasVerticalScroller(true);
    let text = NSTextView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(580.0, 195.0)),
    );
    text.setEditable(true);
    text.setSelectable(true);
    text.setRichText(false);
    text.setVerticallyResizable(true);
    scroll.setDocumentView(Some(&text));
    body.addSubview(&scroll);
    body.addSubview(&label(
        main,
        "Testzeile",
        NSRect::new(NSPoint::new(20.0, 70.0), NSSize::new(600.0, 22.0)),
    ));
    let line = NSTextField::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(20.0, 32.0), NSSize::new(600.0, 30.0)),
    );
    line.setEditable(true);
    line.setSelectable(true);
    line.setBezeled(true);
    body.addSubview(&line);
    window.setContentView(Some(&body));
    window.center();
    window.makeFirstResponder(Some(&text));
    window.makeKeyAndOrderFront(None);
    app.run();
    // Keep delegate and every native owner alive until the ordinary run loop ends.
    app.setDelegate(None);
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
