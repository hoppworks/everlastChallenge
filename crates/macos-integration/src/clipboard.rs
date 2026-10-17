//! Bounded, in-memory pasteboard backup. Never persists clipboard content.
use crate::delivery::DeliveryToken;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardAccessBehavior, NSPasteboardItem, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSString};
use std::{
    io::{Read, Write},
    marker::PhantomData,
    path::Path,
    process::{Child, Command, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

pub const MAX_ITEMS: usize = 16;
pub const MAX_TYPES: usize = 128;
pub const MAX_TYPE_BYTES: usize = 1024;
pub const MAX_DATA_BYTES: usize = 8 * 1024 * 1024;
const MATERIALIZE_LIMIT: Duration = Duration::from_millis(250);
const HELPER_LIMIT: Duration = Duration::from_secs(2);
const HELPER_FLAG: &str = "--everlast-clipboard-snapshot";

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("Clipboard contents exceed the safe backup limits")]
    Limit,
    #[error("Clipboard backup protocol is invalid")]
    Protocol,
    #[error("Clipboard backup could not allocate bounded storage")]
    Memory,
    #[error("Clipboard changed during backup")]
    Changed,
    #[error("Clipboard contents could not be materialized")]
    Unavailable,
    #[error("Clipboard backup exceeded its deadline")]
    Timeout,
    #[error("Clipboard backup was cancelled")]
    Cancelled,
    #[error("Clipboard backup helper failed")]
    Helper,
    #[error("Clipboard write failed")]
    Write,
    #[error("Clipboard restore failed")]
    Restore,
    #[error("Clipboard I/O failed")]
    Io(#[from] std::io::Error),
}

#[derive(PartialEq, Eq)]
struct Representation {
    name: String,
    data: Vec<u8>,
}

#[derive(PartialEq, Eq)]
pub struct Snapshot {
    pasteboard_name: String,
    change_count: i64,
    items: Vec<Vec<Representation>>,
}

type Writers = NSArray<ProtocolObject<dyn NSPasteboardWriting>>;

/// Construct all replacement/restore objects before the last native guards.
/// This value and its resulting lease stay on the main thread.
pub struct PreparedClipboard {
    token: DeliveryToken,
    pasteboard: Retained<NSPasteboard>,
    expected_count: i64,
    original: Retained<Writers>,
    replacement: Retained<Writers>,
    _main_thread: PhantomData<Rc<()>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    Restored,
    OwnershipChanged,
}

pub struct ClipboardLease {
    pasteboard: Retained<NSPasteboard>,
    own_count: i64,
    original: Option<Retained<Writers>>,
    _main_thread: PhantomData<Rc<()>>,
}

impl PreparedClipboard {
    pub fn new(
        name: &str,
        backup: Snapshot,
        text: &str,
        token: &DeliveryToken,
        _main_thread: MainThreadMarker,
    ) -> Result<Self, ClipboardError> {
        backup.validate()?;
        if backup.pasteboard_name != name {
            return Err(ClipboardError::Protocol);
        }
        if name.is_empty()
            || name.len() > 512
            || name.contains('\0')
            || text.is_empty()
            || text.len() > 256 * 1024
        {
            return Err(ClipboardError::Limit);
        }
        let original = make_items(&backup.items)?;
        let replacement = make_items(&[vec![
            Representation {
                name: "public.utf8-plain-text".into(),
                data: text.as_bytes().to_vec(),
            },
            Representation {
                name: "org.nspasteboard.TransientType".into(),
                data: vec![],
            },
            Representation {
                name: "org.nspasteboard.ConcealedType".into(),
                data: vec![],
            },
        ]])?;
        Ok(Self {
            token: token.clone(),
            pasteboard: NSPasteboard::pasteboardWithName(&NSString::from_str(name)),
            expected_count: backup.change_count,
            original,
            replacement,
            _main_thread: PhantomData,
        })
    }

    /// Caller first validates target/layout/modifiers. Token is rechecked here
    /// at mutation; the returned lease must survive until delayed cleanup.
    pub fn install(self, main_thread: MainThreadMarker) -> Result<ClipboardLease, ClipboardError> {
        let token = self.token.clone();
        if !token.is_pending() {
            return Err(ClipboardError::Cancelled);
        }
        if self.pasteboard.changeCount() as i64 != self.expected_count {
            return Err(ClipboardError::Changed);
        }
        let own_count = self.pasteboard.clearContents() as i64;
        let lease = ClipboardLease {
            pasteboard: self.pasteboard,
            own_count,
            original: Some(self.original),
            _main_thread: PhantomData,
        };
        if !lease.is_current() {
            return Err(ClipboardError::Changed);
        }
        if !token.is_pending() {
            lease.restore(main_thread)?;
            return Err(ClipboardError::Cancelled);
        }
        if !lease.pasteboard.writeObjects(&self.replacement) {
            lease.restore(main_thread)?;
            return Err(ClipboardError::Write);
        }
        if !lease.is_current() {
            return Err(ClipboardError::Changed);
        }
        if !token.is_pending() {
            lease.restore(main_thread)?;
            return Err(ClipboardError::Cancelled);
        }
        Ok(lease)
    }

    pub(crate) fn belongs_to(&self, token: &DeliveryToken) -> bool {
        self.token.same_work(token)
    }
}

impl ClipboardLease {
    pub fn is_current(&self) -> bool {
        self.pasteboard.changeCount() as i64 == self.own_count
    }

    pub fn restore(
        mut self,
        _main_thread: MainThreadMarker,
    ) -> Result<RestoreOutcome, ClipboardError> {
        self.restore_inner()
    }

    fn restore_inner(&mut self) -> Result<RestoreOutcome, ClipboardError> {
        let Some(original) = self.original.take() else {
            return Err(ClipboardError::Restore);
        };
        if !self.is_current() {
            return Ok(RestoreOutcome::OwnershipChanged);
        }
        let restored_count = self.pasteboard.clearContents() as i64;
        if self.pasteboard.changeCount() as i64 != restored_count {
            return Ok(RestoreOutcome::OwnershipChanged);
        }
        if original.count() != 0 && !self.pasteboard.writeObjects(&original) {
            return Err(ClipboardError::Restore);
        }
        if self.pasteboard.changeCount() as i64 != restored_count {
            return Ok(RestoreOutcome::OwnershipChanged);
        }
        Ok(RestoreOutcome::Restored)
    }
}

impl Drop for ClipboardLease {
    fn drop(&mut self) {
        if self.original.is_some() && self.restore_inner().is_err() {
            eprintln!("clipboard-restore-failed");
        }
    }
}

fn make_items(items: &[Vec<Representation>]) -> Result<Retained<Writers>, ClipboardError> {
    let mut native_items = Vec::with_capacity(items.len());
    for representations in items {
        let item = NSPasteboardItem::new();
        for representation in representations {
            if !item.setData_forType(
                &NSData::with_bytes(&representation.data),
                &NSString::from_str(&representation.name),
            ) {
                return Err(ClipboardError::Write);
            }
        }
        native_items.push(item);
    }
    let objects: Vec<_> = native_items
        .iter()
        .map(|item| ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&**item))
        .collect();
    Ok(NSArray::from_slice(&objects))
}

