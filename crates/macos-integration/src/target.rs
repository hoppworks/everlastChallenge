//! Read-only target validation; no activation, clipboard mutation or typing.

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSWorkspace;
use std::{
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
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> CFTypeRef;
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

#[derive(Debug, thiserror::Error)]
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

    fn focused(&self, pid: libc::pid_t) -> Result<Self, TargetError> {
        let element = Self::checked(self.attribute("AXFocusedUIElement")?)?;
        let mut observed = 0;
        // SAFETY: element is a live, checked AX reference; output is writable.
        ax_result(unsafe { AXUIElementGetPid(element.raw(), &mut observed) })?;
        if observed != pid {
            return Err(TargetError::Changed);
        }
        Ok(element)
    }

    fn evidence(&self) -> Result<FieldEvidence, TargetError> {
        let role = self.string("AXRole")?;
        let role = if role == CFString::from_static_string("AXTextField") {
            FieldRole::TextField
        } else if role == CFString::from_static_string("AXTextArea") {
            FieldRole::TextArea
        } else if role == CFString::from_static_string("AXComboBox") {
            FieldRole::ComboBox
        } else {
            FieldRole::Other
        };
        let secure = self
            .string("AXSubrole")
            .ok()
            .map(|subrole| subrole == CFString::from_static_string("AXSecureTextField"));
        let enabled = self
            .attribute("AXEnabled")
            .ok()
            .and_then(|value| value.downcast_into::<CFBoolean>())
            .map(bool::from);
        let key = CFString::from_static_string("AXValue");
        let mut settable = 0;
        // SAFETY: reference/key are live; output is writable. Failure is kept
        // as unknown editability, never converted to a positive permission.
        let status = unsafe {
            AXUIElementIsAttributeSettable(self.raw(), key.as_concrete_TypeRef(), &mut settable)
        };
        Ok(FieldEvidence {
            role,
            secure,
            enabled,
            value_settable: (status == 0).then_some(settable != 0),
        })
    }
}

fn ax_result(status: i32) -> Result<(), TargetError> {
    if status == 0 {
        Ok(())
    } else {
        Err(TargetError::Accessibility(status))
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
        let field = application.focused(process.pid)?;
        field
            .evidence()?
            .eligibility()
            .map_err(TargetError::Field)?;
        Ok(Self {
            session,
            process,
            application,
            field,
        })
    }

    /// Caller must obtain a fresh frontmost identity immediately before this
    /// check. Never activates an application or finds a replacement field.
    pub fn verify(&self, frontmost: ProcessIdentity) -> Result<(), TargetError> {
        check_environment()?;
        if frontmost != self.process || ProcessIdentity::read(self.process.pid)? != self.process {
            return Err(TargetError::Changed);
        }
        let focused = self.application.focused(self.process.pid)?;
        if focused.value != self.field.value {
            return Err(TargetError::Changed);
        }
        focused
            .evidence()?
            .eligibility()
            .map_err(TargetError::Field)
    }

    /// The receipt is single-use and carries no native references. It may cross
    /// to the main thread for one mutation or send check, within 250 ms of the
    /// start of this verification (including AX messaging time).
    pub fn approve(&self, frontmost: ProcessIdentity) -> Result<TargetApproval, TargetError> {
        let checked_at = Instant::now();
        self.verify(frontmost)?;
        Ok(TargetApproval {
            session: self.session,
            process: self.process,
            checked_at,
        })
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

pub struct FieldEvidence {
    pub role: FieldRole,
    pub secure: Option<bool>,
    pub enabled: Option<bool>,
    pub value_settable: Option<bool>,
}

impl FieldEvidence {
    pub fn eligibility(&self) -> Result<(), HoldReason> {
        if self.role == FieldRole::Other || self.secure.is_none() {
            return Err(HoldReason::UnconfirmedField);
        }
        if self.secure == Some(true) {
            return Err(HoldReason::SecureField);
        }
        match (self.enabled, self.value_settable) {
            (Some(false), _) | (_, Some(false)) => Err(HoldReason::NotEditable),
            (Some(true), Some(true)) => Ok(()),
            _ => Err(HoldReason::UnknownEditability),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(evidence.eligibility(), Err(HoldReason::UnknownEditability));
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
                value_settable: Some(true)
            }
            .eligibility(),
            Err(HoldReason::UnconfirmedField)
        );
    }
}
