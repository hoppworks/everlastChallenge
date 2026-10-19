//! Ephemeral native recipient; opt-in own-state report, never AX or raw-text logging.
#![allow(
    unsafe_code,
    reason = "Public AppKit construction and standard target/action selectors on the main thread"
)]
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSBorderType, NSMenu, NSMenuItem, NSResponder, NSScrollView, NSTextField, NSTextView, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes,
    NSSize, NSString, NSTimer,
};

use std::{
    cell::RefCell,
    fs::{self, DirBuilder, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Component, Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const EXPECTED: &str = "Forschung im Bereich KI umfasst die Herstellung von Maschinen zur Automatisierung von Aufgaben, die intelligentes Verhalten erfordern.";
const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_REPORTS: u32 = 7_200; // At most 30 minutes at the nominal 250 ms cadence.
const OWNER: &[u8] = b"everlast-own-native-recipient-status-v1\n";
#[derive(Debug)]
struct Snapshot {
    observed_unix_millis: u128,
    monotonic_since_launch_nanos: u128,
    process_id: u32,
    application_active: bool,
    window_key: bool,
    focused_field: &'static str,
    expected_sentence_matches: bool,
    expected_sentence_occurrences: usize,
    shortcut_character_present: bool,
    running: bool,
    status: &'static str,
}
struct ReportDirectory {
    path: PathBuf,
    identity: (u64, u64),
    owner_identity: (u64, u64),
    snapshot_identity: Option<(u64, u64)>,
}
fn identity(meta: &Metadata) -> (u64, u64) {
    (meta.dev(), meta.ino())
}
impl ReportDirectory {
    fn create(path: &Path) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(io::Error::other(
                "report directory must be absolute and canonical",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("report parent missing"))?;
        if parent.canonicalize()? != parent {
            return Err(io::Error::other("symlinked report parent rejected"));
        }
        DirBuilder::new().mode(0o700).create(path)?;
        let mut owner = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("owner"))?;
        owner.write_all(OWNER)?;
        owner.sync_all()?;
        Ok(Self {
            path: path.into(),
            identity: identity(&fs::symlink_metadata(path)?),
            owner_identity: identity(&owner.metadata()?),
            snapshot_identity: None,
        })
    }
    fn owner_matches(&self) -> io::Result<bool> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path.join("owner"))?;
        if identity(&file.metadata()?) != self.owner_identity {
            return Ok(false);
        }
        let mut bytes = [0_u8; OWNER.len()];
        file.read_exact(&mut bytes)?;
        Ok(bytes == OWNER)
    }
    fn verify(&self) -> io::Result<()> {
        let directory = fs::symlink_metadata(&self.path)?;
        let owner = fs::symlink_metadata(self.path.join("owner"))?;
        if !directory.is_dir()
            || directory.mode() & 0o777 != 0o700
            || identity(&directory) != self.identity
            || !owner.is_file()
            || owner.len() != OWNER.len() as u64
            || owner.nlink() != 1
            || identity(&owner) != self.owner_identity
            || owner.mode() & 0o777 != 0o600
            || !self.owner_matches()?
        {
            return Err(io::Error::other("report owner changed"));
        }
        match (
            self.snapshot_identity,
            fs::symlink_metadata(self.path.join("snapshot.json")),
        ) {
            (None, Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            (Some(expected), Ok(meta))
                if meta.is_file()
                    && meta.nlink() == 1
                    && meta.mode() & 0o777 == 0o600
                    && identity(&meta) == expected =>
            {
                Ok(())
            }
            _ => Err(io::Error::other("report target changed")),
        }
    }
    fn publish(&mut self, snapshot: &Snapshot, sequence: u32) -> io::Result<()> {
        if sequence == 0
            || sequence > MAX_REPORTS + 1
            || !matches!(snapshot.focused_field, "multiline" | "singleline" | "none")
            || !matches!(
                snapshot.status,
                "observed" | "stopped" | "observation-limit-reached" | "input-too-large"
            )
        {
            return Err(io::Error::other("invalid bounded report"));
        }
        self.verify()?;
        let bytes = format!(
            concat!(
                "{{\"schemaVersion\":1,\"kind\":\"everlast-own-native-recipient-status\",",
                "\"sequence\":{},\"observedUnixMillis\":{},\"monotonicSinceLaunchNanos\":{},\"processId\":{},",
                "\"applicationActive\":{},\"windowKey\":{},\"focusedField\":\"{}\",",
                "\"expectedSentenceMatches\":{},\"expectedSentenceOccurrences\":{},\"shortcutCharacterPresent\":{},",
                "\"running\":{},\"status\":\"{}\",\"publicExpectedSentenceId\":\"fleurs-de-dev-1517\",",
                "\"scope\":\"own-window-only; not foreign target capture or delivery proof\"}}\n"
            ),
            sequence,
            snapshot.observed_unix_millis,
            snapshot.monotonic_since_launch_nanos,
            snapshot.process_id,
            snapshot.application_active,
            snapshot.window_key,
            snapshot.focused_field,
            snapshot.expected_sentence_matches,
            snapshot.expected_sentence_occurrences,
            snapshot.shortcut_character_present,
            snapshot.running,
            snapshot.status
        );
        let pending = self.path.join("pending.json");
        let target = self.path.join("snapshot.json");
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)?;
        output.write_all(bytes.as_bytes())?;
        output.sync_all()?;
        drop(output);
        self.verify()?;
        if self.snapshot_identity.is_none() {
            fs::hard_link(&pending, &target)?;
            fs::remove_file(&pending)?;
        } else {
            fs::rename(&pending, &target)?;
        }
        self.snapshot_identity = Some(identity(&fs::symlink_metadata(&target)?));
        Ok(())
    }
}
fn configured_report(path: Option<PathBuf>) -> io::Result<Option<ReportDirectory>> {
    path.as_deref().map(ReportDirectory::create).transpose()
}

