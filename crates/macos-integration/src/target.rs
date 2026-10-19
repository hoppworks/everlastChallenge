//! Read-only target validation; no activation, clipboard mutation or typing.

pub mod worker;

use core_foundation::{
    base::{CFRange, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    number::CFNumber,
    string::{CFString, CFStringGetCharacters, CFStringRef},
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSWorkspace;
use std::{
    cell::RefCell,
    ffi::c_void,
    marker::PhantomData,
    mem::MaybeUninit,
    num::NonZeroU64,
    rc::Rc,
    time::{Duration, Instant},
};

// Types and ownership follow the pinned SDK's HIServices/AXUIElement.h.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetType(value: CFTypeRef) -> u32;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, result: *mut c_void) -> u8;
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> CFTypeRef;
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementGetPid(element: CFTypeRef, pid: *mut libc::pid_t) -> i32;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, seconds: f32) -> i32;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementIsAttributeSettable(
        element: CFTypeRef,
        attribute: CFStringRef,
        settable: *mut u8,
    ) -> i32;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> u8;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TargetError {
    #[error("Accessibility permission is missing")]
    Permission,
    #[error("Secure Event Input is active")]
    SecureInput,
    #[error("Target process cannot be confirmed")]
    Process,
    #[error("Target process or focused field changed")]
    Changed,
    #[error("Accessibility query failed ({0})")]
    Accessibility(i32),
    #[error("Accessibility returned an unexpected or missing value")]
    Value,
    #[error("Target field is not eligible: {0:?}")]
    Field(HoldReason),
    #[error("Target verification is stale or belongs to another session")]
    StaleApproval,
    #[error("Target work was cancelled")]
    Cancelled,
}

/// PID plus kernel start timestamp; never identifies a process by PID alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessIdentity {
    pid: libc::pid_t,
    start_seconds: u64,
    start_microseconds: u64,
}

impl ProcessIdentity {
    pub fn read(pid: libc::pid_t) -> Result<Self, TargetError> {
        if pid <= 0 {
            return Err(TargetError::Process);
        }
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
        let size =
            i32::try_from(size_of::<libc::proc_bsdinfo>()).map_err(|_| TargetError::Process)?;
        // SAFETY: info points to writable, aligned storage of the exact SDK
        // struct size. It is read only after the API confirms that full size.
        let received = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast::<c_void>(),
                size,
            )
        };
        if received != size {
            return Err(TargetError::Process);
        }
        // SAFETY: proc_pidinfo reported the complete initialized structure.
        let info = unsafe { info.assume_init() };
        if info.pbi_pid != pid as u32 {
            return Err(TargetError::Process);
        }
        Ok(Self {
            pid,
            start_seconds: info.pbi_start_tvsec,
            start_microseconds: info.pbi_start_tvusec,
        })
    }

    pub fn pid(self) -> libc::pid_t {
        self.pid
    }
}

/// Read on the GUI thread, then pass only this value to the AX owner worker.
pub fn frontmost_process(_main_thread: MainThreadMarker) -> Result<ProcessIdentity, TargetError> {
    let application = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .ok_or(TargetError::Process)?;
    ProcessIdentity::read(application.processIdentifier())
}

/// Developer test boundary: accept only the project's disposable native recipient.
/// Reads the frontmost identity without activating a window or querying its text.
#[cfg(feature = "test-control")]
pub fn frontmost_test_recipient(expected_pid: i32) -> Result<ProcessIdentity, TargetError> {
    let _main = MainThreadMarker::new().ok_or(TargetError::Process)?;
    let application = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .ok_or(TargetError::Process)?;
    let identifier = application.bundleIdentifier().ok_or(TargetError::Process)?;
    if !is_test_recipient_bundle(&identifier.to_string())
        || application.processIdentifier() != expected_pid
    {
        return Err(TargetError::Changed);
    }
    ProcessIdentity::read(expected_pid)
}

#[cfg(feature = "test-control")]
fn is_test_recipient_bundle(identifier: &str) -> bool {
    matches!(
        identifier,
        "de.hoppworks.everlast-native-recipient-hotkey-v1"
            | "de.hoppworks.everlast-native-test-recipient"
            | "de.hoppworks.everlast-native-autonomous-recipient"
    )
}

