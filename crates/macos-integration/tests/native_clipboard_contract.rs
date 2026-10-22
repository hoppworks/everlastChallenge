//! Private-pasteboard to owned offscreen text-input contract. No general clipboard or keys.
#![forbid(unsafe_code)]
use macos_integration::{
    clipboard::{
        ClipboardError, MAX_DATA_BYTES, MAX_ITEMS, MAX_TYPE_BYTES, MAX_TYPES, PreparedClipboard,
        RestoreOutcome, Snapshot,
    },
    delivery::DeliveryToken,
};
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSApplication, NSPasteboard, NSPasteboardItem, NSPasteboardWriting, NSTextView,
};
use objc2_foundation::{NSArray, NSData, NSPoint, NSRange, NSRect, NSSize, NSString};
use sha2::{Digest, Sha256};
use std::{
    num::NonZeroU64,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PRIVATE_PREFIX: &str = "org.everlast.clipboard-contract.";
const FOREIGN_SENTINEL: &[u8] = b"separate-own-writer: foreign sentinel 2026-10-19";
fn valid_writer_board(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(PRIVATE_PREFIX) else {
        return false;
    };
    let Some((pid, nonce)) = rest.split_once('.') else {
        return false;
    };
    !pid.is_empty()
        && pid.len() <= 10
        && pid.bytes().all(|b| b.is_ascii_digit())
        && pid.parse::<u32>().is_ok_and(|p| p > 0)
        && nonce.len() == 32
        && nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn background_writer_contract(main: MainThreadMarker) -> Result<(), Box<dyn std::error::Error>> {
    for invalid in [
        "NSGeneralPboard",
        "org.everlast.clipboard-contract.1.bad",
        "org.everlast.clipboard-contract.0.00000000000000000000000000000000",
    ] {
        assert!(!valid_writer_board(invalid));
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let name = format!("{PRIVATE_PREFIX}{}.{nonce:032x}", std::process::id());
    assert!(valid_writer_board(&name));
    let board = PrivateBoard(NSPasteboard::pasteboardWithName(&NSString::from_str(&name)));
    publish(&board.0, &[item(UTF8_TYPE, b"original before lease")?])?;
    let backup = Snapshot::capture(&name, main)?;
    let token = DeliveryToken::new(NonZeroU64::new(2).ok_or("session missing")?);
    let lease = PreparedClipboard::new(&name, backup, UNICODE, &token, main)?.install(main)?;
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--private-writer", &name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                drop(child.kill());
                drop(child.wait());
                return Err(error.into());
            }
        }
        if started.elapsed() >= Duration::from_secs(2) {
            drop(child.kill());
            drop(child.wait());
            return Err("private writer timed out and was reaped".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(status.success(), "separate private writer failed");
    assert!(!lease.is_current());
    let count = board.0.changeCount();
    assert_eq!(lease.restore(main)?, RestoreOutcome::OwnershipChanged);
    assert_eq!(
        board.0.changeCount(),
        count,
        "restore overwrote newer writer"
    );
    assert_eq!(
        Snapshot::capture(&name, main)?.data(0, UTF8_TYPE),
        Some(FOREIGN_SENTINEL)
    );
    Ok(())
}

const UNICODE: &str = "Grüße aus Köln: Äpfel, Öl, süß — 0123456789.\nZweite Zeile: 42 € 😀.";
const UTF8_TYPE: &str = "public.utf8-plain-text";
struct PrivateBoard(Retained<NSPasteboard>);
impl PrivateBoard {
    fn new() -> Self {
        Self(NSPasteboard::pasteboardWithUniqueName())
    }
}
impl Drop for PrivateBoard {
    fn drop(&mut self) {
        self.0.clearContents();
    }
}
fn publish(board: &NSPasteboard, items: &[Retained<NSPasteboardItem>]) -> Result<(), &'static str> {
    board.clearContents();
    let writers: Vec<_> = items
        .iter()
        .map(|item| ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&**item))
        .collect();
    if !board.writeObjects(&NSArray::from_slice(&writers)) {
        return Err("own pasteboard write failed");
    }
    Ok(())
}
fn item(name: &str, bytes: &[u8]) -> Result<Retained<NSPasteboardItem>, &'static str> {
    let item = NSPasteboardItem::new();
    if !item.setData_forType(&NSData::with_bytes(bytes), &NSString::from_str(name)) {
        return Err("own representation write failed");
    }
    Ok(item)
}

