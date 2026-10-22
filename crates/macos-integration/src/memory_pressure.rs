//! Public libdispatch memory-pressure observation; no allocation-based probing.
use block2::RcBlock;
use dispatch2::{
    _dispatch_source_type_memorypressure, DispatchObject, DispatchQueue, DispatchRetained,
    DispatchSource,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
}
fn decode(flags: usize) -> Option<MemoryPressure> {
    if flags & 4 != 0 {
        Some(MemoryPressure::Critical)
    } else if flags & 2 != 0 {
        Some(MemoryPressure::Warning)
    } else if flags & 1 != 0 {
        Some(MemoryPressure::Normal)
    } else {
        None
    }
}
type Callback = Box<dyn Fn(MemoryPressure) + Send + Sync>;
struct Gate(Mutex<Option<Callback>>);
impl Gate {
    fn emit(&self, flags: usize) {
        let mut callback = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let (Some(level), Some(receive)) = (decode(flags), callback.as_ref())
            && catch_unwind(AssertUnwindSafe(|| receive(level))).is_err()
        {
            *callback = None;
            eprintln!("Memory pressure callback failed; observer disabled");
        }
    }
    fn disable(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
/// Callback delivery is serialized on a private queue. Callbacks must only
/// signal bounded work and must not call `remove` on their own observer.
/// Removal waits only for an already-running callback, never for model cleanup.
pub struct MemoryPressureObserver {
    source: DispatchRetained<DispatchSource>,
    gate: Arc<Gate>,
}
impl MemoryPressureObserver {
    pub fn new(receive: impl Fn(MemoryPressure) + Send + Sync + 'static) -> Self {
        let queue = DispatchQueue::new("de.hoppworks.everlast.memory-pressure", None);
        // SAFETY: Apple's public memory-pressure source uses handle 0 and the
        // documented NORMAL/WARN/CRITICAL mask (1|2|4). The source retains queue.
        let source = unsafe {
            DispatchSource::new(
                (&raw const _dispatch_source_type_memorypressure).cast_mut(),
                0,
                7,
                Some(&queue),
            )
        };
        let address = (&*source as *const DispatchSource) as usize;
        let gate = Arc::new(Gate(Mutex::new(Some(Box::new(receive)))));
        let delivered = Arc::clone(&gate);
        let block = RcBlock::new(move || {
            // SAFETY: this callback belongs to exactly this native source, which
            // libdispatch keeps alive during its event handler. The raw back-
            // reference avoids a retain cycle; it is used nowhere else. Reading
            // source data is only valid inside its event handler, as done here.
            let flags = unsafe { (&*(address as *const DispatchSource)).data() };
            delivered.emit(flags);
        });
        // SAFETY: libdispatch copies the block before returning. Its captures
        // are Send+Sync Rust state and the source-only back-reference above.
        unsafe {
            source.set_event_handler_with_block(RcBlock::as_ptr(&block));
        }
        source.activate();
        Self { source, gate }
    }
    pub fn remove(&mut self) {
        self.gate.disable();
        self.source.cancel();
    }
}
impl Drop for MemoryPressureObserver {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesced_flags_prioritize_pressure_and_removed_callbacks_are_inert()
    -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = std::sync::mpsc::sync_channel(3);
        let gate = Gate(Mutex::new(Some(Box::new(move |level| {
            assert!(tx.try_send(level).is_ok());
        }))));
        gate.emit(1);
        gate.emit(3);
        gate.emit(7);
        gate.emit(0);
        assert_eq!(rx.try_recv()?, MemoryPressure::Normal);
        assert_eq!(rx.try_recv()?, MemoryPressure::Warning);
        assert_eq!(rx.try_recv()?, MemoryPressure::Critical);
        gate.disable();
        gate.disable();
        gate.emit(4);
        assert!(rx.try_recv().is_err());
        Ok(())
    }
    #[test]
    fn native_source_can_be_created_activated_and_removed_twice() {
        let mut observer = MemoryPressureObserver::new(|_| {});
        observer.remove();
        observer.remove();
    }
    #[test]
    fn callback_panic_is_contained_and_disables_further_delivery() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let gate = Gate(Mutex::new(Some(Box::new(move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
            std::panic::resume_unwind(Box::new("controlled callback failure"));
        }))));
        gate.emit(2);
        gate.emit(4);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