fn text_metrics(text: &str) -> io::Result<(bool, usize, bool)> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(io::Error::other("own text exceeds observation bound"));
    }
    Ok((
        text == EXPECTED,
        text.match_indices(EXPECTED).count(),
        text.contains('∂'),
    ))
}
struct Reporting {
    app: Retained<NSApplication>,
    window: Retained<NSWindow>,
    multiline: Retained<NSTextView>,
    singleline: Retained<NSTextField>,
    directory: ReportDirectory,
    started: Instant,
    sequence: u32,
    stopped: bool,
}
impl Reporting {
    fn capture(&self, running: bool, status: &'static str) -> io::Result<Snapshot> {
        let responder = self.window.firstResponder();
        let same = |object: &NSResponder| {
            responder
                .as_ref()
                .is_some_and(|r| std::ptr::eq::<NSResponder>(&**r, object))
        };
        let focused = if same(&self.multiline) {
            "multiline"
        } else if self
            .singleline
            .currentEditor()
            .as_ref()
            .is_some_and(|editor| same(editor))
        {
            "singleline"
        } else {
            "none"
        };
        let multiline = self.multiline.string();
        let singleline = self
            .singleline
            .currentEditor()
            .map(|editor| editor.string())
            .unwrap_or_else(|| self.singleline.stringValue());
        let bounded = multiline.length() <= MAX_TEXT_BYTES && singleline.length() <= MAX_TEXT_BYTES;
        let (matches, occurrences, shortcut, status) = if bounded {
            let a = multiline.to_string();
            let b = singleline.to_string();
            match (text_metrics(&a), text_metrics(&b)) {
                (Ok(am), Ok(bm)) => {
                    let chosen = match focused {
                        "multiline" => am,
                        "singleline" => bm,
                        _ => (false, 0, false),
                    };
                    (chosen.0, chosen.1, am.2 || bm.2, status)
                }
                _ => (false, 0, false, "input-too-large"),
            }
        } else {
            (false, 0, false, "input-too-large")
        };
        Ok(Snapshot {
            observed_unix_millis: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_millis(),
            monotonic_since_launch_nanos: self.started.elapsed().as_nanos(),
            process_id: std::process::id(),
            application_active: self.app.isActive(),
            window_key: self.window.isKeyWindow(),
            focused_field: focused,
            expected_sentence_matches: matches,
            expected_sentence_occurrences: occurrences,
            shortcut_character_present: shortcut,
            running,
            status,
        })
    }
    fn publish(&mut self, stopping: bool) -> io::Result<bool> {
        if self.stopped && !stopping {
            return Ok(false);
        }
        if self.sequence > MAX_REPORTS {
            return Ok(false);
        }
        self.sequence += 1;
        let status = if stopping {
            "stopped"
        } else if self.sequence >= MAX_REPORTS {
            "observation-limit-reached"
        } else {
            "observed"
        };
        let snapshot = self.capture(!stopping, status)?;
        self.directory.publish(&snapshot, self.sequence)?;
        self.stopped = stopping || self.sequence >= MAX_REPORTS;
        Ok(!self.stopped)
    }
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Reporting>]
    struct Reporter;
    unsafe impl NSObjectProtocol for Reporter {}
    impl Reporter {
        #[unsafe(method(tick:))]
        fn tick(&self, timer: &NSTimer) {
            let result=catch_unwind(AssertUnwindSafe(|| self.ivars().borrow_mut().publish(false)));
            if !matches!(result, Ok(Ok(true))) {
                timer.invalidate();
                if !matches!(result, Ok(Ok(false))) { eprintln!("Own-recipient reporting stopped after an observation/export error"); }
            }
        }
    }
);
impl Reporter {
    fn stop(&self) {
        if !matches!(
            catch_unwind(AssertUnwindSafe(|| self.ivars().borrow_mut().publish(true))),
            Ok(Ok(_))
        ) {
            eprintln!("Own-recipient final report unavailable");
        }
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Option<Retained<Reporter>>>]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn close_last(&self, _app: &NSApplication) -> bool {
            true
        }
        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _: &NSNotification) {
            let result = catch_unwind(AssertUnwindSafe(|| {
                if let Some(reporter) = self.ivars().borrow().as_ref() { reporter.stop(); }
            }));
            if result.is_err() { eprintln!("Own-recipient final observation failed"); }
        }
    }
);
fn label(main: MainThreadMarker, text: &str, frame: NSRect) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), main);
    label.setFrame(frame);
    label
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let report_path = std::env::var_os("EVERLAST_RECIPIENT_REPORT_DIR").map(PathBuf::from);
    let directory = configured_report(report_path)?;
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // SAFETY: NSObject's designated initializer on an allocated main-thread subclass.
    let allocated = Delegate::alloc(main).set_ivars(RefCell::new(None));
    let delegate: Retained<Delegate> = unsafe { msg_send![super(allocated), init] };
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
    let timer = if let Some(directory) = directory {
        let state = Reporting {
            app: app.clone(),
            window: window.clone(),
            multiline: text.clone(),
            singleline: line.clone(),
            directory,
            started,
            sequence: 0,
            stopped: false,
        };
        // SAFETY: initialize the declared NSObject subclass on its main thread.
        let allocated = Reporter::alloc(main).set_ivars(RefCell::new(state));
        let reporter: Retained<Reporter> = unsafe { msg_send![super(allocated), init] };
        *delegate.ivars().borrow_mut() = Some(reporter.clone());
        // SAFETY: Reporter implements tick: with exactly NSTimer's public target/action signature;
        // the timer retains its target and is scheduled exclusively on the main run loop.
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                0.25,
                &reporter,
                sel!(tick:),
                None,
                true,
            )
        };
        // SAFETY: public immutable common-mode constant, with timer and main-loop lifetimes owned here.
        unsafe {
            NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
        }
        Some(timer)
    } else {
        None
    };
    window.makeKeyAndOrderFront(None);
    app.run();
    if let Some(timer) = timer {
        timer.invalidate();
    }
    if let Some(reporter) = delegate.ivars().borrow().as_ref() {
        reporter.stop();
    }
    // Keep delegate and every native owner alive until the ordinary run loop ends.
    app.setDelegate(None);
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> io::Result<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let p = std::env::temp_dir().canonicalize()?.join(format!(
                "everlast-recipient-report-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(io::Error::other)?
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&p)?;
            Ok(Self(p))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _cleanup = fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot {
            observed_unix_millis: 1_800_000_000_001,
            monotonic_since_launch_nanos: 31,
            process_id: 123,
            application_active: true,
            window_key: true,
            focused_field: "multiline",
            expected_sentence_matches: true,
            expected_sentence_occurrences: 1,
            shortcut_character_present: false,
            running: true,
            status: "observed",
        }
    }
    #[test]
    fn explicit_report_is_private_fresh_and_replaces_only_its_own_snapshot()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Temp::new()?;
        assert!(configured_report(None)?.is_none());
        assert_eq!(fs::read_dir(&root.0)?.count(), 0);
        let path = root.0.join("reports");
        let mut report = ReportDirectory::create(&path)?;
        report.publish(&snapshot(), 1)?;
        let first = fs::read_to_string(path.join("snapshot.json"))?;
        assert!(
            first.contains("\"sequence\":1"),
            "explicit report did not publish its first snapshot"
        );
        assert_eq!(fs::metadata(&path)?.mode() & 0o777, 0o700);
        assert_eq!(
            fs::metadata(path.join("snapshot.json"))?.mode() & 0o777,
            0o600
        );
        report.publish(
            &Snapshot {
                running: false,
                ..snapshot()
            },
            2,
        )?;
        let last = fs::read(path.join("snapshot.json"))?;
        assert!(String::from_utf8(last.clone())?.contains("\"running\":false"));
        assert!(ReportDirectory::create(&path).is_err());
        fs::rename(path.join("snapshot.json"), path.join("old.json"))?;
        fs::write(path.join("snapshot.json"), b"foreign")?;
        assert!(report.publish(&snapshot(), 3).is_err());
        assert_eq!(fs::read(path.join("snapshot.json"))?, b"foreign");
        Ok(())
    }

    #[test]
    fn directory_collision_and_symlink_parent_never_claim_foreign_outputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Temp::new()?;
        let existing = root.0.join("existing");
        fs::create_dir(&existing)?;
        fs::write(existing.join("keep"), b"original")?;
        assert!(ReportDirectory::create(&existing).is_err());
        assert_eq!(fs::read(existing.join("keep"))?, b"original");
        assert!(ReportDirectory::create(Path::new("relative-reports")).is_err());
        let alias = root.0.join("alias");
        std::os::unix::fs::symlink(&existing, &alias)?;
        assert!(ReportDirectory::create(&alias.join("reports")).is_err());
        assert!(!existing.join("reports").exists());
        Ok(())
    }
    #[test]
    fn expected_metrics_distinguish_exact_duplicate_and_shortcut_without_exporting_text()
    -> io::Result<()> {
        assert_eq!(text_metrics(EXPECTED)?, (true, 1, false));
        assert_eq!(
            text_metrics(&format!("{EXPECTED} {EXPECTED}"))?,
            (false, 2, false)
        );
        assert_eq!(text_metrics("∂")?, (false, 0, true));
        assert_eq!(text_metrics("different test input")?, (false, 0, false));
        assert!(text_metrics(&"a".repeat(MAX_TEXT_BYTES + 1)).is_err());
        Ok(())
    }
    #[test]
    fn observation_limit_rejects_additional_writes() -> Result<(), Box<dyn std::error::Error>> {
        let root = Temp::new()?;
        let mut report = ReportDirectory::create(&root.0.join("reports"))?;
        assert!(report.publish(&snapshot(), MAX_REPORTS + 2).is_err());
        assert!(!report.path.join("snapshot.json").exists());
        Ok(())
    }
}