/// Read-only diagnostic for the configured disposable recipient. This returns
/// metadata, never a Target/TargetApproval, and does not authorize delivery.
#[cfg(feature = "test-control")]
pub mod inspection {
    use super::*;
    use objc2_app_kit::NSRunningApplication;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ReadStatus {
        Value,
        Unsupported,
        NoValue,
        Error,
        WrongType,
        MissingValue,
    }
    impl ReadStatus {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Value => "value",
                Self::Unsupported => "unsupported",
                Self::NoValue => "noValue",
                Self::Error => "error",
                Self::WrongType => "wrongType",
                Self::MissingValue => "missingValue",
            }
        }
    }
    #[derive(Debug)]
    pub struct Read<T> {
        pub status: ReadStatus,
        pub error_code: Option<i32>,
        pub value: Option<T>,
    }
    fn interpreted<T>(
        result: Result<CFType, TargetError>,
        decode: impl FnOnce(CFType) -> Option<T>,
    ) -> Read<T> {
        match result {
            Ok(value) => match decode(value) {
                Some(value) => Read {
                    status: ReadStatus::Value,
                    error_code: None,
                    value: Some(value),
                },
                None => Read {
                    status: ReadStatus::WrongType,
                    error_code: None,
                    value: None,
                },
            },
            Err(TargetError::Accessibility(code)) => Read {
                status: match code {
                    -25205 => ReadStatus::Unsupported,
                    -25212 => ReadStatus::NoValue,
                    _ => ReadStatus::Error,
                },
                error_code: Some(code),
                value: None,
            },
            Err(TargetError::Value) => Read {
                status: ReadStatus::MissingValue,
                error_code: None,
                value: None,
            },
            Err(_) => Read {
                status: ReadStatus::Error,
                error_code: None,
                value: None,
            },
        }
    }

    pub struct Metadata {
        pub pid: i32,
        pub role: Read<FieldRole>,
        pub subrole: Read<bool>,
        pub enabled: Read<bool>,
        pub value_settable: Option<bool>,
        pub value_settable_error_code: Option<i32>,
        pub selected_text_settable: Option<bool>,
        pub selected_text_settable_error_code: Option<i32>,
        pub evidence: FieldEvidence,
    }

    fn configured_recipient(pid: i32) -> Result<ProcessIdentity, TargetError> {
        let before = ProcessIdentity::read(pid)?;
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .ok_or(TargetError::Process)?;
        if app.isTerminated()
            || app.processIdentifier() != pid
            || !app
                .bundleIdentifier()
                .is_some_and(|id| is_test_recipient_bundle(&id.to_string()))
            || ProcessIdentity::read(pid)? != before
        {
            return Err(TargetError::Changed);
        }
        Ok(before)
    }
    fn within(deadline: Instant) -> Result<(), &'static str> {
        if Instant::now() < deadline {
            Ok(())
        } else {
            Err("Recipient inspection deadline exceeded")
        }
    }

    /// All AX references remain on this calling main thread. Per-query AX
    /// timeouts are 250ms, with a 1250ms admission deadline between queries.
    pub fn inspect(pid: i32) -> Result<Metadata, &'static str> {
        let _main = MainThreadMarker::new().ok_or("Recipient inspection requires main thread")?;
        if !accessibility_trusted() {
            return Err("Accessibility permission is missing");
        }
        let before = configured_recipient(pid)
            .map_err(|_| "Configured recipient identity is unavailable")?;
        let deadline = Instant::now() + Duration::from_millis(1250);
        let application =
            Element::application(pid).map_err(|_| "Recipient AX application is unavailable")?;
        let field = application
            .focused(pid)
            .map_err(|_| "Recipient focused field is unavailable")?;
        within(deadline)?;
        let role = interpreted(field.attribute("AXRole"), |value| {
            value.downcast_into::<CFString>().map(|value| {
                if value == CFString::from_static_string("AXTextArea") {
                    FieldRole::TextArea
                } else if value == CFString::from_static_string("AXTextField") {
                    FieldRole::TextField
                } else if value == CFString::from_static_string("AXComboBox") {
                    FieldRole::ComboBox
                } else {
                    FieldRole::Other
                }
            })
        });
        within(deadline)?;
        let subrole = interpreted(field.attribute("AXSubrole"), |value| {
            value
                .downcast_into::<CFString>()
                .map(|value| value == CFString::from_static_string("AXSecureTextField"))
        });
        within(deadline)?;
        let enabled = interpreted(field.attribute("AXEnabled"), |value| {
            value.downcast_into::<CFBoolean>().map(bool::from)
        });
        within(deadline)?;
        let key = CFString::from_static_string("AXValue");
        let mut settable = 0;
        // SAFETY: only asks whether AXValue is settable; never reads its value.
        // field/key are live on this thread and output storage is writable.
        let code = unsafe {
            AXUIElementIsAttributeSettable(field.raw(), key.as_concrete_TypeRef(), &mut settable)
        };
        within(deadline)?;
        let (selected_text_settable, selected_text_settable_error_code) = field
            .settable_before("AXSelectedText", deadline)
            .map_err(|_| "Recipient inspection deadline exceeded")?;
        within(deadline)?;
        if configured_recipient(pid).map_err(|_| "Configured recipient identity changed")? != before
        {
            return Err("Configured recipient identity changed");
        }
        let value_settable = (code == 0).then_some(settable != 0);
        let field_role = role.value.unwrap_or(FieldRole::Other);
        let subrole_evidence = match (subrole.status, subrole.value, subrole.error_code) {
            (ReadStatus::Value, Some(value), _) => SubroleEvidence::Value(value),
            (_, _, Some(code)) => SubroleEvidence::Error(code),
            (ReadStatus::MissingValue, _, _) => SubroleEvidence::MissingValue,
            _ => SubroleEvidence::UnexpectedType,
        };
        let evidence = FieldEvidence {
            role: field_role,
            secure: subrole_security(field_role, subrole_evidence),
            enabled: enabled.value,
            value_settable,
            selected_text_settable,
        };
        Ok(Metadata {
            pid,
            role,
            subrole,
            enabled,
            value_settable,
            value_settable_error_code: (code != 0).then_some(code),
            selected_text_settable,
            selected_text_settable_error_code,
            evidence,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn diagnostic_preserves_absence_errors_and_wrong_types_without_values() {
            for (code, status) in [
                (-25205, ReadStatus::Unsupported),
                (-25212, ReadStatus::NoValue),
                (-25204, ReadStatus::Error),
            ] {
                let read = interpreted::<bool>(Err(TargetError::Accessibility(code)), |_| None);
                assert_eq!(read.status, status);
                assert_eq!(read.error_code, Some(code));
                assert_eq!(read.value, None);
            }
            let wrong = interpreted(
                Ok(CFString::from_static_string("private sentinel").as_CFType()),
                |value| value.downcast_into::<CFBoolean>().map(bool::from),
            );
            assert_eq!(wrong.status, ReadStatus::WrongType);
            assert_eq!(wrong.value, None);
        }
        #[test]
        fn only_exact_disposable_recipient_bundles_are_allowed() {
            assert!(is_test_recipient_bundle(
                "de.hoppworks.everlast-native-autonomous-recipient"
            ));
            for bundle in [
                "com.apple.Notes",
                "de.hoppworks.everlast-native-autonomous-recipient.evil",
                "",
            ] {
                assert!(!is_test_recipient_bundle(bundle));
            }
        }
    }
}

