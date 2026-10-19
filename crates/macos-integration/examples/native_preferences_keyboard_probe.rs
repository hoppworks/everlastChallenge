//! Own Regular-App lifecycle probe; no audio, AX queries of other apps, or OS settings writes.
#![allow(
    unsafe_code,
    reason = "Own AppKit delegate, menu and timer selectors on the main thread"
)]
use macos_integration::native_preferences::{
    NativePreferences, PreferenceChoice, PreferencesDraft, PreferencesPermission, PreferencesState,
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSEvent,
    NSEventMask, NSMenu, NSMenuItem, NSResponder, NSTabView, NSView, NSWindowWillCloseNotification,
    NSWorkspace,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSRunLoop,
    NSRunLoopCommonModes, NSString, NSTimer,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path},
    ptr::NonNull,
    rc::Rc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
const MAX_SAMPLES: u32 = 1200;
const MAX_ROW_BYTES: usize = 65536;
const REPORT_ENV: &str = "EVERLAST_PREFERENCES_PROBE_REPORT_DIR";
struct Report {
    file: File,
    directory: std::path::PathBuf,
    identity: (u64, u64),
    file_identity: (u64, u64),
}
fn id(meta: &fs::Metadata) -> (u64, u64) {
    (meta.dev(), meta.ino())
}
impl Report {
    fn create(path: &Path) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|p| matches!(p, Component::ParentDir | Component::CurDir))
            || path
                .parent()
                .ok_or_else(|| io::Error::other("missing parent"))?
                .canonicalize()?
                != path
                    .parent()
                    .ok_or_else(|| io::Error::other("missing parent"))?
        {
            return Err(io::Error::other("canonical absolute report path required"));
        }
        DirBuilder::new().mode(0o700).create(path)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.join("observations.jsonl"))?;
        Ok(Self {
            identity: id(&fs::symlink_metadata(path)?),
            file_identity: id(&file.metadata()?),
            file,
            directory: path.into(),
        })
    }
    fn write(&mut self, value: &Value) -> io::Result<()> {
        let directory = fs::symlink_metadata(&self.directory)?;
        let current = fs::symlink_metadata(self.directory.join("observations.jsonl"))?;
        let opened = self.file.metadata()?;
        if !directory.is_dir()
            || id(&directory) != self.identity
            || directory.mode() & 0o777 != 0o700
            || directory.uid() != opened.uid()
            || !current.is_file()
            || id(&current) != self.file_identity
            || id(&opened) != self.file_identity
            || current.nlink() != 1
            || opened.nlink() != 1
            || current.mode() & 0o777 != 0o600
            || opened.mode() & 0o777 != 0o600
        {
            return Err(io::Error::other("report ownership changed"));
        }
        let mut row = serde_json::to_vec(value).map_err(io::Error::other)?;
        if row.len() > MAX_ROW_BYTES {
            return Err(io::Error::other("report bound exceeded"));
        }
        row.push(b'\n');
        self.file.write_all(&row)?;
        self.file.flush()
    }
}
fn controlled_state() -> PreferencesState {
    PreferencesState {
        current: PreferencesDraft {
            device: None,
            language: "german".into(),
            key: 'D',
            option: true,
            command: false,
            control: false,
            shift: false,
        },
        devices: vec![PreferenceChoice {
            value: "controlled-device".into(),
            label: "Kontrolliertes Testgerät".into(),
            enabled: true,
        }],
        languages: vec![
            PreferenceChoice {
                value: "german".into(),
                label: "Deutsch".into(),
                enabled: true,
            },
            PreferenceChoice {
                value: "english".into(),
                label: "Englisch".into(),
                enabled: true,
            },
        ],
        key_labels: ('A'..='Z').map(|key| (key, key.to_string())).collect(),
        busy: false,
        permissions_busy: false,
        microphone: PreferencesPermission::Granted,
        accessibility: PreferencesPermission::Granted,
        model_summary: "Kontrollierter Testzustand".into(),
        model_details: "Keine Modell- oder Geräteoperationen".into(),
        error: None,
    }
}
fn responder_type(value: Option<Retained<NSResponder>>) -> Option<String> {
    value.map(|r| r.class().name().to_string_lossy().into_owned())
}
fn view_metadata(view: &NSView, index: usize) -> Value {
    json!({"index":index,"class":view.class().name().to_string_lossy(),"accessibilityLabel":view.accessibilityLabel().map(|s|s.to_string()),"accessibilityFocused":view.isAccessibilityFocused(),"acceptsFirstResponder":view.acceptsFirstResponder(),"canBecomeKeyView":view.canBecomeKeyView(),"hidden":view.isHiddenOrHasHiddenAncestor(),"focusRingType":view.focusRingType().0})
}
fn collect(view: &NSView, out: &mut Vec<Retained<NSView>>) {
    if out.len() >= 96 {
        return;
    }
    out.push(view.retain());
    for child in view.subviews() {
        collect(&child, out);
    }
}
#[derive(Default)]
struct KeyCounts {
    total: u32,
    overflow: u32,
    by_code_and_modifiers: BTreeMap<(u16, u64), u32>,
}
impl KeyCounts {
    fn observe(&mut self, code: u16, modifiers: u64) {
        self.total = self.total.saturating_add(1);
        if let Some(count) = self.by_code_and_modifiers.get_mut(&(code, modifiers)) {
            *count = count.saturating_add(1);
        } else if self.by_code_and_modifiers.len() < 32 {
            self.by_code_and_modifiers.insert((code, modifiers), 1);
        } else {
            self.overflow = self.overflow.saturating_add(1);
        }
    }
    fn snapshot(&self) -> Value {
        json!({"total":self.total,"overflow":self.overflow,"maximumDistinct":32,"codesAndModifiers":self.by_code_and_modifiers.iter().map(|((key,modifiers),count)|json!({"keyCode":key,"modifierFlags":modifiers,"count":count})).collect::<Vec<_>>()})
    }
}
struct State {
    preferences: NativePreferences,
    keys: Rc<RefCell<KeyCounts>>,
    show_calls: u32,
    cmd_comma_actions: u32,
    close_notifications: u32,
    report: Report,
    started: Instant,
    sequence: u32,
    callbacks: std::sync::Arc<std::sync::atomic::AtomicU32>,
    timer: Option<Retained<NSTimer>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<State>]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn launched(&self,_:&NSNotification) {
            let main=MainThreadMarker::from(self);
            self.ivars().borrow_mut().show_calls+=1;
            if self.ivars().borrow().preferences.show().is_err() { NSApplication::sharedApplication(main).terminate(None); return; }
            if let Some(window)=NSApplication::sharedApplication(main).windows().iter().find(|w|w.title().to_string()=="Einstellungen") {
                // SAFETY: passive notification scoped to the retained own preferences window.
                unsafe {NSNotificationCenter::defaultCenter().addObserver_selector_name_object(self,sel!(ownWindowClosed:),Some(NSWindowWillCloseNotification),Some(&window));}
            }
            // SAFETY: retained own delegate and selector; timer invalidated before teardown.
            let timer=unsafe { NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(0.25,self,sel!(observe:),None,true) };
            unsafe {NSRunLoop::mainRunLoop().addTimer_forMode(&timer,NSRunLoopCommonModes);}
            self.ivars().borrow_mut().timer=Some(timer);
        }
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn keep_alive(&self,_:&NSApplication)->bool { false }
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn reopen(&self,_:&NSApplication,_:bool)->bool { self.ivars().borrow_mut().show_calls+=1; let _result=self.ivars().borrow().preferences.show(); true }
    }
    impl Delegate {
        #[unsafe(method(openPreferences:))]
        fn open_preferences(&self,_:Option<&NSObject>) { {let mut state=self.ivars().borrow_mut();state.show_calls+=1;state.cmd_comma_actions+=1;} let _result=self.ivars().borrow().preferences.show(); }
        #[unsafe(method(ownWindowClosed:))]
        fn own_closed(&self,_:&NSNotification) { self.ivars().borrow_mut().close_notifications+=1; }
        #[unsafe(method(observe:))]
        fn observe(&self,_:&NSTimer) {
            let main=MainThreadMarker::from(self);
            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||self.capture(main)));
            if !matches!(result,Ok(Ok(()))) { NSApplication::sharedApplication(main).terminate(None); }
        }
    }
);
impl Delegate {
    fn capture(&self, main: MainThreadMarker) -> io::Result<()> {
        let app = NSApplication::sharedApplication(main);
        let mut state = self.ivars().borrow_mut();
        if state.started.elapsed().as_secs() >= 300 || state.sequence >= MAX_SAMPLES {
            if let Some(timer) = &state.timer {
                timer.invalidate();
            }
            app.terminate(None);
            return Ok(());
        }
        let window = app
            .windows()
            .iter()
            .find(|w| w.title().to_string() == "Einstellungen")
            .ok_or_else(|| io::Error::other("own preferences missing"))?;
        let mut controls = Vec::new();
        if let Some(root) = window.contentView() {
            collect(&root, &mut controls);
        }
        let focused = window.firstResponder();
        let focused_index = focused
            .as_ref()
            .and_then(|r| r.downcast_ref::<NSView>())
            .and_then(|r| controls.iter().position(|v| &**v == r));
        let mut chain = Vec::new();
        let mut next = focused
            .as_ref()
            .and_then(|r| r.downcast_ref::<NSView>())
            .map(NSView::retain);
        for _ in 0..16 {
            let Some(view) = next else { break };
            let index = controls.iter().position(|v| **v == *view);
            if chain.contains(&index) {
                break;
            }
            chain.push(index);
            // SAFETY: readonly key-view traversal on our own main-thread view.
            next = unsafe { view.nextValidKeyView() };
        }
        let tab = controls
            .iter()
            .find_map(|v| v.downcast_ref::<NSTabView>())
            .and_then(|v| v.selectedTabViewItem())
            .map(|i| i.label().to_string());
        state.sequence += 1;
        let row = json!({"schemaVersion":1,"kind":"everlast-own-preferences-keyboard-probe-v2","sequence":state.sequence,"processId":std::process::id(),"observedUnixMillis":SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_millis(),"elapsedMillis":state.started.elapsed().as_millis(),"applicationActive":app.isActive(),"frontmostIsSelf":NSWorkspace::sharedWorkspace().frontmostApplication().is_some_and(|a|a.processIdentifier()==std::process::id() as i32),"windowVisible":window.isVisible(),"windowKey":window.isKeyWindow(),"fullKeyboardAccess":app.isFullKeyboardAccessEnabled(),"initialResponderClass":window.initialFirstResponder().map(|r|r.class().name().to_string_lossy().into_owned()),"firstResponderClass":responder_type(focused),"focusedControlIndex":focused_index,"selectedTab":tab,"nextValidKeyViewIndices":chain,"controls":controls.iter().enumerate().map(|(i,v)|view_metadata(v,i)).collect::<Vec<_>>(),"callbackCount":state.callbacks.load(std::sync::atomic::Ordering::Relaxed),"localKeyDown":state.keys.borrow().snapshot(),"showCalls":state.show_calls,"cmdCommaActionCount":state.cmd_comma_actions,"closeNotificationCount":state.close_notifications,"controlledState":true,"visualFocusRingConfirmed":false});
        state.report.write(&row)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let report = Report::create(Path::new(
        &std::env::var_os(REPORT_ENV).ok_or("explicit report path required")?,
    ))?;
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // Product callbacks require Send+Sync. Atomic count records no drafts or external effect.
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let callback_counter = counter.clone();
    let mut preferences = NativePreferences::new(move |_| {
        callback_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    })?;
    preferences.update(controlled_state())?;
    let keys = Rc::new(RefCell::new(KeyCounts::default()));
    let monitor_keys = keys.clone();
    let monitor_block = block2::RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit supplies the live event for this local callback. The
        // identical pointer is returned; no event is consumed, changed or dispatched.
        let own_event = unsafe { event.as_ref() };
        if let Ok(mut counts) = monitor_keys.try_borrow_mut() {
            counts.observe(own_event.keyCode(), own_event.modifierFlags().0 as u64);
        }
        event.as_ptr()
    });
    // SAFETY: the block returns precisely AppKit's incoming live event pointer.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &monitor_block)
    }
    .ok_or("local monitor unavailable")?;
    let delegate = Delegate::alloc(main).set_ivars(RefCell::new(State {
        preferences,
        keys,
        show_calls: 0,
        cmd_comma_actions: 0,
        close_notifications: 0,
        report,
        started: Instant::now(),
        sequence: 0,
        callbacks: counter,
        timer: None,
    }));
    let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
    let menu = NSMenu::new(main);
    let item = NSMenuItem::new(main);
    menu.addItem(&item);
    let submenu = NSMenu::new(main);
    item.setSubmenu(Some(&submenu));
    for (title, key, action) in [
        ("Einstellungen …", ",", sel!(openPreferences:)),
        ("Schließen", "w", sel!(performClose:)),
        ("Beenden", "q", sel!(terminate:)),
    ] {
        let command = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(main),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            )
        };
        if key == "," {
            unsafe {
                command.setTarget(Some(&delegate));
            }
        }
        submenu.addItem(&command);
    }
    app.setMainMenu(Some(&menu));
    app.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*delegate)));
    app.run();
    // SAFETY: exact token returned by our local monitor registration.
    unsafe {
        NSEvent::removeMonitor(&monitor);
        NSNotificationCenter::defaultCenter().removeObserver(&delegate);
    }
    if let Some(timer) = delegate.ivars().borrow_mut().timer.take() {
        timer.invalidate();
    }
    delegate.ivars().borrow_mut().preferences.detach()?;
    app.setDelegate(None);
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_counts_are_numeric_bounded_and_do_not_store_characters() {
        let mut counts = KeyCounts::default();
        counts.observe(48, 0);
        counts.observe(48, 0);
        assert_eq!(counts.by_code_and_modifiers[&(48, 0)], 2);
        for key in 100..140 {
            counts.observe(key, 1);
        }
        assert_eq!(counts.by_code_and_modifiers.len(), 32);
        assert_eq!(counts.overflow, 9);
        assert_eq!(counts.total, 42);
        assert!(counts.snapshot().get("characters").is_none());
    }
    #[test]
    fn explicit_report_rejects_relative_or_existing_directory() {
        assert!(Report::create(Path::new("relative")).is_err());
        assert!(Report::create(&std::env::temp_dir()).is_err());
    }
}
