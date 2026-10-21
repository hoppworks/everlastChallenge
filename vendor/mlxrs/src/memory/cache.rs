//! Exclusive ownership of the process-wide reusable-buffer cache budget.

use std::sync::{Mutex, MutexGuard, TryLockError};

use crate::error::{Error, InvariantViolationPayload, Result, check, ensure_handler_installed};

static CACHE_OWNER: Mutex<()> = Mutex::new(());

/// Set the reusable-buffer budget and return the previous value, in bytes.
///
/// This changes a process-wide policy, not the active-memory limit. A final
/// returned buffer can temporarily exceed the budget. Prefer [`CacheLimitGuard`]
/// when a model owns the policy across multiple calls. Fails while a guard owns
/// the policy, including on the same thread.
pub fn set_cache_limit(bytes: u64) -> Result<u64> {
  let _owner = cache_owner()?;
  set_cache_limit_inner(bytes)
}

fn cache_owner() -> Result<MutexGuard<'static, ()>> {
  match CACHE_OWNER.try_lock() {
    Ok(owner) => Ok(owner),
    Err(TryLockError::Poisoned(error)) => Ok(error.into_inner()),
    Err(TryLockError::WouldBlock) => {
      Err(Error::InvariantViolation(InvariantViolationPayload::new(
        "cache budget",
        "another model owns the cache policy",
      )))
    }
  }
}

// Caller holds CACHE_OWNER for the complete native mutation.
fn set_cache_limit_inner(bytes: u64) -> Result<u64> {
  let bytes = usize::try_from(bytes).map_err(|_| {
    Error::InvariantViolation(InvariantViolationPayload::new(
      "cache budget",
      "byte count does not fit the platform size type",
    ))
  })?;
  ensure_handler_installed();
  let mut previous = 0usize;
  // SAFETY: the writable out-pointer lives through the call and is not retained.
  check(unsafe { mlxrs_sys::mlx_set_cache_limit(&mut previous, bytes) })?;
  Ok(previous as u64)
}

/// One model's exclusive process-wide cache policy. Finish or unwind frees
/// reusable buffers and restores the previous budget. Active arrays stay live.
/// Raw setters must not bypass ownership.
#[must_use = "hold the cache policy for the model's complete lifetime"]
pub struct CacheLimitGuard {
  previous: Option<u64>,
  limit: u64,
  _owner: MutexGuard<'static, ()>,
}

impl CacheLimitGuard {
  /// Fail rather than overwrite a concurrently owned cache policy.
  pub fn install(bytes: u64) -> Result<Self> {
    let owner = cache_owner()?;
    let previous = set_cache_limit_inner(bytes)?;
    Ok(Self {
      previous: Some(previous),
      limit: bytes,
      _owner: owner,
    })
  }

  /// Requested reusable-buffer budget; not a measurement of memory consumption.
  pub fn limit(&self) -> u64 {
    self.limit
  }

  /// Budget captured before this owner installed its policy.
  pub fn old_limit(&self) -> u64 {
    self.previous.unwrap_or(self.limit)
  }

  /// After dropping the model, free reusable buffers and restore the previous
  /// policy. Both operations are attempted and backend failures are surfaced.
  pub fn finish(mut self) -> Result<()> {
    if let Some(previous) = self.previous {
      ensure_handler_installed();
      // SAFETY: this call releases only allocator-owned reusable buffers.
      let released = check(unsafe { mlxrs_sys::mlx_clear_cache() });
      let restored = set_cache_limit_inner(previous);
      if restored.is_ok() {
        self.previous = None;
      }
      return released.and(restored.map(|_| ()));
    }
    Ok(())
  }
}

impl Drop for CacheLimitGuard {
  fn drop(&mut self) {
    if let Some(previous) = self.previous.take() {
      let mut replaced = 0usize;
      // SAFETY: previous came from size_t; the out-pointer is local. Drop does
      // not create streams, drain TLS errors, or panic during worker teardown.
      unsafe {
        let _ = mlxrs_sys::mlx_clear_cache();
        let _ = mlxrs_sys::mlx_set_cache_limit(&mut replaced, previous as usize);
      }
    }
  }
}
