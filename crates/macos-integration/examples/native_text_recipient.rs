//! Ephemeral native recipient; opt-in own-state report, never AX or raw-text logging.
#![allow(
    unsafe_code,
    reason = "Public AppKit construction and standard target/action selectors on the main thread"
)]
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSBackingStoreType, NSBorderType, NSMenu, NSMenuItem, NSResponder, NSScrollView, NSTextField,
    NSTextView, NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSBundle, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoop,
    NSRunLoopCommonModes, NSSize, NSString, NSTimer, NSURL,
};

use sha2::{Digest, Sha256};

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
    process_started_unix_millis: u128,
    field_utf8_bytes: usize,
    field_sha256: Option<String>,
    total_utf8_bytes: usize,
    signal_state: String,
    signal_run_id: Option<String>,
    application_active: bool,
    frontmost_is_self: bool,
    own_bundle_identifier: Option<String>,
    window_key: bool,
    focused_field: &'static str,
    accessibility_focused: bool,
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
        let bundle = match snapshot.own_bundle_identifier.as_deref() {
            Some(id)
                if !id.is_empty()
                    && id.len() <= 255
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) =>
            {
                format!("\"{id}\"")
            }
            Some(_) => return Err(io::Error::other("invalid bounded own bundle identifier")),
            None => "null".to_owned(),
        };
        let receipt_hash = match snapshot.field_sha256.as_deref() {
            Some(hash)
                if hash.len() == 64
                    && hash
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) =>
            {
                format!("\"{hash}\"")
            }
            None if snapshot.field_utf8_bytes == 0 => "null".into(),
            _ => return Err(io::Error::other("invalid bounded own text digest")),
        };
        if snapshot.field_utf8_bytes > MAX_TEXT_BYTES
            || snapshot.total_utf8_bytes > 2 * MAX_TEXT_BYTES
        {
            return Err(io::Error::other("invalid own text size"));
        }
        let signal_state =
            serde_json::to_string(&snapshot.signal_state).map_err(io::Error::other)?;
        let signal_run_id =
            serde_json::to_string(&snapshot.signal_run_id).map_err(io::Error::other)?;
        let bytes = format!(
            concat!(
                "{{\"schemaVersion\":1,\"kind\":\"everlast-own-native-recipient-status\",",
                "\"sequence\":{},\"observedUnixMillis\":{},\"monotonicSinceLaunchNanos\":{},\"processId\":{},",
                "\"processStartedUnixMillis\":{},\"fieldUtf8Bytes\":{},\"fieldSha256\":{},\"totalUtf8Bytes\":{},",
                "\"signalState\":{},\"signalRunId\":{},",
                "\"applicationActive\":{},\"frontmostIsSelf\":{},\"ownBundleIdentifier\":{},\"windowKey\":{},\"focusedField\":\"{}\",",
                "\"accessibilityFocused\":{},",
                "\"expectedSentenceMatches\":{},\"expectedSentenceOccurrences\":{},\"shortcutCharacterPresent\":{},",
                "\"running\":{},\"status\":\"{}\",\"publicExpectedSentenceId\":\"fleurs-de-dev-1517\",",
                "\"scope\":\"own-window-only; not foreign target capture or delivery proof\"}}\n"
            ),
            sequence,
            snapshot.observed_unix_millis,
            snapshot.monotonic_since_launch_nanos,
            snapshot.process_id,
            snapshot.process_started_unix_millis,
            snapshot.field_utf8_bytes,
            receipt_hash,
            snapshot.total_utf8_bytes,
            signal_state,
            signal_run_id,
            snapshot.application_active,
            snapshot.frontmost_is_self,
            bundle,
            snapshot.window_key,
            snapshot.focused_field,
            snapshot.accessibility_focused,
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

fn receipt_metrics(text: &str) -> io::Result<(usize, Option<String>)> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(io::Error::other("own text exceeds observation bound"));
    }
    Ok((
        text.len(),
        (!text.is_empty()).then(|| {
            (Sha256::digest(text.as_bytes()))
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        }),
    ))
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
#[derive(Clone, Copy)]
enum SignalCheckpoint {
    AfterOpen,
    AfterRead,
}
enum SignalReadError {
    Replaced,
    Invalid(io::Error),
}
impl From<io::Error> for SignalReadError {
    fn from(error: io::Error) -> Self {
        Self::Invalid(error)
    }
}
fn signal_metadata_valid(meta: &Metadata, owner: u32) -> bool {
    meta.is_file() && meta.uid() == owner && meta.mode() & 0o777 == 0o600 && meta.len() <= 1024
}
fn signal_descriptor_valid(meta: &Metadata, owner: u32) -> Result<(), SignalReadError> {
    if !signal_metadata_valid(meta, owner) || meta.nlink() > 1 {
        return Err(io::Error::other("invalid private signal file").into());
    }
    if meta.nlink() == 0 {
        return Err(SignalReadError::Replaced);
    }
    Ok(())
}
fn signal_read_attempt(
    directory: &ReportDirectory,
    attempt: usize,
    checkpoint: &mut impl FnMut(SignalCheckpoint, usize) -> io::Result<()>,
) -> Result<Vec<u8>, SignalReadError> {
    directory
        .verify()
        .map_err(|error| io::Error::other(format!("signal directory: {error}")))?;
    let owner = fs::symlink_metadata(&directory.path)?.uid();
    let path = directory.path.join("signal.json");
    let before = fs::symlink_metadata(&path)?;
    if !signal_metadata_valid(&before, owner) || before.nlink() != 1 {
        return Err(io::Error::other("invalid private signal file").into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)?;
    checkpoint(SignalCheckpoint::AfterOpen, attempt)?;
    let opened = file.metadata()?;
    signal_descriptor_valid(&opened, owner)?;
    if identity(&opened) != identity(&before) {
        return Err(SignalReadError::Replaced);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file).take(1025).read_to_end(&mut bytes)?;
    checkpoint(SignalCheckpoint::AfterRead, attempt)?;
    signal_descriptor_valid(&file.metadata()?, owner)?;
    if bytes.len() > 1024 {
        return Err(io::Error::other("signal too large").into());
    }
    let current = fs::symlink_metadata(&path)?;
    if !signal_metadata_valid(&current, owner) || current.nlink() != 1 {
        return Err(io::Error::other("invalid private signal file").into());
    }
    if identity(&current) != identity(&opened) {
        return Err(SignalReadError::Replaced);
    }
    directory
        .verify()
        .map_err(|error| io::Error::other(format!("signal directory: {error}")))?;
    Ok(bytes)
}
// Only this atomically published signal is replaceable. The directory and owner
// marker remain immutable; malformed contents never trigger a retry.
fn read_signal_bytes_with(
    directory: &ReportDirectory,
    mut checkpoint: impl FnMut(SignalCheckpoint, usize) -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    for attempt in 0..8 {
        match signal_read_attempt(directory, attempt, &mut checkpoint) {
            Ok(bytes) => return Ok(bytes),
            Err(SignalReadError::Replaced) => std::thread::yield_now(),
            Err(SignalReadError::Invalid(error)) => return Err(error),
        }
    }
    Err(io::Error::other("signal replacement retry exhausted"))
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Signal {
    schema_version: u8,
    contract_id: String,
    state: String,
    observed_unix_millis: u128,
    run_id: String,
}
fn decode_signal(bytes: &[u8], now: u128) -> io::Result<(String, String)> {
    if bytes.len() > 1024 {
        return Err(io::Error::other("signal too large"));
    }
    let signal: Signal = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    if signal.schema_version != 1
        || signal.contract_id != "single-press-2026-10-19"
        || !matches!(
            signal.state.as_str(),
            "preparing" | "ready" | "running" | "verifying" | "passed" | "failed"
        )
        || signal.run_id.len() != 32
        || !signal
            .run_id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || now > signal.observed_unix_millis.saturating_add(2000)
        || signal.observed_unix_millis > now.saturating_add(100)
    {
        return Err(io::Error::other("invalid or stale signal"));
    }
    Ok((signal.state, signal.run_id))
}
fn check_signal_progress(
    current: &str,
    run: Option<&str>,
    next: &str,
    next_run: &str,
) -> io::Result<()> {
    let valid = match current {
        "preparing" => matches!(next, "preparing" | "ready" | "failed"),
        "ready" => matches!(next, "preparing" | "ready" | "running" | "failed"),
        "running" => matches!(next, "running" | "verifying" | "failed"),
        "verifying" => matches!(next, "verifying" | "passed" | "failed"),
        "passed" => matches!(next, "passed" | "failed"),
        "failed" => next == "failed",
        _ => false,
    };
    if !valid || run.is_some_and(|run| run != next_run) {
        return Err(io::Error::other("signal identity or phase regressed"));
    }
    Ok(())
}
fn missing_signal(current: &str, run: Option<&str>) -> io::Result<()> {
    if current == "preparing" && run.is_none() {
        Ok(())
    } else {
        Err(io::Error::other("active signal disappeared"))
    }
}
fn signal_label(state: &str) -> &'static str {
    match state {
        "ready" => {
            "Bitte jetzt einmal ⌥D drücken. Danach nichts weiter bedienen; ich übernehme den Rest."
        }
        "running" => "Test läuft. Wiedergabe und Stopp erfolgen automatisch.",
        "verifying" => "Aufnahme beendet. Das Ergebnis wird automatisch geprüft.",
        "passed" => "Test abgeschlossen. Keine weitere Bedienung erforderlich.",
        "failed" => "Test beendet. Die Diagnose erfolgt automatisch; bitte nichts drücken.",
        _ => "Test wird vorbereitet. Bitte noch nichts drücken.",
    }
}
struct Reporting {
    app: Retained<NSApplication>,
    window: Retained<NSWindow>,
    multiline: Retained<NSTextView>,
    singleline: Retained<NSTextField>,
    directory: ReportDirectory,
    instruction: Retained<NSTextField>,
    signal_state: String,
    signal_run_id: Option<String>,
    started: Instant,
    started_unix_millis: u128,
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
        let mut receipt = (0, None);
        let mut total_utf8_bytes = 0;
        let (matches, occurrences, shortcut, status) = if bounded {
            let a = multiline.to_string();
            let b = singleline.to_string();
            total_utf8_bytes = a.len() + b.len();
            let chosen_text = match focused {
                "multiline" => a.as_str(),
                "singleline" => b.as_str(),
                _ => "",
            };
            if let Ok(metrics) = receipt_metrics(chosen_text) {
                receipt = metrics;
            }
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
            process_started_unix_millis: self.started_unix_millis,
            field_utf8_bytes: receipt.0,
            field_sha256: receipt.1,
            total_utf8_bytes,
            signal_state: self.signal_state.clone(),
            signal_run_id: self.signal_run_id.clone(),
            application_active: self.app.isActive(),
            frontmost_is_self: NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|frontmost| {
                    i64::from(frontmost.processIdentifier()) == i64::from(std::process::id())
                }),
            own_bundle_identifier: NSBundle::mainBundle()
                .bundleIdentifier()
                .map(|id| id.to_string()),
            window_key: self.window.isKeyWindow(),
            focused_field: focused,
            accessibility_focused: match focused {
                "multiline" => self.multiline.isAccessibilityFocused(),
                "singleline" => self.singleline.isAccessibilityFocused(),
                _ => false,
            },
            expected_sentence_matches: matches,
            expected_sentence_occurrences: occurrences,
            shortcut_character_present: shortcut,
            running,
            status,
        })
    }
    fn refresh_signal(&mut self) -> io::Result<()> {
        let bytes = match read_signal_bytes_with(&self.directory, |_, _| Ok(())) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return missing_signal(&self.signal_state, self.signal_run_id.as_deref());
            }
            Err(error) => return Err(error),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis();
        let (state, run_id) = decode_signal(&bytes, now)?;
        check_signal_progress(
            &self.signal_state,
            self.signal_run_id.as_deref(),
            &state,
            &run_id,
        )?;
        self.signal_run_id = Some(run_id);
        if self.signal_state != state {
            self.instruction
                .setStringValue(&NSString::from_str(signal_label(&state)));
            self.signal_state = state;
        }
        Ok(())
    }
    fn publish(&mut self, stopping: bool) -> io::Result<bool> {
        if self.stopped && !stopping {
            return Ok(false);
        }
        if self.sequence > MAX_REPORTS {
            return Ok(false);
        }
        if let Err(error) = self.refresh_signal() {
            self.instruction
                .setStringValue(&NSString::from_str(signal_label("failed")));
            return Err(error);
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
    #[ivars = RefCell<Box<Reporting>>]
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

fn own_launch_url(value: &str) -> bool {
    value == "everlast-autonomous-testziel://activate"
}

struct DelegateState {
    url_activation_consumed: bool,
    reporter: Option<Retained<Reporter>>,
    activation: Option<Retained<ActivateOwnField>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<DelegateState>]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(application:openURLs:))]
        fn open_urls(&self, _: &NSApplication, urls: &NSArray<NSURL>) {
            if urls.count() != 1 { return; }
            let Some(url) = urls.firstObject() else { return; };
            let Some(value) = url.absoluteString() else { return; };
            if value.length() > 64 || !own_launch_url(&value.to_string()) { return; }
            let activation = {
                let mut state = self.ivars().borrow_mut();
                if state.url_activation_consumed || state.reporter.as_ref().is_some_and(|r| { let state = r.ivars().borrow(); state.signal_state != "preparing" || state.signal_run_id.is_some() }) { return; }
                state.url_activation_consumed = true;
                state.activation.clone()
            };
            if let Some(activation) = activation { activation.prepare_own_field(); }
        }

        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _: &NSNotification) {
            if let Some(activation) = self.ivars().borrow().activation.as_ref() {
                activation.prepare_own_field();
            }
        }
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn close_last(&self, _app: &NSApplication) -> bool {
            true
        }
        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _: &NSNotification) {
            let result = catch_unwind(AssertUnwindSafe(|| {
                if let Some(reporter) = self.ivars().borrow().reporter.as_ref() { reporter.stop(); }
            }));
            if result.is_err() { eprintln!("Own-recipient final observation failed"); }
        }
    }
);
struct OwnField {
    app: Retained<NSApplication>,
    window: Retained<NSWindow>,
    text: Retained<NSTextView>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OwnField]
    struct ActivateOwnField;
    unsafe impl NSObjectProtocol for ActivateOwnField {}
    impl ActivateOwnField {

    }
);
impl ActivateOwnField {
    fn prepare_own_field(&self) {
        // Ordinary launch of this owned fixture activates only its own window.
        // Never repeated by the passive status timer or during dictation.
        let result = catch_unwind(AssertUnwindSafe(|| {
            let own = self.ivars();
            own.window.setInitialFirstResponder(Some(&own.text));
            own.app.activate();
            own.window.makeKeyAndOrderFront(None);
            own.window.makeFirstResponder(Some(&own.text));
        }));
        if result.is_err() {
            eprintln!("Own test field activation failed");
        }
    }
}
fn label(main: MainThreadMarker, text: &str, frame: NSRect) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), main);
    label.setFrame(frame);
    label
}
fn own_process_started_unix_millis() -> io::Result<u128> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size =
        i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).map_err(io::Error::other)?;
    let pid = i32::try_from(std::process::id()).map_err(io::Error::other)?;
    // SAFETY: own PID only, writable aligned SDK structure, exact size checked
    // before reading initialized fields. This is kernel identity, not launch time.
    let received = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if received != size {
        return Err(io::Error::other("own process identity unavailable"));
    }
    // SAFETY: proc_pidinfo returned the full initialized SDK structure.
    let info = unsafe { info.assume_init() };
    if info.pbi_pid != std::process::id()
        || info.pbi_start_tvsec == 0
        || info.pbi_start_tvusec >= 1_000_000
    {
        return Err(io::Error::other("own process identity invalid"));
    }
    Ok(u128::from(info.pbi_start_tvsec) * 1000 + u128::from(info.pbi_start_tvusec) / 1000)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let started_unix_millis = own_process_started_unix_millis()?;
    let report_path = std::env::var_os("EVERLAST_RECIPIENT_REPORT_DIR").map(PathBuf::from);
    let directory = configured_report(report_path)?;
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // SAFETY: NSObject's designated initializer on an allocated main-thread subclass.
    let allocated = Delegate::alloc(main).set_ivars(RefCell::new(DelegateState {
        url_activation_consumed: false,
        reporter: None,
        activation: None,
    }));
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
    let instruction = label(
        main,
        signal_label("preparing"),
        NSRect::new(NSPoint::new(20.0, 328.0), NSSize::new(600.0, 38.0)),
    );
    instruction.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);
    instruction.setMaximumNumberOfLines(2);
    body.addSubview(&instruction);
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
    let allocated = ActivateOwnField::alloc(main).set_ivars(OwnField {
        app: app.clone(),
        window: window.clone(),
        text: text.clone(),
    });
    // SAFETY: initialize the declared own main-thread NSObject target.
    let activation: Retained<ActivateOwnField> = unsafe { msg_send![super(allocated), init] };
    delegate.ivars().borrow_mut().activation = Some(activation);
    window.setContentView(Some(&body));
    window.setInitialFirstResponder(Some(&text));
    window.center();
    window.makeFirstResponder(Some(&text));
    let timer = if let Some(directory) = directory {
        let state = Reporting {
            app: app.clone(),
            window: window.clone(),
            multiline: text.clone(),
            singleline: line.clone(),
            directory,
            instruction,
            signal_state: "preparing".into(),
            signal_run_id: None,
            started,
            started_unix_millis,
            sequence: 0,
            stopped: false,
        };
        // SAFETY: initialize the declared NSObject subclass on its main thread.
        let allocated = Reporter::alloc(main).set_ivars(RefCell::new(Box::new(state)));
        let reporter: Retained<Reporter> = unsafe { msg_send![super(allocated), init] };
        delegate.ivars().borrow_mut().reporter = Some(reporter.clone());
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
    if let Some(reporter) = delegate.ivars().borrow().reporter.as_ref() {
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
    fn signal_replacement_at(stage: SignalCheckpoint) -> io::Result<()> {
        let temp = Temp::new()?;
        let directory = ReportDirectory::create(&temp.0.join("reports"))?;
        let path = directory.path.join("signal.json");
        let publish = |bytes: &[u8]| -> io::Result<()> {
            let pending = directory.path.join("signal.pending");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&pending)?;
            file.write_all(bytes)?;
            drop(file);
            fs::rename(pending, &path)
        };
        publish(b"old")?;
        let mut replaced = false;
        let bytes = read_signal_bytes_with(&directory, |at, _| {
            if !replaced && std::mem::discriminant(&at) == std::mem::discriminant(&stage) {
                publish(b"new")?;
                replaced = true;
            }
            Ok(())
        })?;
        assert!(replaced);
        assert_eq!(
            bytes, b"new",
            "never accept bytes from the unlinked old signal"
        );
        Ok(())
    }
    #[test]
    fn signal_atomic_replacement_after_open_reads_current_version() -> io::Result<()> {
        signal_replacement_at(SignalCheckpoint::AfterOpen)
    }
    #[test]
    fn signal_atomic_replacement_after_read_discards_old_bytes() -> io::Result<()> {
        signal_replacement_at(SignalCheckpoint::AfterRead)
    }
    #[test]
    fn signal_retry_is_bounded_and_unsafe_metadata_never_retries() -> io::Result<()> {
        let temp = Temp::new()?;
        let directory = ReportDirectory::create(&temp.0.join("reports"))?;
        let signal = directory.path.join("signal.json");
        let publish = || -> io::Result<()> {
            let pending = directory.path.join("next");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&pending)?;
            file.write_all(b"{}")?;
            drop(file);
            fs::rename(pending, &signal)
        };
        publish()?;
        let mut attempts = 0;
        assert!(
            read_signal_bytes_with(&directory, |at, _| {
                if matches!(at, SignalCheckpoint::AfterOpen) {
                    attempts += 1;
                    publish()?;
                }
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 8);
        fs::hard_link(&signal, directory.path.join("hardlink"))?;
        attempts = 0;
        assert!(
            read_signal_bytes_with(&directory, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 0);
        fs::remove_file(directory.path.join("hardlink"))?;
        fs::write(&signal, [b'x'; 1025])?;
        assert!(
            read_signal_bytes_with(&directory, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 0);
        Ok(())
    }
    #[test]
    fn launch_url_is_a_single_fixed_action_without_payload() {
        assert!(own_launch_url("everlast-autonomous-testziel://activate"));
        for value in [
            "",
            "https://example.com",
            "everlast-autonomous-testziel://activate?text=x",
            "everlast-autonomous-testziel://activate/",
            "everlast-autonomous-testziel://record",
            "EVERLAST-AUTONOMOUS-TESTZIEL://activate",
        ] {
            assert!(!own_launch_url(value));
        }
    }
    #[test]
    fn reporter_ivars_fit_objective_c_runtime_alignment() {
        assert!(
            std::mem::align_of::<<Reporter as objc2::DefinedClass>::Ivars>()
                <= std::mem::align_of::<usize>(),
            "Reporter ivars must fit objc2 instance-ivar alignment"
        );
    }
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
            process_started_unix_millis: 1_800_000_000_000,
            field_utf8_bytes: 3,
            field_sha256: Some(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            ),
            total_utf8_bytes: 3,
            signal_state: "preparing".into(),
            signal_run_id: None,
            application_active: true,
            frontmost_is_self: false,
            own_bundle_identifier: Some("org.example.own-test-recipient".into()),
            window_key: true,
            focused_field: "multiline",
            accessibility_focused: true,
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
        assert!(
            first.contains("\"frontmostIsSelf\":false"),
            "active/key state alone must not imply actual frontmost identity"
        );
        assert!(first.contains("\"ownBundleIdentifier\":\"org.example.own-test-recipient\""));
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
    fn receipt_metrics_bind_actual_utf8_content_and_reject_duplicates() -> io::Result<()> {
        assert_eq!(receipt_metrics("")?, (0, None));
        assert_eq!(
            receipt_metrics("abc")?,
            (
                3,
                Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into())
            )
        );
        assert_ne!(receipt_metrics("abcabc")?, receipt_metrics("abc")?);
        assert_eq!(receipt_metrics("ä")?.0, 2);
        assert!(receipt_metrics(&"a".repeat(MAX_TEXT_BYTES + 1)).is_err());
        Ok(())
    }
    #[test]
    fn signal_is_fixed_fresh_and_never_an_arbitrary_instruction() -> io::Result<()> {
        let ready = br#"{"schemaVersion":1,"contractId":"single-press-2026-10-19","state":"ready","observedUnixMillis":10000,"runId":"0123456789abcdef0123456789abcdef"}"#;
        assert_eq!(decode_signal(ready, 10000)?.0, "ready");
        assert!(decode_signal(ready, 12001).is_err());
        assert!(decode_signal(ready, 9899).is_err());
        assert!(decode_signal(br#"{"schemaVersion":1,"contractId":"single-press-2026-10-19","state":"click","observedUnixMillis":10000,"runId":"0123456789abcdef0123456789abcdef"}"#, 10000).is_err());
        assert!(decode_signal(br#"{"schemaVersion":1,"contractId":"single-press-2026-10-19","state":"ready","observedUnixMillis":10000,"runId":"0123456789abcdef0123456789abcdef","text":"press again"}"#, 10000).is_err());
        Ok(())
    }
    #[test]
    fn signal_progress_never_rearms_and_missing_signal_revokes_readiness() -> io::Result<()> {
        let run = "0123456789abcdef0123456789abcdef";
        check_signal_progress("preparing", None, "ready", run)?;
        check_signal_progress("ready", Some(run), "ready", run)?;
        check_signal_progress("ready", Some(run), "running", run)?;
        check_signal_progress("running", Some(run), "verifying", run)?;
        check_signal_progress("verifying", Some(run), "passed", run)?;
        for state in ["running", "verifying", "passed", "failed"] {
            assert!(check_signal_progress(state, Some(run), "ready", run).is_err());
        }
        assert!(
            check_signal_progress(
                "ready",
                Some(run),
                "running",
                "abcdef0123456789abcdef0123456789"
            )
            .is_err()
        );
        assert!(missing_signal("ready", Some(run)).is_err());
        assert!(missing_signal("preparing", Some(run)).is_err());
        check_signal_progress("ready", Some(run), "preparing", run)?;
        check_signal_progress("preparing", Some(run), "ready", run)?;
        assert!(check_signal_progress("running", Some(run), "preparing", run).is_err());
        assert!(missing_signal("preparing", None).is_ok());
        Ok(())
    }
    #[test]
    fn own_kernel_process_identity_is_stable_across_observations() -> io::Result<()> {
        let first = own_process_started_unix_millis()?;
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert_eq!(own_process_started_unix_millis()?, first);
        assert!(
            first
                <= SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(io::Error::other)?
                    .as_millis()
        );
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