impl Snapshot {
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn data(&self, item: usize, name: &str) -> Option<&[u8]> {
        self.items
            .get(item)?
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.data.as_slice())
    }

    /// Called only in the isolated helper, never in the app's GUI process.
    pub fn capture(name: &str, _main_thread: MainThreadMarker) -> Result<Self, ClipboardError> {
        if name.is_empty() || name.len() > 512 || name.contains('\0') {
            return Err(ClipboardError::Protocol);
        }
        let started = Instant::now();
        let pasteboard = NSPasteboard::pasteboardWithName(&NSString::from_str(name));
        if pasteboard.accessBehavior() == NSPasteboardAccessBehavior::AlwaysDeny {
            return Err(ClipboardError::Unavailable);
        }
        let change_count = pasteboard.changeCount() as i64;
        let native_items = pasteboard
            .pasteboardItems()
            .ok_or(ClipboardError::Unavailable)?;
        let mut items = Vec::new();
        let mut types_left = MAX_TYPES;
        let mut bytes_left = MAX_DATA_BYTES;
        {
            if native_items.count() > MAX_ITEMS {
                return Err(ClipboardError::Limit);
            }
            for native_item in &native_items {
                let types = native_item.types();
                if types.count() == 0 || types.count() > types_left {
                    return Err(ClipboardError::Limit);
                }
                types_left -= types.count();
                let mut item = Vec::with_capacity(types.count());
                for native_type in &types {
                    if started.elapsed() >= MATERIALIZE_LIMIT {
                        return Err(ClipboardError::Timeout);
                    }
                    if native_type.length() > MAX_TYPE_BYTES {
                        return Err(ClipboardError::Limit);
                    }
                    let name = native_type.to_string();
                    if name.len() > MAX_TYPE_BYTES {
                        return Err(ClipboardError::Limit);
                    }
                    if file_promise_type(&name) {
                        return Err(ClipboardError::Unavailable);
                    }
                    let data = native_item
                        .dataForType(&native_type)
                        .ok_or(ClipboardError::Unavailable)?;
                    if data.length() > bytes_left {
                        return Err(ClipboardError::Limit);
                    }
                    bytes_left -= data.length();
                    item.push(Representation {
                        name,
                        data: data.to_vec(),
                    });
                }
                items.push(item);
            }
        }
        if started.elapsed() >= MATERIALIZE_LIMIT {
            return Err(ClipboardError::Timeout);
        }
        if pasteboard.changeCount() as i64 != change_count {
            return Err(ClipboardError::Changed);
        }
        let snapshot = Self {
            pasteboard_name: name.to_owned(),
            change_count,
            items,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn decode(mut input: impl Read) -> Result<Self, ClipboardError> {
        let mut magic = [0_u8; 5];
        input.read_exact(&mut magic)?;
        if &magic != b"EVPB2" {
            return Err(ClipboardError::Protocol);
        }
        let name_length = read_u32(&mut input)? as usize;
        if name_length == 0 || name_length > 512 {
            return Err(ClipboardError::Protocol);
        }
        let pasteboard_name = String::from_utf8(read_bytes(&mut input, name_length)?)
            .map_err(|_| ClipboardError::Protocol)?;
        if pasteboard_name.contains('\0') {
            return Err(ClipboardError::Protocol);
        }
        let mut count_bytes = [0; 8];
        input.read_exact(&mut count_bytes)?;
        let change_count = i64::from_le_bytes(count_bytes);
        let item_count = read_u32(&mut input)? as usize;
        if item_count > MAX_ITEMS {
            return Err(ClipboardError::Limit);
        }
        let mut items = Vec::with_capacity(item_count);
        let mut types_left = MAX_TYPES;
        let mut bytes_left = MAX_DATA_BYTES;
        for _ in 0..item_count {
            let count = read_u32(&mut input)? as usize;
            if count == 0 || count > types_left {
                return Err(ClipboardError::Limit);
            }
            types_left -= count;
            let mut item: Vec<Representation> = Vec::with_capacity(count);
            for _ in 0..count {
                let name_length = read_u32(&mut input)? as usize;
                if name_length == 0 || name_length > MAX_TYPE_BYTES {
                    return Err(ClipboardError::Limit);
                }
                let name = String::from_utf8(read_bytes(&mut input, name_length)?)
                    .map_err(|_| ClipboardError::Protocol)?;
                if name.contains('\0') || item.iter().any(|entry| entry.name == name) {
                    return Err(ClipboardError::Protocol);
                }
                if file_promise_type(&name) {
                    return Err(ClipboardError::Unavailable);
                }
                let data_length = read_u32(&mut input)? as usize;
                if data_length > bytes_left {
                    return Err(ClipboardError::Limit);
                }
                bytes_left -= data_length;
                item.push(Representation {
                    name,
                    data: read_bytes(&mut input, data_length)?,
                });
            }
            items.push(item);
        }
        let mut trailing = [0_u8; 1];
        if input.read(&mut trailing)? != 0 {
            return Err(ClipboardError::Protocol);
        }
        Ok(Self {
            pasteboard_name,
            change_count,
            items,
        })
    }

    pub fn encode(&self, mut output: impl Write) -> Result<(), ClipboardError> {
        self.validate()?;
        output.write_all(b"EVPB2")?;
        output.write_all(&(self.pasteboard_name.len() as u32).to_le_bytes())?;
        output.write_all(self.pasteboard_name.as_bytes())?;
        output.write_all(&self.change_count.to_le_bytes())?;
        output.write_all(&(self.items.len() as u32).to_le_bytes())?;
        for item in &self.items {
            output.write_all(&(item.len() as u32).to_le_bytes())?;
            for representation in item {
                output.write_all(&(representation.name.len() as u32).to_le_bytes())?;
                output.write_all(representation.name.as_bytes())?;
                output.write_all(&(representation.data.len() as u32).to_le_bytes())?;
                output.write_all(&representation.data)?;
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ClipboardError> {
        if self.pasteboard_name.is_empty()
            || self.pasteboard_name.len() > 512
            || self.pasteboard_name.contains('\0')
        {
            return Err(ClipboardError::Protocol);
        }
        if self.items.len() > MAX_ITEMS {
            return Err(ClipboardError::Limit);
        }
        let mut types = 0;
        let mut bytes = 0;
        for item in &self.items {
            if item.is_empty() {
                return Err(ClipboardError::Protocol);
            }
            types += item.len();
            if types > MAX_TYPES {
                return Err(ClipboardError::Limit);
            }
            for (index, entry) in item.iter().enumerate() {
                if file_promise_type(&entry.name) {
                    return Err(ClipboardError::Unavailable);
                }
                if entry.name.is_empty()
                    || entry.name.len() > MAX_TYPE_BYTES
                    || entry.name.contains('\0')
                    || item[..index].iter().any(|prior| prior.name == entry.name)
                {
                    return Err(ClipboardError::Protocol);
                }
                bytes = usize::checked_add(bytes, entry.data.len()).ok_or(ClipboardError::Limit)?;
                if bytes > MAX_DATA_BYTES {
                    return Err(ClipboardError::Limit);
                }
            }
        }
        Ok(())
    }
}

#[allow(
    deprecated,
    reason = "Legacy file-promise markers must also be rejected to avoid replaying an unfulfillable promise."
)]
fn file_promise_type(name: &str) -> bool {
    // SAFETY: immutable exported NSString, valid for the process lifetime.
    let legacy = unsafe { objc2_app_kit::NSFilesPromisePboardType };
    name.starts_with("com.apple.pasteboard.promised-")
        || name == legacy.to_string()
        || name == "com.apple.NSFilePromiseItemMetaData"
}

/// Early executable entry point, before initializing Tauri or any app service.
/// The parent passes only a pasteboard name; content leaves the helper via pipe.
pub fn snapshot_helper_main() -> Option<Result<(), ClipboardError>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(HELPER_FLAG)) {
        return None;
    }
    Some((|| {
        let name = args
            .next()
            .and_then(|name| name.into_string().ok())
            .ok_or(ClipboardError::Protocol)?;
        if args.next().is_some() {
            return Err(ClipboardError::Protocol);
        }
        let main_thread = MainThreadMarker::new().ok_or(ClipboardError::Helper)?;
        let snapshot = Snapshot::capture(&name, main_thread)?;
        snapshot.encode(std::io::stdout().lock())
    })())
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                if self.0.kill().is_err() {
                    eprintln!("clipboard-helper-stop-failed");
                }
                if self.0.wait().is_err() {
                    eprintln!("clipboard-helper-reap-failed");
                }
            }
        }
    }
}

