//! Own-control metadata diagnostic. No AX client calls, focus, values or TCC.
#![allow(
    unsafe_code,
    reason = "Creates only an invisible owned AppKit window on its main thread"
)]
use core_foundation::base::{CFType, TCFType};
use macos_integration::target::{FieldEvidence, FieldRole, HoldReason};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBackingStoreType, NSObjectNSAccessibility, NSSecureTextField,
    NSTextField, NSTextView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSNumber, NSObject, NSPoint, NSRect, NSSize, NSString};

#[derive(Debug, PartialEq, Eq)]
struct BridgeMetadata {
    modern_enabled: bool,
    enabled: Option<bool>,
    value_settable: bool,
}

#[allow(
    deprecated,
    reason = "Observe own-control compatibility metadata without an AX client"
)]
fn bridged_metadata<T: AsRef<NSObject> + NSAccessibility + objc2::Message>(
    name: &str,
    view: &T,
) -> BridgeMetadata {
    let enabled = view
        .as_ref()
        .accessibilityAttributeValue(&NSString::from_str("AXEnabled"));
    let number = enabled
        .as_ref()
        .and_then(|value| value.downcast_ref::<NSNumber>());
    let kind = number.map(|value| {
        // SAFETY: NSNumber is toll-free bridged to a CoreFoundation number or
        // boolean. Get-rule retain is scoped to this own-control observation.
        let cf = unsafe { CFType::wrap_under_get_rule((value as *const NSNumber).cast()) };
        cf.type_of()
    });
    let writable = view
        .as_ref()
        .accessibilityIsAttributeSettable(&NSString::from_str("AXValue"));
    println!(
        "ownedBridge={name}; modernEnabled={}; enabledPresent={}; enabledValue={:?}; enabledCFType={kind:?}; valueSettable={writable}",
        view.isAccessibilityEnabled(),
        enabled.is_some(),
        number.map(|value| value.boolValue())
    );
    BridgeMetadata {
        modern_enabled: view.isAccessibilityEnabled(),
        enabled: number.map(|value| value.boolValue()),
        value_settable: writable,
    }
}

