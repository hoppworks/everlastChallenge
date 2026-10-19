//! Opt-in read-only Notes diagnosis. Never reads text, prompts, activates or sends.
#![allow(
    unsafe_code,
    reason = "Bounded public AX FFI with Copy/Create ownership checked below"
)]
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use macos_integration::{
    delivery::DeliveryToken,
    target::{
        ProcessIdentity, accessibility_trusted, frontmost_process, secure_input_active,
        worker::{TargetEvent, TargetWorker},
    },
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSWorkspace;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> i32;
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
fn attribute(element: &CFType, name: &'static str) -> Result<CFType, String> {
    let key = CFString::from_static_string(name);
    let mut raw = std::ptr::null();
    // SAFETY: live CF element/key and writable output. Non-null Copy output is
    // owned even on an error, so wrap before checking the status.
    let code = unsafe {
        AXUIElementCopyAttributeValue(element.as_CFTypeRef(), key.as_concrete_TypeRef(), &mut raw)
    };
    let owned = if raw.is_null() {
        None
    } else {
        // SAFETY: non-null Copy-rule reference from the AX API.
        Some(unsafe { CFType::wrap_under_create_rule(raw) })
    };
    if code != 0 {
        return Err(match code {
            -25205 => "unsupported:-25205".into(),
            -25212 => "no-value:-25212".into(),
            _ => format!("ax-error:{code}"),
        });
    }
    owned.ok_or_else(|| "missing-value".into())
}
fn element(value: CFType) -> Result<CFType, String> {
    // SAFETY: public no-argument stable type identifier.
    if value.type_of() != unsafe { AXUIElementGetTypeID() } {
        return Err("unexpected-element-type".into());
    }
    // SAFETY: checked live AX element; timeout changes only messaging for this reference.
    let code = unsafe { AXUIElementSetMessagingTimeout(value.as_CFTypeRef(), 0.25) };
    if code != 0 {
        return Err(format!("timeout-error:{code}"));
    }
    Ok(value)
}
fn diagnosis(process: ProcessIdentity) -> Result<(), String> {
    if !accessibility_trusted() {
        return Err("permission-missing".into());
    }
    if ProcessIdentity::read(process.pid()).map_err(|_| "process-unavailable")? != process {
        return Err("process-changed".into());
    }
    // SAFETY: positive validated PID, Create result checked for null before ownership.
    let raw = unsafe { AXUIElementCreateApplication(process.pid()) };
    if raw.is_null() {
        return Err("missing-application".into());
    }
    // SAFETY: non-null Create-rule reference from AX.
    let app = element(unsafe { CFType::wrap_under_create_rule(raw) })?;
    let field = element(attribute(&app, "AXFocusedUIElement")?)?;
    let mut pid = 0;
    // SAFETY: checked element and writable scalar; no contents requested.
    let code = unsafe { AXUIElementGetPid(field.as_CFTypeRef(), &mut pid) };
    if code != 0 || pid != process.pid() {
        return Err("focused-process-unconfirmed".into());
    }
    let role = attribute(&field, "AXRole")?
        .downcast_into::<CFString>()
        .ok_or("role-unexpected-type")?;
    let role = if role == CFString::from_static_string("AXTextArea") {
        "text-area"
    } else if role == CFString::from_static_string("AXTextField") {
        "text-field"
    } else if role == CFString::from_static_string("AXComboBox") {
        "combo-box"
    } else {
        "other"
    };
    println!("role={role}");
    match attribute(&field, "AXSubrole") {
        Ok(value) => match value.downcast_into::<CFString>() {
            Some(value) if value == CFString::from_static_string("AXSecureTextField") => {
                println!("subrole=secure")
            }
            Some(_) => println!("subrole=supported-nonsecure"),
            None => println!("subrole=unexpected-type"),
        },
        Err(status) => println!("subrole={status}"), // -25205 unsupported; -25212 no value.
    }
    match attribute(&field, "AXEnabled") {
        Ok(value) => match value.downcast_into::<CFBoolean>() {
            Some(value) => println!("enabled={}", bool::from(value)),
            None => println!("enabled=unexpected-type"),
        },
        Err(status) => println!("enabled={status}"),
    }
    let key = CFString::from_static_string("AXValue");
    let mut settable = 0;
    // SAFETY: asks only whether AXValue is settable; never copies its value.
    let code = unsafe {
        AXUIElementIsAttributeSettable(
            field.as_CFTypeRef(),
            key.as_concrete_TypeRef(),
            &mut settable,
        )
    };
    if code == 0 {
        println!("valueSettable={}", settable != 0);
    } else {
        println!("valueSettable=ax-error:{code}");
    }
    if ProcessIdentity::read(process.pid()).map_err(|_| "process-unavailable")? != process {
        return Err("process-changed".into());
    }
    Ok(())
}
fn notes(main: MainThreadMarker) -> Result<ProcessIdentity, Box<dyn std::error::Error>> {
    let app = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .ok_or("no frontmost application")?;
    if app
        .bundleIdentifier()
        .is_none_or(|id| id.to_string() != "com.apple.Notes")
    {
        return Err("Notes must already be frontmost".into());
    }
    let process = frontmost_process(main)?;
    if process.pid() != app.processIdentifier() {
        return Err("frontmost application changed during identity read".into());
    }
    Ok(process)
}
fn response(worker: &TargetWorker) -> Result<TargetEvent, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(event) = worker.next_event()? {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err("target response timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    if !accessibility_trusted() {
        return Err("Accessibility permission missing; no prompt requested".into());
    }
    if secure_input_active() {
        return Err("Secure Input active".into());
    }
    let process = notes(main)?;
    println!(
        "pid={};trusted=true;secureInput=false;readsText=false",
        process.pid()
    );
    let diagnostic = std::thread::Builder::new()
        .name("notes-ax-diagnostic".into())
        .spawn(move || diagnosis(process))?;
    diagnostic
        .join()
        .map_err(|_| "diagnostic thread failed")?
        .map_err(|status| format!("AX diagnosis failed: {status}"))?;
    if notes(main)? != process {
        return Err("frontmost Notes changed".into());
    }
    let worker = TargetWorker::start()?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let session = NonZeroU64::new(1).ok_or("invalid session")?;
        worker.capture(session, process, DeliveryToken::new(session))?;
        match response(&worker)? {
            TargetEvent::Captured {
                session: observed,
                result,
            } if observed == session => {
                println!("capture={result:?}");
                result?;
            }
            _ => return Err("unexpected capture event".into()),
        }
        worker.approve(session, session, notes(main)?)?;
        match response(&worker)? {
            TargetEvent::Approved {
                session: observed,
                request,
                result,
            } if observed == session && request == session => match result {
                Ok(receipt) => {
                    receipt.validate(session, notes(main)?)?;
                    println!("approve=ok");
                }
                Err(error) => {
                    println!("approve={error:?}");
                    return Err(error.into());
                }
            },
            _ => return Err("unexpected approval event".into()),
        }
        Ok(())
    })();
    let cleanup = worker.shutdown();
    result?;
    cleanup?;
    Ok(())
}