/// Blocking native-worker operation with one bounded helper and pipe reader.
/// `executable` must be this app's executable supporting snapshot_helper_main.
pub fn capture_bounded(
    executable: &Path,
    name: &str,
    token: &DeliveryToken,
) -> Result<Snapshot, ClipboardError> {
    if !token.is_pending() {
        return Err(ClipboardError::Cancelled);
    }
    let started = Instant::now();
    let mut child = ChildGuard(
        Command::new(executable)
            .arg(HELPER_FLAG)
            .arg(name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let stdout = child.0.stdout.take().ok_or(ClipboardError::Helper)?;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name("clipboard-backup-pipe".into())
        .spawn(move || {
            // Bounded decode rejects lengths before allocating or reading payloads.
            if send.send(Snapshot::decode(stdout)).is_err() { /* cancelled consumer */ }
        })?;
    let result = (|| {
        let mut decoded = None;
        loop {
            if !token.is_pending() {
                return Err(ClipboardError::Cancelled);
            }
            if started.elapsed() >= HELPER_LIMIT {
                return Err(ClipboardError::Timeout);
            }
            if decoded.is_none() {
                match receive.try_recv() {
                    Ok(value) => decoded = Some(value?),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Err(ClipboardError::Helper);
                    }
                }
            }
            if let Some(status) = child.0.try_wait()? {
                if !status.success() {
                    return Err(ClipboardError::Helper);
                }
                if let Some(snapshot) = decoded {
                    return Ok(snapshot);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    // Terminate/reap before joining so a blocked pipe read sees EOF.
    drop(child);
    reader.join().map_err(|_| ClipboardError::Helper)?;
    result
}

fn read_u32(input: &mut impl Read) -> Result<u32, ClipboardError> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_bytes(input: &mut impl Read, length: usize) -> Result<Vec<u8>, ClipboardError> {
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|_| ClipboardError::Memory)?;
    data.resize(length, 0);
    input.read_exact(&mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_wire_lengths_are_rejected_before_reading_or_allocating_payloads() {
        let mut wire = b"EVPB2".to_vec();
        wire.extend(1_u32.to_le_bytes());
        wire.push(b'x');
        wire.extend(0_i64.to_le_bytes());
        wire.extend(((MAX_ITEMS + 1) as u32).to_le_bytes());
        assert!(matches!(
            Snapshot::decode(wire.as_slice()),
            Err(ClipboardError::Limit)
        ));
        wire.truncate(18);
        wire.extend(1_u32.to_le_bytes());
        wire.extend(1_u32.to_le_bytes());
        wire.extend(1_u32.to_le_bytes());
        wire.push(b'x');
        wire.extend(((MAX_DATA_BYTES + 1) as u32).to_le_bytes());
        assert!(matches!(
            Snapshot::decode(wire.as_slice()),
            Err(ClipboardError::Limit)
        ));
    }

    #[test]
    fn trailing_data_and_truncated_backups_are_never_accepted()
    -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = Snapshot {
            pasteboard_name: "test".into(),
            change_count: 5,
            items: vec![],
        };
        let mut wire = Vec::new();
        snapshot.encode(&mut wire)?;
        assert!(Snapshot::decode(&wire[..wire.len() - 1]).is_err());
        wire.push(0);
        assert!(matches!(
            Snapshot::decode(wire.as_slice()),
            Err(ClipboardError::Protocol)
        ));
        Ok(())
    }

    #[test]
    fn binary_backup_preserves_all_items_types_and_bytes() -> Result<(), Box<dyn std::error::Error>>
    {
        let snapshot = Snapshot {
            pasteboard_name: "test".into(),
            change_count: 17,
            items: vec![
                vec![
                    Representation {
                        name: "public.utf8-plain-text".into(),
                        data: b"text".to_vec(),
                    },
                    Representation {
                        name: "public.html".into(),
                        data: b"<b>text</b>".to_vec(),
                    },
                ],
                vec![Representation {
                    name: "public.png".into(),
                    data: vec![0, 255, 128, 1],
                }],
            ],
        };
        let mut wire = Vec::new();
        snapshot.encode(&mut wire)?;
        assert!(Snapshot::decode(wire.as_slice())? == snapshot);
        Ok(())
    }
}
