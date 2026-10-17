//! Uses only a newly created private pasteboard, never the general clipboard.
use macos_integration::{
    clipboard::{
        ClipboardError, PreparedClipboard, RestoreOutcome, capture_bounded, snapshot_helper_main,
    },
    delivery::DeliveryToken,
};
use objc2::{MainThreadMarker, runtime::ProtocolObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};
use std::num::NonZeroU64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = snapshot_helper_main() {
        return result.map_err(Into::into);
    }
    let helper = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)?;
    let main = MainThreadMarker::new().ok_or("not on main thread")?;
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    let item = NSPasteboardItem::new();
    if !item.setData_forType(
        &NSData::with_bytes(b"fixture text"),
        &NSString::from_str("public.utf8-plain-text"),
    ) || !item.setData_forType(
        &NSData::with_bytes(b"<b>fixture text</b>"),
        &NSString::from_str("public.html"),
    ) {
        return Err("could not prepare private fixture".into());
    }
    let objects =
        NSArray::from_slice(&[ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&*item)]);
    if !pasteboard.writeObjects(&objects) {
        return Err("could not write private fixture".into());
    }
    let token = DeliveryToken::new(NonZeroU64::new(1).ok_or("invalid id")?);
    let backup = capture_bounded(&helper, &pasteboard.name().to_string(), &token)?;
    if backup.item_count() != 1
        || backup.data(0, "public.utf8-plain-text") != Some(b"fixture text".as_slice())
        || backup.data(0, "public.html") != Some(b"<b>fixture text</b>".as_slice())
    {
        return Err("private snapshot lost an item or representation".into());
    }
    let prepared = PreparedClipboard::new(
        &pasteboard.name().to_string(),
        backup,
        "dictated fixture",
        main,
    )?;
    let lease = prepared.install(&token, main)?;
    if !lease.is_current() {
        return Err("lease lost immediately".into());
    }
    if pasteboard
        .stringForType(&NSString::from_str("public.utf8-plain-text"))
        .map(|value| value.to_string())
        .as_deref()
        != Some("dictated fixture")
    {
        return Err("replacement missing".into());
    }
    if lease.restore(main)? != RestoreOutcome::Restored {
        return Err("original not restored".into());
    }
    if pasteboard
        .stringForType(&NSString::from_str("public.utf8-plain-text"))
        .map(|value| value.to_string())
        .as_deref()
        != Some("fixture text")
        || pasteboard
            .stringForType(&NSString::from_str("public.html"))
            .map(|value| value.to_string())
            .as_deref()
            != Some("<b>fixture text</b>")
    {
        return Err("restored data differs".into());
    }
    let name = pasteboard.name().to_string();
    let backup = capture_bounded(&helper, &name, &token)?;
    let lease = PreparedClipboard::new(&name, backup, "another dictated fixture", main)?
        .install(&token, main)?;
    pasteboard.clearContents();
    let foreign = NSPasteboardItem::new();
    if !foreign.setString_forType(
        &NSString::from_str("foreign change"),
        &NSString::from_str("public.utf8-plain-text"),
    ) {
        return Err("foreign fixture could not be prepared".into());
    }
    if !pasteboard.writeObjects(&NSArray::from_slice(&[ProtocolObject::<
        dyn NSPasteboardWriting,
    >::from_ref(&*foreign)]))
    {
        return Err("foreign fixture could not be written".into());
    }
    let changed_count = pasteboard.changeCount();
    if lease.restore(main)? != RestoreOutcome::OwnershipChanged
        || pasteboard.changeCount() != changed_count
    {
        return Err("foreign change was overwritten".into());
    }
    if pasteboard
        .stringForType(&NSString::from_str("public.utf8-plain-text"))
        .map(|value| value.to_string())
        .as_deref()
        != Some("foreign change")
    {
        return Err("foreign content was not preserved".into());
    }
    let backup = capture_bounded(&helper, &name, &token)?;
    let prepared = PreparedClipboard::new(&name, backup, "cancelled text", main)?;
    token.cancel();
    if !matches!(
        prepared.install(&token, main),
        Err(ClipboardError::Cancelled)
    ) || pasteboard.changeCount() != changed_count
    {
        return Err("cancelled install changed pasteboard".into());
    }
    pasteboard.clearContents();
    let empty_token = DeliveryToken::new(NonZeroU64::new(2).ok_or("invalid id")?);
    let empty_backup = capture_bounded(&helper, &name, &empty_token)?;
    if empty_backup.item_count() != 0 {
        return Err("empty private pasteboard was not empty".into());
    }
    let promise = NSPasteboardItem::new();
    if !promise.setData_forType(
        &NSData::with_bytes(b"public.plain-text"),
        &NSString::from_str("com.apple.pasteboard.promised-file-content-type"),
    ) {
        return Err("could not create promise fixture".into());
    }
    if !pasteboard.writeObjects(&NSArray::from_slice(&[ProtocolObject::<
        dyn NSPasteboardWriting,
    >::from_ref(&*promise)]))
    {
        return Err("could not publish promise fixture".into());
    }
    let promise_count = pasteboard.changeCount();
    if capture_bounded(&helper, &name, &empty_token).is_ok()
        || pasteboard.changeCount() != promise_count
    {
        return Err("file promise was not held without mutation".into());
    }
    pasteboard.clearContents();
    println!(
        "Private pasteboard: text/HTML restored; foreign change and pre-write cancellation preserved; general clipboard untouched"
    );
    Ok(())
}