fn capture(board: &NSPasteboard, main: MainThreadMarker) -> Result<Snapshot, ClipboardError> {
    let change_count = board.changeCount();
    let result = Snapshot::capture(&board.name().to_string(), main);
    assert_eq!(
        board.changeCount(),
        change_count,
        "native backup mutated own pasteboard"
    );
    result
}
fn native_limits(main: MainThreadMarker) -> Result<(), Box<dyn std::error::Error>> {
    let board = PrivateBoard::new();
    for count in [MAX_TYPES, MAX_TYPES + 1] {
        {
            let item = NSPasteboardItem::new();
            for index in 0..count {
                assert!(item.setData_forType(
                    &NSData::with_bytes(&[]),
                    &NSString::from_str(&format!("org.everlast.limit.t{index}"))
                ));
            }
            assert_eq!(item.types().count(), count);
            publish(&board.0, &[item])?;
        }
        let result = capture(&board.0, main);
        if count == MAX_TYPES {
            let snapshot = result?;
            assert_eq!(snapshot.item_count(), 1);
            for index in 0..count {
                assert_eq!(
                    snapshot.data(0, &format!("org.everlast.limit.t{index}")),
                    Some([].as_slice())
                );
            }
        } else {
            assert!(
                matches!(result, Err(ClipboardError::Limit)),
                "native type overflow was not Limit"
            );
        }
    }
    for bytes in [MAX_TYPE_BYTES, MAX_TYPE_BYTES + 1] {
        // AppKit accepts ASCII UTI names; it rejects non-ASCII before capture.
        let prefix = "org.everlast.";
        let remaining = bytes - prefix.len();
        let name = format!("{prefix}{}", "x".repeat(remaining));
        assert_eq!(name.len(), bytes);
        publish(&board.0, &[item(&name, b"x")?])?;
        let result = capture(&board.0, main);
        if bytes == MAX_TYPE_BYTES {
            assert_eq!(result?.data(0, &name), Some(b"x".as_slice()));
        } else {
            assert!(
                matches!(result, Err(ClipboardError::Limit)),
                "native type-name overflow was not Limit"
            );
        }
    }
    for size in [MAX_DATA_BYTES, MAX_DATA_BYTES + 1] {
        objc2::rc::autoreleasepool(|_| -> Result<(), Box<dyn std::error::Error>> {
            {
                let bytes = vec![0x5a; size];
                let native_item = item("org.everlast.binary", &bytes)?;
                drop(bytes);
                publish(&board.0, &[native_item])?;
            }
            let result = capture(&board.0, main);
            if size == MAX_DATA_BYTES {
                let snapshot = result?;
                let data = snapshot
                    .data(0, "org.everlast.binary")
                    .ok_or("native bytes missing")?;
                assert_eq!(data.len(), MAX_DATA_BYTES);
                assert!(data.iter().all(|byte| *byte == 0x5a));
            } else {
                assert!(
                    matches!(result, Err(ClipboardError::Limit)),
                    "native data overflow was not Limit"
                );
            }
            board.0.clearContents();
            Ok(())
        })?;
    }
    for count in [MAX_ITEMS, MAX_ITEMS + 1] {
        {
            let items = (0..count)
                .map(|_| item("org.everlast.item", b"x"))
                .collect::<Result<Vec<_>, _>>()?;
            publish(&board.0, &items)?;
        }
        let result = capture(&board.0, main);
        if count == MAX_ITEMS {
            assert_eq!(result?.item_count(), MAX_ITEMS);
        } else {
            assert!(
                matches!(result, Err(ClipboardError::Limit)),
                "native item overflow was not Limit"
            );
        }
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let app = NSApplication::sharedApplication(main);
    let active = app.isActive();
    let visible_windows = app
        .windows()
        .iter()
        .filter(|window| window.isVisible())
        .count();
    let board = PrivateBoard::new();
    publish(&board.0, &[item(UTF8_TYPE, b"own seed")?])?;
    let token = DeliveryToken::new(NonZeroU64::new(1).ok_or("session missing")?);
    let name = board.0.name().to_string();
    // Native capture in this disposable contract process, then real preparation/install.
    let backup = Snapshot::capture(&name, main)?;
    assert_eq!(backup.data(0, UTF8_TYPE), Some(b"own seed".as_slice()));
    let lease = PreparedClipboard::new(&name, backup, UNICODE, &token, main)?.install(main)?;
    let view = NSTextView::initWithFrame(
        main.alloc(),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(400., 120.)),
    );
    view.setEditable(true);
    view.setRichText(false);
    view.setSelectedRange(NSRange::new(0, 0));
    assert!(
        view.readSelectionFromPasteboard_type(&board.0, &NSString::from_str(UTF8_TYPE)),
        "owned native paste input rejected"
    );
    assert!(
        view.string().length() <= 512,
        "owned readback exceeded bound"
    );
    let readback = view.string().to_string();
    assert!(
        readback.as_bytes() == UNICODE.as_bytes(),
        "owned native Unicode input differs"
    );
    assert_eq!(readback.match_indices(UNICODE).count(), 1);
    assert_eq!(readback.len(), 80);
    assert_eq!(
        (Sha256::digest(readback.as_bytes()))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "55dcecf192298167c397764f94f84551b7bd6980287a3cf310b8eb1346a0fcfb"
    );
    assert_eq!(lease.restore(main)?, RestoreOutcome::Restored);
    assert_eq!(
        board
            .0
            .stringForType(&NSString::from_str(UTF8_TYPE))
            .map(|value| value.to_string())
            .as_deref(),
        Some("own seed")
    );
    assert_eq!(app.isActive(), active);
    assert_eq!(
        app.windows()
            .iter()
            .filter(|window| window.isVisible())
            .count(),
        visible_windows
    );
    background_writer_contract(main)?;
    native_limits(main)?;
    assert_eq!(app.isActive(), active);
    assert_eq!(
        app.windows()
            .iter()
            .filter(|window| window.isVisible())
            .count(),
        visible_windows
    );
    println!(
        "native_clipboard_contract: {}",
        serde_json::json!({
            "kind":"private-native-paste-input-and-backup-limits", "unicodeUtf8Bytes":80,
            "unicodeSha256":"55dcecf192298167c397764f94f84551b7bd6980287a3cf310b8eb1346a0fcfb",
            "exactOccurrences":1,"restoreVerified":true,"maxTypes":MAX_TYPES,"maxTypeBytes":MAX_TYPE_BYTES,
            "maxDataBytes":MAX_DATA_BYTES,"maxItems":MAX_ITEMS,"atLimitCases":4,"oneOverLimitCases":4,
            "separateWriterProcess":true,"writerWaitBoundMillis":2000,
            "ownershipChanged":true,"foreignSentinelPreserved":true,
            "overflowError":"Limit","nativeBackupReadOnly":true,"generalClipboardUsed":false,
            "physicalCommandV":false,"globalHotkey":false,"visibleWindowsAdded":0,"activationChanged":false
        })
    );
    Ok(())
}
fn private_writer(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !valid_writer_board(name) {
        return Err("private writer namespace rejected".into());
    }
    let main = MainThreadMarker::new().ok_or("main thread required")?;
    let board = NSPasteboard::pasteboardWithName(&NSString::from_str(name));
    // Read-only precondition binds this helper to an already installed test lease.
    let before = Snapshot::capture(name, main)?;
    if before.data(0, UTF8_TYPE) != Some(UNICODE.as_bytes()) {
        return Err("private writer lease precondition rejected".into());
    }
    publish(&board, &[item(UTF8_TYPE, FOREIGN_SENTINEL)?])?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    objc2::rc::autoreleasepool(|_| match args.as_slice() {
        [] => run(),
        [mode, name] if mode == "--private-writer" => private_writer(name),
        _ => Err("private contract arguments rejected".into()),
    })
}
