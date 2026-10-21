//! `memory.h` peak / active / cache counter wrappers. Re-exported by
//! [`crate::memory`]; see the parent module's docs for the unified surface
//! description (counters + wired-limit guard + policies).

use crate::error::{Result, check, ensure_handler_installed};

/// Peak simultaneously active bytes in the process-global MLX allocator since
/// process start (or [`reset_peak_memory`]). Excludes its reusable-buffer cache.
///
/// Wraps `mlx_get_peak_memory`. mlx-lm's `mx.get_peak_memory()` (used in
/// `mlx_lm/generate.py` `stream_generate` to populate the
/// `GenerationResponse.peak_memory` field) is the same C++ entry point;
/// mlx-lm divides by `1e9` to report GB — the safe-Rust surface stays in
/// the raw byte count and lets the caller choose the scale.
pub fn peak_memory() -> Result<u64> {
  ensure_handler_installed();
  let mut bytes: usize = 0;
  // SAFETY: `&mut bytes` is a valid writable `size_t*` for the call; mlx-c
  // does not retain the pointer past the call. The backend rc is surfaced
  // via `check()`.
  check(unsafe { mlxrs_sys::mlx_get_peak_memory(&mut bytes) })?;
  Ok(bytes as u64)
}

/// Reset the peak-memory counter to zero. This does not free any allocation;
/// the next allocation updates the peak from the then-current active size.
///
/// Wraps `mlx_reset_peak_memory`. mlx-swift exposes the same hook as
/// `GPU.resetPeakMemory()`; mlx-lm uses it indirectly through
/// `mx.reset_peak_memory()` in its perf-eval scripts.
pub fn reset_peak_memory() -> Result<()> {
  ensure_handler_installed();
  // SAFETY: `mlx_reset_peak_memory` takes no arguments; the backend rc is
  // surfaced via `check()`.
  check(unsafe { mlxrs_sys::mlx_reset_peak_memory() })
}

/// The MLX allocator's active allocation size, excluding its recycled cache.
/// This is not the process RSS or physical footprint.
///
/// Wraps `mlx_get_active_memory`. Mirrors mlx-swift's `GPU.activeMemory`
/// and mlx-lm's `mx.get_active_memory()` (debug / diagnostics use only;
/// `GenerationStats` itself reports the peak).
pub fn active_memory() -> Result<u64> {
  ensure_handler_installed();
  let mut bytes: usize = 0;
  // SAFETY: same `size_t*` out-param contract as `peak_memory`.
  check(unsafe { mlxrs_sys::mlx_get_active_memory(&mut bytes) })?;
  Ok(bytes as u64)
}

/// The mlx allocator's currently-recycled cache size in bytes.
///
/// Wraps `mlx_get_cache_memory`. Mirrors mlx-swift's `GPU.cacheMemory`
/// (`active + cache` is the allocator's total reservation).
pub fn cache_memory() -> Result<u64> {
  ensure_handler_installed();
  let mut bytes: usize = 0;
  // SAFETY: same `size_t*` out-param contract as `peak_memory`.
  check(unsafe { mlxrs_sys::mlx_get_cache_memory(&mut bytes) })?;
  Ok(bytes as u64)
}