fn evidence(name: &str, view: &NSView, editable: bool) -> FieldEvidence {
    let role = view.accessibilityRole().map(|value| value.to_string());
    let subrole = view.accessibilitySubrole().map(|value| value.to_string());
    let enabled = view.isAccessibilityEnabled();
    println!(
        "ownControl={name}; role={role:?}; subrole={subrole:?}; enabled={enabled}; isEditable={editable}"
    );
    let role = match role.as_deref() {
        Some("AXTextArea") => FieldRole::TextArea,
        Some("AXTextField") => FieldRole::TextField,
        Some("AXComboBox") => FieldRole::ComboBox,
        _ => FieldRole::Other,
    };
    FieldEvidence {
        role,
        secure: subrole.map(|value| value == "AXSecureTextField"),
        enabled: Some(enabled),
        // This controlled positive premise isolates the subrole gate. Native
        // isEditable is observed above; it is NOT a cross-process AXValue
        // IsAttributeSettable measurement or proof of target delivery.
        value_settable: Some(editable),
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    let was_active = app.isActive();
    // SAFETY: designated initializer for a retained, never-shown own window.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(480., 320.)),
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Rust retains the window; prevent AppKit releasing it on close.
    unsafe {
        window.setReleasedWhenClosed(false);
    }
    let body = window.contentView().ok_or("content view missing")?;
    let multiline = NSTextView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(400., 160.)),
    );
    let singleline = NSTextField::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 180.), NSSize::new(400., 30.)),
    );
    let secure = NSSecureTextField::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 230.), NSSize::new(400., 30.)),
    );
    multiline.setEditable(true);
    singleline.setEditable(true);
    secure.setEditable(true);
    body.addSubview(&multiline);
    body.addSubview(&singleline);
    body.addSubview(&secure);
    let area = evidence("NSTextView", &multiline, multiline.isEditable());
    let normal = bridged_metadata("NSTextView/editable", &*multiline);
    multiline.setEditable(false);
    let readonly = bridged_metadata("NSTextView/readonly", &*multiline);
    multiline.setEditable(true);
    let restored = bridged_metadata("NSTextView/editable-restored", &*multiline);
    multiline.setAccessibilityEnabled(true);
    let enabled = bridged_metadata("NSTextView/accessibility-enabled", &*multiline);
    multiline.setAccessibilityEnabled(false);
    let disabled = bridged_metadata("NSTextView/accessibility-disabled", &*multiline);
    multiline.setAccessibilityEnabled(true);
    let reenabled = bridged_metadata("NSTextView/accessibility-reenabled", &*multiline);
    multiline.setEditable(false);
    let enabled_readonly =
        bridged_metadata("NSTextView/accessibility-enabled-readonly", &*multiline);
    multiline.setEditable(true);
    // Native editability changes are reflected by legacy AXValue writability.
    // An explicit accessibility-disabled state is NOT reflected by legacy
    // AXEnabled here: absent + writable is shared with the normal control.
    assert_eq!(normal.enabled, None);
    assert!(normal.value_settable);
    assert_eq!(readonly.enabled, None);
    assert!(!readonly.value_settable);
    assert_eq!(restored, normal);
    assert!(enabled.modern_enabled);
    assert!(!disabled.modern_enabled);
    assert_eq!(disabled, normal);
    assert_eq!(reenabled, enabled);
    assert_eq!(enabled.enabled, None);
    assert!(enabled.value_settable);
    assert!(enabled_readonly.modern_enabled);
    assert_eq!(enabled_readonly.enabled, None);
    assert!(!enabled_readonly.value_settable);
    let line = evidence("NSTextField", &singleline, singleline.isEditable());
    let password = evidence("NSSecureTextField", &secure, secure.isEditable());
    for (name, cell) in [
        ("NSTextField.cell", singleline.cell()),
        ("NSSecureTextField.cell", secure.cell()),
    ] {
        if let Some(cell) = cell {
            let bridge = bridged_metadata(name, &*cell);
            assert_eq!(bridge.enabled, Some(true));
            assert!(bridge.value_settable);
            println!(
                "ownControl={name}; role={:?}; subrole={:?}; enabled={}",
                cell.accessibilityRole().map(|v| v.to_string()),
                cell.accessibilitySubrole().map(|v| v.to_string()),
                cell.isAccessibilityEnabled()
            );
        }
    }
    println!(
        "projectedEligibility: area={:?}; line={:?}; secure={:?}",
        area.eligibility(),
        line.eligibility(),
        password.eligibility()
    );
    assert!(!window.isVisible());
    assert_eq!(app.isActive(), was_active);
    window.close();
    assert_eq!(area.role, FieldRole::TextArea);
    assert_eq!(
        area.secure, None,
        "this diagnostic hypothesis needs actual absent optional native subrole"
    );
    let ordinary_cell = singleline.cell().ok_or("text field cell missing")?;
    let secure_cell = secure.cell().ok_or("secure field cell missing")?;
    assert_eq!(
        ordinary_cell
            .accessibilityRole()
            .map(|v| v.to_string())
            .as_deref(),
        Some("AXTextField")
    );
    assert!(ordinary_cell.accessibilitySubrole().is_none());
    assert_eq!(
        secure_cell
            .accessibilitySubrole()
            .map(|v| v.to_string())
            .as_deref(),
        Some("AXSecureTextField")
    );
    // Preserve the safety meaning of unknown in the current FieldEvidence API.
    // Missing native optional subrole and failed AX transport currently collapse
    // into the same value: a safe future fix needs a richer reader boundary,
    // not a blanket eligibility(None) success. No AX transport status is guessed.
    assert_eq!(area.eligibility(), Err(HoldReason::UnconfirmedField));
    println!(
        "confirmed: readonly toggles legacy writability; explicit accessibility-disabled NSTextView remains absent-Enabled and writable, indistinguishable from normal by those two legacy fields; secure cell is marked; no cross-process AX status or Notes field queried"
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    objc2::rc::autoreleasepool(|_| run())
}