pub fn accessibility_trusted() -> bool {
    // SAFETY: query takes no pointers, prompts for nothing and changes no TCC state.
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn secure_input_active() -> bool {
    // SAFETY: documented process-global read-only query, with no arguments.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

struct Element {
    value: CFType,
    // AX references are created, queried and released on one owner thread.
    _owner: PhantomData<Rc<()>>,
}

impl Element {
    fn checked(value: CFType) -> Result<Self, TargetError> {
        // SAFETY: the function has no arguments and returns a stable CF type ID.
        if value.type_of() != unsafe { AXUIElementGetTypeID() } {
            return Err(TargetError::Value);
        }
        let element = Self {
            value,
            _owner: PhantomData,
        };
        // SAFETY: checked live AX reference; timeout affects only this object.
        ax_result(unsafe { AXUIElementSetMessagingTimeout(element.raw(), 0.25) })?;
        Ok(element)
    }

    fn application(pid: libc::pid_t) -> Result<Self, TargetError> {
        // SAFETY: positive PID was validated by ProcessIdentity. Create rule
        // transfers one owned reference; null is checked before wrapping.
        let raw = unsafe { AXUIElementCreateApplication(pid) };
        if raw.is_null() {
            return Err(TargetError::Value);
        }
        // SAFETY: non-null owned Create-rule CF reference from the SDK.
        Self::checked(unsafe { CFType::wrap_under_create_rule(raw) })
    }

    fn system() -> Result<Self, TargetError> {
        // SAFETY: Create rule returns an owned system-wide AX root, not content.
        let raw = unsafe { AXUIElementCreateSystemWide() };
        if raw.is_null() {
            return Err(TargetError::Value);
        }
        // SAFETY: the non-null reference is owned by this call.
        Self::checked(unsafe { CFType::wrap_under_create_rule(raw) })
    }

    fn before(&self, deadline: Instant) -> Result<(), TargetError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(TargetError::StaleApproval)?;
        if remaining.is_zero() {
            return Err(TargetError::StaleApproval);
        }
        // SAFETY: live owner-thread element; timeout is scoped to this reference.
        ax_result(unsafe {
            AXUIElementSetMessagingTimeout(self.raw(), remaining.as_secs_f32().min(0.1))
        })
    }
    fn attribute_before(
        &self,
        name: &'static str,
        deadline: Instant,
    ) -> Result<CFType, TargetError> {
        self.before(deadline)?;
        self.attribute(name)
    }
    fn settable_before(
        &self,
        name: &'static str,
        deadline: Instant,
    ) -> Result<(Option<bool>, Option<i32>), TargetError> {
        self.before(deadline)?;
        let key = CFString::from_static_string(name);
        let mut settable = 0;
        // SAFETY: live references, writable output; capability query reads no text.
        let status = unsafe {
            AXUIElementIsAttributeSettable(self.raw(), key.as_concrete_TypeRef(), &mut settable)
        };
        Ok((
            (status == 0).then_some(settable != 0),
            (status != 0).then_some(status),
        ))
    }

    fn raw(&self) -> CFTypeRef {
        self.value.as_CFTypeRef()
    }

    fn attribute(&self, name: &'static str) -> Result<CFType, TargetError> {
        let key = CFString::from_static_string(name);
        let mut value = std::ptr::null();
        // SAFETY: self/key are live; output storage is writable. A returned
        // value follows Copy ownership and is always released by its wrapper.
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.raw(), key.as_concrete_TypeRef(), &mut value)
        };
        let owned = if value.is_null() {
            None
        } else {
            // SAFETY: non-null Copy-rule reference returned by AX.
            Some(unsafe { CFType::wrap_under_create_rule(value) })
        };
        ax_result(status)?;
        owned.ok_or(TargetError::Value)
    }

    fn string(&self, name: &'static str) -> Result<CFString, TargetError> {
        self.attribute(name)?
            .downcast_into::<CFString>()
            .ok_or(TargetError::Value)
    }

    fn related(
        &self,
        name: &'static str,
        pid: libc::pid_t,
        deadline: Instant,
    ) -> Result<Self, TargetError> {
        let element = Self::checked(self.attribute_before(name, deadline)?)?;
        let mut observed = 0;
        // SAFETY: element is a checked AX reference; output is writable.
        ax_result(unsafe { AXUIElementGetPid(element.raw(), &mut observed) })?;
        if observed != pid {
            return Err(TargetError::Changed);
        }
        Ok(element)
    }
    #[cfg(feature = "test-control")]
    fn focused(&self, pid: libc::pid_t) -> Result<Self, TargetError> {
        self.related(
            "AXFocusedUIElement",
            pid,
            Instant::now() + Duration::from_millis(750),
        )
    }

    fn evidence(&self, deadline: Instant) -> Result<FieldEvidence, TargetError> {
        let role = self
            .attribute_before("AXRole", deadline)?
            .downcast_into::<CFString>()
            .ok_or(TargetError::Value)?;
        let role = if role == CFString::from_static_string("AXTextField") {
            FieldRole::TextField
        } else if role == CFString::from_static_string("AXTextArea") {
            FieldRole::TextArea
        } else if role == CFString::from_static_string("AXComboBox") {
            FieldRole::ComboBox
        } else {
            FieldRole::Other
        };
        let subrole = match self.attribute_before("AXSubrole", deadline) {
            Ok(value) => match value.downcast_into::<CFString>() {
                Some(value) => SubroleEvidence::Value(
                    value == CFString::from_static_string("AXSecureTextField"),
                ),
                None => SubroleEvidence::UnexpectedType,
            },
            Err(TargetError::Accessibility(code)) => SubroleEvidence::Error(code),
            Err(TargetError::Value) => SubroleEvidence::MissingValue,
            Err(_) => SubroleEvidence::UnexpectedType,
        };
        let secure = subrole_security(role, subrole);
        let enabled = self
            .attribute_before("AXEnabled", deadline)
            .ok()
            .and_then(|value| value.downcast_into::<CFBoolean>())
            .map(bool::from);
        let (value_settable, _) = self.settable_before("AXValue", deadline)?;
        let (selected_text_settable, _) = self.settable_before("AXSelectedText", deadline)?;
        Ok(FieldEvidence {
            role,
            secure,
            enabled,
            value_settable,
            selected_text_settable,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusRoute {
    System,
    Application,
    Window,
}

/// Shared resolver seam: only absent routes and opaque containers may fall
/// through. A concrete unsafe field, foreign PID or failed security query ends
/// selection; no later route can replace that negative evidence.
fn resolve_candidates<T>(
    mut candidate: impl FnMut(FocusRoute) -> Result<(T, FieldEvidence), TargetError>,
) -> Result<T, TargetError> {
    for route in [
        FocusRoute::System,
        FocusRoute::Application,
        FocusRoute::Window,
    ] {
        match candidate(route) {
            Ok((_, evidence)) if evidence.secure == Some(true) => {
                return Err(TargetError::Field(HoldReason::SecureField));
            }
            Ok((_field, evidence)) if evidence.role == FieldRole::Other => continue,
            Ok((field, evidence)) => {
                evidence.eligibility().map_err(TargetError::Field)?;
                return Ok(field);
            }
            Err(TargetError::Accessibility(-25205 | -25212)) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(TargetError::Field(HoldReason::UnconfirmedField))
}

fn resolve_field(application: &Element, process: ProcessIdentity) -> Result<Element, TargetError> {
    let deadline = Instant::now() + Duration::from_millis(750);
    let field = resolve_candidates(|route| {
        check_environment()?;
        if ProcessIdentity::read(process.pid)? != process {
            return Err(TargetError::Changed);
        }
        let field = match route {
            FocusRoute::System => {
                Element::system()?.related("AXFocusedUIElement", process.pid, deadline)?
            }
            FocusRoute::Application => {
                application.related("AXFocusedUIElement", process.pid, deadline)?
            }
            FocusRoute::Window => application
                .related("AXFocusedWindow", process.pid, deadline)?
                .related("AXFocusedUIElement", process.pid, deadline)?,
        };
        let evidence = field.evidence(deadline)?;
        Ok((field, evidence))
    })?;
    check_environment()?;
    if ProcessIdentity::read(process.pid)? != process {
        return Err(TargetError::Changed);
    }
    Ok(field)
}

fn ax_result(status: i32) -> Result<(), TargetError> {
    if status == 0 {
        Ok(())
    } else {
        Err(TargetError::Accessibility(status))
    }
}

const MAX_OBSERVED_UNITS: usize = 128 * 1024;
const MAX_OBSERVED_BYTES: usize = 256 * 1024;

// Kept only by the AX owner for one target; never emitted, persisted or logged.
struct TextSnapshot {
    observed_at: Instant,
    text: Vec<u16>,
    selection: std::ops::Range<usize>,
}
impl TextSnapshot {
    fn compare(&self, after: &[u16], inserted: &str) -> worker::PostSendObservation {
        use worker::PostSendObservation::*;
        let replacement: Vec<u16> = inserted.encode_utf16().collect();
        let Some(length) = self
            .text
            .len()
            .checked_sub(self.selection.len())
            .and_then(|n| n.checked_add(replacement.len()))
        else {
            return Unverifiable;
        };
        if length > MAX_OBSERVED_UNITS || inserted.len() > MAX_OBSERVED_BYTES {
            return Unverifiable;
        }
        let expected: Vec<u16> = self.text[..self.selection.start]
            .iter()
            .chain(replacement.iter())
            .chain(self.text[self.selection.end..].iter())
            .copied()
            .collect();
        if expected == self.text {
            return Unverifiable;
        }
        if expected == after {
            Confirmed
        } else {
            Uncertain
        }
    }
}
fn bounded_text_value(
    mut attribute: impl FnMut(&'static str) -> Result<CFType, TargetError>,
) -> Option<(CFString, usize)> {
    // Refuse optional text observation before fetching a potentially huge value.
    // The server can still change between these two reads; recheck the actual value.
    let number = attribute("AXNumberOfCharacters")
        .ok()?
        .downcast_into::<CFNumber>()?;
    let reported = number.to_i64()?;
    let count = usize::try_from(reported).ok()?;
    if count > MAX_OBSERVED_UNITS || number.to_f64()? != reported as f64 {
        return None;
    }
    let value = attribute("AXValue").ok()?.downcast_into::<CFString>()?;
    let actual = usize::try_from(value.char_len()).ok()?;
    (actual <= MAX_OBSERVED_UNITS).then_some((value, actual))
}

impl Element {
    fn bounded_text(&self, deadline: Instant) -> Option<Vec<u16>> {
        let (value, count) = bounded_text_value(|name| self.attribute_before(name, deadline))?;
        let mut text = vec![0u16; count];
        // SAFETY: CFString has exactly count UTF-16 units and text owns that capacity.
        unsafe {
            CFStringGetCharacters(
                value.as_concrete_TypeRef(),
                CFRange {
                    location: 0,
                    length: count as isize,
                },
                text.as_mut_ptr(),
            );
        }
        if String::from_utf16(&text).ok()?.len() > MAX_OBSERVED_BYTES {
            return None;
        }
        Some(text)
    }
    fn text_snapshot(&self, deadline: Instant) -> Option<TextSnapshot> {
        let range_value = self
            .attribute_before("AXSelectedTextRange", deadline)
            .ok()?;
        // SAFETY: only a checked AXValue wrapping CFRange is decoded into valid storage.
        let range = unsafe {
            if range_value.type_of() != AXValueGetTypeID()
                || AXValueGetType(range_value.as_CFTypeRef()) != 4
            {
                return None;
            }
            let mut range = CFRange {
                location: 0,
                length: 0,
            };
            if AXValueGetValue(
                range_value.as_CFTypeRef(),
                4,
                (&mut range as *mut CFRange).cast(),
            ) == 0
            {
                return None;
            }
            range
        };
        let text = self.bounded_text(deadline)?;
        let start = usize::try_from(range.location).ok()?;
        let end = start.checked_add(usize::try_from(range.length).ok()?)?;
        if end > text.len() {
            return None;
        }
        // Reject UTF-16 selections bisecting a surrogate pair.
        String::from_utf16(&text[..start]).ok()?;
        String::from_utf16(&text[end..]).ok()?;
        Some(TextSnapshot {
            observed_at: Instant::now(),
            text,
            selection: start..end,
        })
    }
}

/// Opaque and deliberately !Send/!Sync: keep it on the serial AX worker.
/// Failure to capture must remain a missing target for that entire session.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<macos_integration::target::Target>();
/// ```
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<macos_integration::target::Target>();
/// ```
pub struct Target {
    session: NonZeroU64,
    process: ProcessIdentity,
    application: Element,
    field: Element,
    before_send: RefCell<Option<TextSnapshot>>,
}

impl Target {
    pub fn capture(session: NonZeroU64, process: ProcessIdentity) -> Result<Self, TargetError> {
        check_environment()?;
        if ProcessIdentity::read(process.pid)? != process {
            return Err(TargetError::Changed);
        }
        let application = Element::application(process.pid)?;
        if application.string("AXRole")? != CFString::from_static_string("AXApplication") {
            return Err(TargetError::Value);
        }
        let field = resolve_field(&application, process)?;
        Ok(Self {
            session,
            process,
            application,
            field,
            before_send: RefCell::new(None),
        })
    }

    /// Caller must obtain a fresh frontmost identity immediately before this
    /// check. Never activates an application or finds a replacement field.
    pub fn verify(&self, frontmost: ProcessIdentity) -> Result<(), TargetError> {
        check_environment()?;
        if frontmost != self.process || ProcessIdentity::read(self.process.pid)? != self.process {
            return Err(TargetError::Changed);
        }
        let focused = resolve_field(&self.application, self.process)?;
        if focused.value != self.field.value {
            return Err(TargetError::Changed);
        }
        Ok(())
    }

    /// The receipt is single-use and carries no native references. It may cross
    /// to the main thread for one mutation or send check, within 250 ms of the
    /// start of this verification (including AX messaging time).
    pub fn approve(&self, frontmost: ProcessIdentity) -> Result<TargetApproval, TargetError> {
        let checked_at = Instant::now();
        self.verify(frontmost)?;
        *self.before_send.borrow_mut() = self
            .field
            .text_snapshot(checked_at + Duration::from_millis(250));
        Ok(TargetApproval {
            session: self.session,
            process: self.process,
            checked_at,
        })
    }

    pub(crate) fn clear_observation(&self) {
        self.before_send.borrow_mut().take();
    }

    pub(crate) fn expire_observation(&self) {
        if self
            .before_send
            .borrow()
            .as_ref()
            .is_some_and(|value| value.observed_at.elapsed() > Duration::from_secs(1))
        {
            self.clear_observation();
        }
    }

    /// Read-only post-send check; never captures a replacement target or grants approval.
    pub fn observe_after_send(
        &self,
        frontmost: ProcessIdentity,
        expected: &str,
    ) -> Result<worker::PostSendObservation, TargetError> {
        // Consume before checking: every outcome drops the private snapshot.
        let before = self.before_send.borrow_mut().take();
        if expected.len() > MAX_OBSERVED_BYTES {
            return Err(TargetError::Value);
        }
        self.verify(frontmost)?;
        let Some(before) =
            before.filter(|value| value.observed_at.elapsed() <= Duration::from_secs(1))
        else {
            return Ok(worker::PostSendObservation::Unverifiable);
        };
        let deadline = Instant::now() + Duration::from_millis(150);
        let mut observed_mismatch = false;
        loop {
            let Some(after) = self.field.bounded_text(deadline) else {
                self.verify(frontmost)?;
                return Ok(if observed_mismatch {
                    worker::PostSendObservation::Uncertain
                } else {
                    worker::PostSendObservation::Unverifiable
                });
            };
            let observation = before.compare(&after, expected);
            if observation != worker::PostSendObservation::Uncertain
                || Instant::now() + Duration::from_millis(10) >= deadline
            {
                self.verify(frontmost)?;
                return Ok(observation);
            }
            observed_mismatch = true;
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// A single-use receipt: a second delivery stage needs a new AX verification.
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<macos_integration::target::TargetApproval>();
/// ```
pub struct TargetApproval {
    session: NonZeroU64,
    process: ProcessIdentity,
    checked_at: Instant,
}

impl TargetApproval {
    /// Consumes this receipt. Native references remain on their owner worker.
    pub fn validate(
        self,
        session: NonZeroU64,
        frontmost: ProcessIdentity,
    ) -> Result<(), TargetError> {
        if self.session != session || self.checked_at.elapsed() > Duration::from_millis(250) {
            return Err(TargetError::StaleApproval);
        }
        if self.process != frontmost {
            return Err(TargetError::Changed);
        }
        Ok(())
    }
}

fn check_environment() -> Result<(), TargetError> {
    if !accessibility_trusted() {
        return Err(TargetError::Permission);
    }
    if secure_input_active() {
        return Err(TargetError::SecureInput);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldReason {
    UnconfirmedField,
    NotEditable,
    UnknownEditability,
    SecureField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRole {
    TextField,
    TextArea,
    ComboBox,
    Other,
}

// Preserve absence codes separately from malformed replies and messaging failures.
#[derive(Debug, Clone, Copy)]
enum SubroleEvidence {
    Value(bool),
    Error(i32),
    MissingValue,
    UnexpectedType,
}
fn subrole_security(role: FieldRole, observation: SubroleEvidence) -> Option<bool> {
    match observation {
        SubroleEvidence::Value(secure) => Some(secure),
        // NSAccessibility.accessibilitySubrole is nullable; AX documents these
        // two absence replies are ordinary for supported native text roles.
        // Malformed values and actual communication failures remain unknown.
        SubroleEvidence::Error(-25205 | -25212) if role != FieldRole::Other => Some(false),
        _ => None,
    }
}

pub struct FieldEvidence {
    pub role: FieldRole,
    pub secure: Option<bool>,
    pub enabled: Option<bool>,
    pub value_settable: Option<bool>,
    pub selected_text_settable: Option<bool>,
}

impl FieldEvidence {
    pub fn eligibility(&self) -> Result<(), HoldReason> {
        if self.role == FieldRole::Other || self.secure.is_none() {
            return Err(HoldReason::UnconfirmedField);
        }
        if self.secure == Some(true) {
            return Err(HoldReason::SecureField);
        }
        if self.enabled == Some(false) {
            return Err(HoldReason::NotEditable);
        }
        match (self.value_settable, self.selected_text_settable) {
            (Some(true), _) | (_, Some(true)) => Ok(()),
            (Some(false), _) | (_, Some(false)) => Err(HoldReason::NotEditable),
            _ => Err(HoldReason::UnknownEditability),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary_field() -> FieldEvidence {
        FieldEvidence {
            role: FieldRole::TextArea,
            secure: Some(false),
            enabled: None,
            value_settable: Some(true),
            selected_text_settable: None,
        }
    }

    #[test]
    fn resolver_falls_back_in_order_from_absent_and_opaque_to_known_field()
    -> Result<(), TargetError> {
        let mut calls = Vec::new();
        let resolved = resolve_candidates(|route| {
            calls.push(route);
            match route {
                FocusRoute::System => Err(TargetError::Accessibility(-25212)),
                FocusRoute::Application => Ok((
                    1,
                    FieldEvidence {
                        role: FieldRole::Other,
                        ..ordinary_field()
                    },
                )),
                FocusRoute::Window => Ok((2, ordinary_field())),
            }
        })?;
        assert_eq!(resolved, 2);
        assert_eq!(
            calls,
            [
                FocusRoute::System,
                FocusRoute::Application,
                FocusRoute::Window
            ]
        );
        let mut called = 0;
        assert_eq!(
            resolve_candidates(|_| {
                called += 1;
                Ok((7, ordinary_field()))
            })?,
            7
        );
        assert_eq!(called, 1);
        Ok(())
    }

    #[test]
    fn resolver_never_falls_through_concrete_security_disabled_or_readonly_fields() {
        for (field, reason) in [
            (
                FieldEvidence {
                    secure: Some(true),
                    ..ordinary_field()
                },
                HoldReason::SecureField,
            ),
            (
                FieldEvidence {
                    secure: None,
                    ..ordinary_field()
                },
                HoldReason::UnconfirmedField,
            ),
            (
                FieldEvidence {
                    enabled: Some(false),
                    ..ordinary_field()
                },
                HoldReason::NotEditable,
            ),
            (
                FieldEvidence {
                    value_settable: Some(false),
                    ..ordinary_field()
                },
                HoldReason::NotEditable,
            ),
            (
                FieldEvidence {
                    value_settable: None,
                    ..ordinary_field()
                },
                HoldReason::UnknownEditability,
            ),
        ] {
            let mut calls = 0;
            let result = resolve_candidates(|_| {
                calls += 1;
                Ok((
                    1,
                    FieldEvidence {
                        role: field.role,
                        secure: field.secure,
                        enabled: field.enabled,
                        value_settable: field.value_settable,
                        selected_text_settable: field.selected_text_settable,
                    },
                ))
            });
            assert!(matches!(result, Err(TargetError::Field(actual)) if actual == reason));
            assert_eq!(calls, 1);
        }
        for error in [
            TargetError::Value,
            TargetError::Changed,
            TargetError::Process,
            TargetError::Permission,
            TargetError::SecureInput,
            TargetError::Accessibility(-25204),
        ] {
            let mut calls = 0;
            let result = resolve_candidates::<()>(|_| {
                calls += 1;
                Err(error.clone())
            });
            assert!(result.is_err());
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn resolver_with_no_field_is_unconfirmed_and_does_not_invent_editability() {
        let mut calls = 0;
        let result = resolve_candidates::<()>(|_| {
            calls += 1;
            Err(TargetError::Accessibility(-25205))
        });
        assert!(matches!(
            result,
            Err(TargetError::Field(HoldReason::UnconfirmedField))
        ));
        assert_eq!(calls, 3);
    }

    #[test]
    fn approvals_reject_old_checks_wrong_sessions_and_reused_process_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let session = NonZeroU64::new(1).ok_or("invalid id")?;
        let other_session = NonZeroU64::new(2).ok_or("invalid id")?;
        let process = ProcessIdentity {
            pid: 123,
            start_seconds: 10,
            start_microseconds: 20,
        };
        let receipt = || TargetApproval {
            session,
            process,
            checked_at: Instant::now(),
        };
        receipt().validate(session, process)?;
        assert!(matches!(
            receipt().validate(other_session, process),
            Err(TargetError::StaleApproval)
        ));
        let reused_pid = ProcessIdentity {
            start_seconds: 30,
            ..process
        };
        assert!(matches!(
            receipt().validate(session, reused_pid),
            Err(TargetError::Changed)
        ));
        let old = TargetApproval {
            session,
            process,
            checked_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .ok_or("clock fixture failed")?,
        };
        assert!(matches!(
            old.validate(session, process),
            Err(TargetError::StaleApproval)
        ));
        Ok(())
    }

    #[test]
    fn kernel_process_identity_is_stable_and_rejects_invalid_pids()
    -> Result<(), Box<dyn std::error::Error>> {
        let pid = libc::pid_t::try_from(std::process::id())?;
        let first = ProcessIdentity::read(pid)?;
        assert_eq!(first.pid(), pid);
        assert_eq!(ProcessIdentity::read(pid)?, first);
        assert!(matches!(
            ProcessIdentity::read(0),
            Err(TargetError::Process)
        ));
        assert!(matches!(
            ProcessIdentity::read(-1),
            Err(TargetError::Process)
        ));
        Ok(())
    }

    #[test]
    fn text_area_optional_subrole_absence_is_distinct_from_failed_or_malformed_queries() {
        let eligibility = |role, observation| {
            FieldEvidence {
                role,
                secure: subrole_security(role, observation),
                enabled: Some(true),
                value_settable: Some(true),
                selected_text_settable: None,
            }
            .eligibility()
        };
        // Public AX constants: AttributeUnsupported and NoValue. The optional
        // subrole on a text area is distinct from a failed security query.
        for code in [-25205, -25212] {
            assert_eq!(
                eligibility(FieldRole::TextArea, SubroleEvidence::Error(code)),
                Ok(())
            );
            for role in [FieldRole::TextField, FieldRole::ComboBox] {
                assert_eq!(eligibility(role, SubroleEvidence::Error(code)), Ok(()));
            }
            assert_eq!(
                eligibility(FieldRole::Other, SubroleEvidence::Error(code)),
                Err(HoldReason::UnconfirmedField)
            );
        }
        for observation in [
            SubroleEvidence::Error(-25204), // CannotComplete / messaging failure
            SubroleEvidence::Error(-25202), // InvalidUIElement
            SubroleEvidence::Error(-25208), // NotImplemented
            SubroleEvidence::Error(-25211), // APIDisabled
            SubroleEvidence::Error(0),
            SubroleEvidence::UnexpectedType,
            SubroleEvidence::MissingValue,
        ] {
            assert_eq!(
                eligibility(FieldRole::TextArea, observation),
                Err(HoldReason::UnconfirmedField)
            );
        }
        for role in [
            FieldRole::TextArea,
            FieldRole::TextField,
            FieldRole::ComboBox,
        ] {
            assert_eq!(
                eligibility(role, SubroleEvidence::Value(true)),
                Err(HoldReason::SecureField)
            );
            assert_eq!(eligibility(role, SubroleEvidence::Value(false)), Ok(()));
        }
    }

    #[test]
    fn replacement_capabilities_work_without_enabled_but_never_override_explicit_vetoes() {
        for role in [
            FieldRole::TextField,
            FieldRole::TextArea,
            FieldRole::ComboBox,
        ] {
            let mut field = FieldEvidence {
                role,
                secure: Some(false),
                enabled: None,
                value_settable: Some(true),
                selected_text_settable: None,
            };
            assert_eq!(field.eligibility(), Ok(()));
            field.value_settable = Some(false);
            field.selected_text_settable = Some(true);
            assert_eq!(field.eligibility(), Ok(()));
            field.enabled = Some(false);
            assert_eq!(field.eligibility(), Err(HoldReason::NotEditable));
            field.enabled = None;
            field.secure = Some(true);
            assert_eq!(field.eligibility(), Err(HoldReason::SecureField));
            field.secure = None;
            assert_eq!(field.eligibility(), Err(HoldReason::UnconfirmedField));
            field.secure = Some(false);
            field.selected_text_settable = None;
            assert_eq!(field.eligibility(), Err(HoldReason::NotEditable));
            field.value_settable = None;
            assert_eq!(field.eligibility(), Err(HoldReason::UnknownEditability));
        }
    }

    #[test]
    fn only_positive_editability_evidence_allows_supported_fields() {
        for role in [
            FieldRole::TextField,
            FieldRole::TextArea,
            FieldRole::ComboBox,
        ] {
            let mut evidence = FieldEvidence {
                role,
                secure: Some(false),
                enabled: Some(true),
                value_settable: Some(true),
                selected_text_settable: None,
            };
            assert_eq!(evidence.eligibility(), Ok(()));
            evidence.value_settable = Some(false);
            assert_eq!(evidence.eligibility(), Err(HoldReason::NotEditable));
            evidence.value_settable = None;
            assert_eq!(evidence.eligibility(), Err(HoldReason::UnknownEditability));
            evidence.value_settable = Some(true);
            evidence.enabled = Some(false);
            assert_eq!(evidence.eligibility(), Err(HoldReason::NotEditable));
            evidence.enabled = None;
            assert_eq!(evidence.eligibility(), Ok(()));
            evidence.enabled = Some(true);
            evidence.secure = Some(true);
            assert_eq!(evidence.eligibility(), Err(HoldReason::SecureField));
            evidence.secure = None;
            assert_eq!(evidence.eligibility(), Err(HoldReason::UnconfirmedField));
        }
        assert_eq!(
            FieldEvidence {
                role: FieldRole::Other,
                secure: Some(false),
                enabled: Some(true),
                value_settable: Some(true),
                selected_text_settable: None,
            }
            .eligibility(),
            Err(HoldReason::UnconfirmedField)
        );
    }
}

#[cfg(test)]
mod post_send_tests {
    use super::*;
    use worker::PostSendObservation::*;

    #[test]
    fn character_count_preflight_rejects_invalid_values_without_reading_text() {
        for count in [
            Err(TargetError::Accessibility(-25205)),
            Ok(CFString::new("4").as_CFType()),
            Ok(CFNumber::from(-1_i64).as_CFType()),
            Ok(CFNumber::from((MAX_OBSERVED_UNITS + 1) as i64).as_CFType()),
            Ok(CFNumber::from(1.5_f64).as_CFType()),
        ] {
            let mut reads = Vec::new();
            let result = bounded_text_value(|name| {
                reads.push(name);
                count.clone()
            });
            assert!(result.is_none());
            assert_eq!(reads, ["AXNumberOfCharacters"]);
        }
    }

    #[test]
    fn bounded_character_count_allows_text_and_rechecks_actual_value() {
        for (value, accepted) in [
            ("Ä😀".to_owned(), true),
            ("x".repeat(MAX_OBSERVED_UNITS + 1), false),
        ] {
            let mut reads = Vec::new();
            let result = bounded_text_value(|name| {
                reads.push(name);
                match name {
                    "AXNumberOfCharacters" => Ok(CFNumber::from(3_i64).as_CFType()),
                    "AXValue" => Ok(CFString::new(&value).as_CFType()),
                    _ => Err(TargetError::Value),
                }
            });
            assert_eq!(result.is_some(), accepted);
            assert_eq!(reads, ["AXNumberOfCharacters", "AXValue"]);
        }
    }

    #[test]
    fn exact_utf16_replacement_confirms_only_expected_result() {
        let text: Vec<u16> = "Ä😀 alt Ende".encode_utf16().collect();
        let snapshot = TextSnapshot {
            observed_at: Instant::now(),
            text,
            selection: 4..7,
        };
        assert_eq!(
            snapshot.compare(
                &"Ä😀 neu\n42 Ende".encode_utf16().collect::<Vec<_>>(),
                "neu\n42"
            ),
            Confirmed
        );
        assert_eq!(
            snapshot.compare(
                &"Ä😀 alt Ende".encode_utf16().collect::<Vec<_>>(),
                "neu\n42"
            ),
            Uncertain
        );
        assert_eq!(
            snapshot.compare(
                &"irgendeine Änderung".encode_utf16().collect::<Vec<_>>(),
                "neu\n42"
            ),
            Uncertain
        );
    }

    #[test]
    fn no_op_or_oversized_replacement_cannot_confirm_delivery() {
        let snapshot = TextSnapshot {
            observed_at: Instant::now(),
            text: "alt".encode_utf16().collect(),
            selection: 0..3,
        };
        assert_eq!(snapshot.compare(&snapshot.text, "alt"), Unverifiable);
        assert_eq!(
            snapshot.compare(&[], &"x".repeat(MAX_OBSERVED_BYTES + 1)),
            Unverifiable
        );
    }
}
