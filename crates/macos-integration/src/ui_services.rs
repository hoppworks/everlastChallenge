//! Explicit UI actions; authorization reads never prompt or start recording.

use block2::RcBlock;
use core_foundation::{
    base::TCFType,
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use objc2::{MainThreadMarker, runtime::Bool};
use objc2_app_kit::NSPasteboard;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_foundation::NSString;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};

pub const MAX_COPY_BYTES: usize = 256 * 1024;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicAuthorization {
    NotDetermined,
    Restricted,
    Denied,
    Granted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionSnapshot {
    pub microphone: MicAuthorization,
    pub accessibility_trusted: bool,
    pub secure_input_active: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UiServicesError {
    #[error("This operation requires the native main thread")]
    MainThread,
    #[error("Text is empty or exceeds the copy limit")]
    InvalidText,
    #[error("The audio media type is unavailable")]
    Unavailable,
    #[error("The clipboard could not accept the text")]
    Clipboard,
}

fn authorization(value: AVAuthorizationStatus) -> MicAuthorization {
    match value {
        AVAuthorizationStatus::NotDetermined => MicAuthorization::NotDetermined,
        AVAuthorizationStatus::Restricted => MicAuthorization::Restricted,
        AVAuthorizationStatus::Denied => MicAuthorization::Denied,
        AVAuthorizationStatus::Authorized => MicAuthorization::Granted,
        _ => MicAuthorization::Unknown,
    }
}

/// Read-only and callable from workers. It neither requests access nor opens a
/// capture device. Unknown future native status values fail closed.
pub fn microphone_authorization() -> MicAuthorization {
    // SAFETY: AVMediaTypeAudio is a framework-owned immutable constant. Only
    // the documented audio media type is passed, avoiding invalid-type exceptions.
    unsafe {
        AVMediaTypeAudio
            .map(|audio| authorization(AVCaptureDevice::authorizationStatusForMediaType(audio)))
            .unwrap_or(MicAuthorization::Unknown)
    }
}

pub fn permission_snapshot() -> PermissionSnapshot {
    PermissionSnapshot {
        microphone: microphone_authorization(),
        accessibility_trusted: crate::target::accessibility_trusted(),
        secure_input_active: crate::target::secure_input_active(),
    }
}

type ReplyBlock = RcBlock<dyn Fn(Bool)>;

fn send_reply(sender: &SyncSender<MicAuthorization>, authorization: MicAuthorization) {
    // A repeated native callback or an abandoned receiver must never block a
    // dispatch queue. The first result wins, and all later results are discarded.
    match sender.try_send(authorization) {
        Ok(())
        | Err(std::sync::mpsc::TrySendError::Full(_))
        | Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {}
    }
}

fn microphone_reply() -> (ReplyBlock, Receiver<MicAuthorization>) {
    let (sender, receiver) = sync_channel(1);
    let completed = AtomicBool::new(false);
    let block = RcBlock::new(move |granted: Bool| {
        if !completed.swap(true, Ordering::Relaxed) {
            send_reply(
                &sender,
                if granted.as_bool() {
                    MicAuthorization::Granted
                } else {
                    MicAuthorization::Denied
                },
            );
        }
    });
    (block, receiver)
}

/// Explicit user action only. Invoke on the native main thread; the permission
/// dialog completes asynchronously on Apple's dispatch queue. Receive on a
/// worker or poll with try_recv; never wait on the GUI thread.
pub fn request_microphone_access() -> Result<Receiver<MicAuthorization>, UiServicesError> {
    let _main = MainThreadMarker::new().ok_or(UiServicesError::MainThread)?;
    let current = microphone_authorization();
    if current == MicAuthorization::Unknown {
        return Err(UiServicesError::Unavailable);
    }
    if current != MicAuthorization::NotDetermined {
        let (sender, receiver) = sync_channel(1);
        send_reply(&sender, current);
        return Ok(receiver);
    }
    let (block, receiver) = microphone_reply();
    // SAFETY: the media type is the framework-owned audio constant. The block
    // owns a Send + Sync bounded sender, captures no GUI objects, and AVFoundation
    // copies the completion block for asynchronous invocation before returning.
    unsafe {
        let audio = AVMediaTypeAudio.ok_or(UiServicesError::Unavailable)?;
        AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &block);
    }
    Ok(receiver)
}

/// Explicit user action only. The return value is current trust, not approval
/// of the asynchronous system prompt. Refresh the snapshot on application focus.
pub fn request_accessibility_access() -> Result<bool, UiServicesError> {
    let _main = MainThreadMarker::new().ok_or(UiServicesError::MainThread)?;
    // SAFETY: Apple's exported key is a non-null, framework-owned CFString.
    // Get-rule ownership retains it while the dictionary and call use it.
    let prompt_key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(prompt_key, CFBoolean::true_value())]);
    // SAFETY: the dictionary contains Apple's documented key and CFBoolean,
    // lives across the call, and no pointer escapes this synchronous invocation.
    Ok(unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 })
}

