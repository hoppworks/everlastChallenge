//! Read-only keyboard layout resolution. Never selects a layout or posts input.
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    data::CFData,
    string::{CFString, CFStringRef},
};
use core_graphics::{event::CGEventFlags, event_source::CGEventSourceStateID};
use objc2::MainThreadMarker;
use std::{ffi::c_void, marker::PhantomData, rc::Rc};

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> CFTypeRef;
    fn TISGetInputSourceProperty(source: CFTypeRef, property: CFStringRef) -> *const c_void;
    static kTISPropertyInputSourceID: CFStringRef;
    static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    fn LMGetKbdType() -> u8;
    fn UCKeyTranslate(
        layout: *const c_void,
        key: u16,
        action: u16,
        modifiers: u32,
        keyboard_type: u32,
        options: u32,
        dead_state: *mut u32,
        capacity: libc::c_ulong,
        length: *mut libc::c_ulong,
        output: *mut u16,
    ) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceFlagsState(state: CGEventSourceStateID) -> u64;
    fn CGEventSourceKeyState(state: CGEventSourceStateID, key: u16) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum KeyboardError {
    #[error("The current keyboard layout has no unambiguous Command+V mapping")]
    Unsupported,
}

pub struct PasteKey {
    key_code: u16,
    layout: Layout,
}

struct Layout {
    source_id: CFString,
    data: CFData,
    keyboard_type: u32,
    _main_thread: PhantomData<Rc<()>>,
}

impl Layout {
    fn current(_main_thread: MainThreadMarker) -> Result<Self, KeyboardError> {
        // SAFETY: called only on main; Copy returns an owned TIS/CF reference.
        let raw = unsafe { TISCopyCurrentKeyboardLayoutInputSource() };
        if raw.is_null() {
            return Err(KeyboardError::Unsupported);
        }
        // SAFETY: non-null Copy-rule reference from the OS, released by CFType.
        let source = unsafe { CFType::wrap_under_create_rule(raw) };
        // SAFETY: the exported immutable property keys and source are valid.
        let id =
            unsafe { TISGetInputSourceProperty(source.as_CFTypeRef(), kTISPropertyInputSourceID) };
        // SAFETY: as above; this property is the OS's active uchr layout data.
        let data = unsafe {
            TISGetInputSourceProperty(source.as_CFTypeRef(), kTISPropertyUnicodeKeyLayoutData)
        };
        if id.is_null() || data.is_null() {
            return Err(KeyboardError::Unsupported);
        }
        // SAFETY: properties use Get ownership; retain while source is alive,
        // then check actual types before interpreting their contents.
        let id = unsafe { CFType::wrap_under_get_rule(id) };
        // SAFETY: same Get-rule lifetime as id; wrapped reference retains data.
        let data = unsafe { CFType::wrap_under_get_rule(data) };
        let source_id = id
            .downcast_into::<CFString>()
            .ok_or(KeyboardError::Unsupported)?;
        let data = data
            .downcast_into::<CFData>()
            .ok_or(KeyboardError::Unsupported)?;
        if data.is_empty() {
            return Err(KeyboardError::Unsupported);
        }
        // SAFETY: main-thread-only call satisfies this legacy API's lack of
        // thread safety. No keyboard or input-source setting is changed.
        let keyboard_type = u32::from(unsafe { LMGetKbdType() });
        Ok(Self {
            source_id,
            data,
            keyboard_type,
            _main_thread: PhantomData,
        })
    }
}

impl PasteKey {
    pub fn resolve(main_thread: MainThreadMarker) -> Result<Self, KeyboardError> {
        let layout = Layout::current(main_thread)?;
        let mut matching = None;
        for key_code in 0..128 {
            let mut dead_state = 0;
            let mut length: libc::c_ulong = 0;
            let mut output = [0_u16; 255];
            // SAFETY: data is the retained immutable uchr resource supplied by
            // the current OS layout. Scalars follow UnicodeUtilities.h:
            // action Down=0, Command modifier (cmdKey >> 8)=1, no-dead-keys=1.
            // State/length/output point to writable storage of capacity 255.
            let status = unsafe {
                UCKeyTranslate(
                    layout.data.bytes().as_ptr().cast(),
                    key_code,
                    0,
                    1,
                    layout.keyboard_type,
                    1,
                    &mut dead_state,
                    255,
                    &mut length,
                    output.as_mut_ptr(),
                )
            };
            if status != 0 {
                return Err(KeyboardError::Unsupported);
            }
            if length == 1
                && matches!(output[0], 0x76 | 0x56)
                && matching.replace(key_code).is_some()
            {
                return Err(KeyboardError::Unsupported);
            }
        }
        Ok(Self {
            key_code: matching.ok_or(KeyboardError::Unsupported)?,
            layout,
        })
    }

    pub fn key_code(&self) -> u16 {
        self.key_code
    }

    /// Recheck immediately before mutation and again before the send commit.
    pub fn is_current(&self, main_thread: MainThreadMarker) -> Result<bool, KeyboardError> {
        let current = Layout::current(main_thread)?;
        Ok(current.source_id == self.layout.source_id
            && current.data == self.layout.data
            && current.keyboard_type == self.layout.keyboard_type)
    }
}

/// Physical hardware state excludes synthetic CombinedSession events.
/// Caps Lock and NumPad are modes rather than held modifiers, so are ignored.
pub fn modifiers_held() -> bool {
    // SAFETY: read-only system state query without pointers or side effects.
    let flags = CGEventFlags::from_bits_retain(unsafe {
        CGEventSourceFlagsState(CGEventSourceStateID::HIDSystemState)
    });
    flags.intersects(
        CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagSecondaryFn
            | CGEventFlags::CGEventFlagHelp,
    )
}

/// Current physical Escape state, including while no global shortcut is registered.
/// This read-only Quartz query neither requests permissions nor synthesizes input.
pub fn escape_key_pressed() -> bool {
    // SAFETY: CGEventSource.h declares (CGEventSourceStateID, CGKeyCode=u16) -> bool.
    // Events.h defines layout-independent kVK_Escape=0x35. The query takes no
    // pointers and reads shared HID state without owning an AppKit object.
    unsafe { CGEventSourceKeyState(CGEventSourceStateID::HIDSystemState, 0x35) }
}

mod labels;
pub use labels::{KeyLabels, LETTER_CODES};
mod observer;
pub use observer::{LayoutObserver, ObserverError};
