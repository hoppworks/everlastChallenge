//! Temporary, explicit opt-in diagnostics; never records audio, text or other keys.
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy)]
pub(crate) enum Key {
    Toggle,
    Cancel,
}
#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Registered,
    RegisterFailed,
    Unregistered,
    UnregisterFailed,
    NativePressed,
    NativeReleased,
    Accepted,
    Ignored,
    Queued,
    Busy,
    Revoked,
    Callback,
    DispatchFailed,
    Armed,
    Held,
}
#[derive(Clone, Default)]
pub(crate) struct Diagnostics(Option<Arc<Shared>>);
struct Shared {
    buffer: Mutex<Buffer>,
    lost: AtomicBool,
}
struct Buffer {
    file: File,
    start: Instant,
    remaining: usize,
    incomplete_reported: bool,
    events: VecDeque<(u128, Key, Stage)>,
}
const LIMIT: usize = 2048;
const QUEUE: usize = 128;

impl Diagnostics {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        if !directory.join("hotkey-diagnostics.enabled").is_file() {
            return Ok(Self::default());
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let name = format!("hotkey-diagnostics-{}-{nonce}.log", std::process::id());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(directory.join(name))?;
        Ok(Self(Some(Arc::new(Shared {
            lost: AtomicBool::new(false),
            buffer: Mutex::new(Buffer {
                file,
                start: Instant::now(),
                remaining: LIMIT,
                incomplete_reported: false,
                events: VecDeque::with_capacity(QUEUE),
            }),
        }))))
    }
    /// Callback path never waits for a lock, performs IO or grows its bounded queue.
    pub(crate) fn record(&self, key: Key, stage: Stage) {
        let Some(shared) = &self.0 else {
            return;
        };
        if let Ok(mut buffer) = shared.buffer.try_lock()
            && buffer.remaining > 0
            && buffer.events.len() < QUEUE
        {
            let elapsed = buffer.start.elapsed().as_micros();
            buffer.events.push_back((elapsed, key, stage));
            buffer.remaining -= 1;
        } else {
            shared.lost.store(true, Ordering::Relaxed);
        }
    }

    /// Called only by the background hotkey owner, never by native callbacks.
    pub(crate) fn flush(&self) -> io::Result<()> {
        if let Some(shared) = &self.0
            && let Ok(mut buffer) = shared.buffer.try_lock()
        {
            if shared.lost.load(Ordering::Relaxed) && !buffer.incomplete_reported {
                writeln!(buffer.file, "0\tdiagnostic\tincomplete")?;
                buffer.incomplete_reported = true;
            }
            while let Some((elapsed, key, stage)) = buffer.events.pop_front() {
                let key = match key {
                    Key::Toggle => "toggle",
                    Key::Cancel => "cancel",
                };
                let stage = match stage {
                    Stage::Registered => "registered",
                    Stage::RegisterFailed => "register_failed",
                    Stage::Unregistered => "unregistered",
                    Stage::UnregisterFailed => "unregister_failed",
                    Stage::NativePressed => "native_pressed",
                    Stage::NativeReleased => "native_released",
                    Stage::Accepted => "accepted",
                    Stage::Ignored => "ignored",
                    Stage::Queued => "queued",
                    Stage::Busy => "busy",
                    Stage::Revoked => "revoked",
                    Stage::Callback => "callback",
                    Stage::DispatchFailed => "dispatch_failed",
                    Stage::Armed => "armed",
                    Stage::Held => "held",
                };
                writeln!(buffer.file, "{elapsed}\t{key}\t{stage}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_in_and_bounded_event_only_output() -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            std::env::temp_dir().join(format!("everlast-trace-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        assert!(Diagnostics::open(&directory)?.0.is_none());
        std::fs::write(directory.join("hotkey-diagnostics.enabled"), b"")?;
        let trace = Diagnostics::open(&directory)?;
        let log = std::fs::read_dir(&directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "log"))
            .ok_or("log missing")?;
        // A contended callback must return immediately and mark the trace incomplete.
        let shared = trace.0.as_ref().ok_or("trace disabled")?;
        let guard = shared.buffer.lock().map_err(|_| "trace poisoned")?;
        trace.record(Key::Cancel, Stage::NativePressed);
        drop(guard);
        trace.record(Key::Cancel, Stage::Registered);
        trace.flush()?;
        assert!(std::fs::read_to_string(&log)?.contains("\tcancel\tregistered\n"));
        for _ in 0..(LIMIT * 2) {
            trace.record(Key::Toggle, Stage::NativePressed);
            trace.flush()?;
        }
        let output = std::fs::read_to_string(&log)?;
        assert_eq!(output.lines().count(), LIMIT + 1);
        assert!(output.contains("\tdiagnostic\tincomplete"));
        assert!(output.len() < 128 * 1024);
        assert!(
            Diagnostics::open(&directory)?.0.is_some(),
            "reinstall gets a fresh log"
        );
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