/// Copies plain text only after an explicit user action. Intentionally replaces
/// the general clipboard; performs no automatic paste or clipboard restoration.
/// If the OS rejects the write after clearContents, the clipboard can be empty.
pub fn manual_copy(text: &str) -> Result<(), UiServicesError> {
    if text.is_empty() || text.len() > MAX_COPY_BYTES {
        return Err(UiServicesError::InvalidText);
    }
    let _main = MainThreadMarker::new().ok_or(UiServicesError::MainThread)?;
    let string = NSString::from_str(text);
    let data_type = NSString::from_str("public.utf8-plain-text");
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    if pasteboard.setString_forType(&string, &data_type) {
        Ok(())
    } else {
        Err(UiServicesError::Clipboard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_copy_refuses_worker_threads_without_touching_the_clipboard()
    -> Result<(), Box<dyn std::error::Error>> {
        let result = std::thread::spawn(|| manual_copy("not written"))
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(result, Err(UiServicesError::MainThread));
        Ok(())
    }

    #[test]
    fn permission_completion_is_one_shot_even_after_the_first_result_was_consumed()
    -> Result<(), Box<dyn std::error::Error>> {
        let (reply, result) = microphone_reply();
        reply.call((Bool::YES,));
        assert_eq!(result.try_recv()?, MicAuthorization::Granted);
        reply.call((Bool::NO,));
        assert!(matches!(
            result.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        drop(reply);
        assert!(matches!(
            result.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
        Ok(())
    }

    #[test]
    fn prompt_actions_refuse_workers_before_invoking_system_prompts()
    -> Result<(), Box<dyn std::error::Error>> {
        let (microphone, accessibility) = std::thread::spawn(|| {
            (
                request_microphone_access().err(),
                request_accessibility_access().err(),
            )
        })
        .join()
        .map_err(|_| "worker panicked")?;
        assert_eq!(microphone, Some(UiServicesError::MainThread));
        assert_eq!(accessibility, Some(UiServicesError::MainThread));
        Ok(())
    }

    #[test]
    fn manual_copy_bounds_utf8_bytes_before_any_native_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let result = std::thread::spawn(|| {
            let boundary = "ä".repeat(MAX_COPY_BYTES / 2);
            (
                manual_copy(""),
                manual_copy(&boundary),
                manual_copy(&(boundary + "x")),
            )
        })
        .join()
        .map_err(|_| "worker panicked")?;
        assert_eq!(
            result,
            (
                Err(UiServicesError::InvalidText),
                Err(UiServicesError::MainThread),
                Err(UiServicesError::InvalidText),
            )
        );
        Ok(())
    }

    #[test]
    fn native_authorization_values_are_explicit_and_unknown_values_fail_closed() {
        assert_eq!(
            authorization(AVAuthorizationStatus(0)),
            MicAuthorization::NotDetermined
        );
        assert_eq!(
            authorization(AVAuthorizationStatus(1)),
            MicAuthorization::Restricted
        );
        assert_eq!(
            authorization(AVAuthorizationStatus(2)),
            MicAuthorization::Denied
        );
        assert_eq!(
            authorization(AVAuthorizationStatus(3)),
            MicAuthorization::Granted
        );
        assert_eq!(
            authorization(AVAuthorizationStatus(4)),
            MicAuthorization::Unknown
        );
        assert_eq!(
            authorization(AVAuthorizationStatus(-1)),
            MicAuthorization::Unknown
        );
    }

    #[test]
    fn completion_is_bounded_and_tolerates_an_abandoned_receiver()
    -> Result<(), Box<dyn std::error::Error>> {
        let (reply, result) = microphone_reply();
        reply.call((Bool::NO,));
        for _ in 0..100 {
            reply.call((Bool::YES,));
        }
        assert_eq!(result.try_recv()?, MicAuthorization::Denied);
        drop(result);
        reply.call((Bool::YES,));
        let (reply, result) = microphone_reply();
        drop(result);
        reply.call((Bool::YES,));
        Ok(())
    }
}
