//! Standalone constructor lifetime regression, compiled directly with rustc.
//! Includes the complete real wrapper module; the synchronous FFI boundary is
//! controlled. This does not initialize a model or test native inference.
#![allow(
    non_camel_case_types,
    dead_code,
    reason = "stand-in bindings retain upstream names and the entire wrapper module is imported"
)]

extern crate self as whisper_rs_sys;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    ffi::{c_char, CStr},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
};

mod actual {
    include!(env!("WHISPER_VAD_SOURCE"));
}

#[derive(Debug)]
pub enum WhisperError {
    NullPointer,
    GenericError(i32),
}

const ITERATIONS: usize = 100;
const MODEL_PATH: &str = "/constructor-lifetime-fixture/not-a-real-model.bin";
static CAPTURED_PATH: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static PATH_FREES: AtomicUsize = AtomicUsize::new(0);
static VALID_PATHS: AtomicUsize = AtomicUsize::new(0);
static NATIVE_DROPS: AtomicUsize = AtomicUsize::new(0);
static SUCCEED: AtomicBool = AtomicBool::new(false);

struct TrackingAllocator;
// SAFETY: every allocation and deallocation is delegated with the original
// pointer/layout to System. Observation uses only lock-free atomics, no memory
// dereference or allocation. The harness runs constructor scenarios serially.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the GlobalAlloc layout contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if pointer == CAPTURED_PATH.load(Ordering::SeqCst) {
            PATH_FREES.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: pointer/layout are forwarded unchanged to their allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[derive(Clone, Copy, Debug)]
pub struct whisper_vad_context_params {
    pub n_threads: i32,
    pub use_gpu: bool,
    pub gpu_device: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct whisper_vad_params {
    pub threshold: f32,
    pub min_speech_duration_ms: i32,
    pub min_silence_duration_ms: i32,
    pub max_speech_duration_s: f32,
    pub speech_pad_ms: i32,
    pub samples_overlap: f32,
}
pub struct whisper_vad_context;
pub struct whisper_vad_segments;

/// Synchronous stand-in: consumes the filename before returning, retains none.
/// # Safety
/// The actual constructor must pass a live NUL-terminated C string.
pub unsafe fn whisper_vad_init_from_file_with_params(
    path: *const c_char,
    _: whisper_vad_context_params,
) -> *mut whisper_vad_context {
    CAPTURED_PATH.store(path.cast_mut().cast(), Ordering::SeqCst);
    // SAFETY: this is the actual wrapper's CString argument during its FFI call.
    if unsafe { CStr::from_ptr(path) }.to_bytes() == MODEL_PATH.as_bytes() {
        VALID_PATHS.fetch_add(1, Ordering::SeqCst);
    }
    if SUCCEED.load(Ordering::SeqCst) {
        ptr::NonNull::<whisper_vad_context>::dangling().as_ptr()
    } else {
        ptr::null_mut()
    }
}

/// # Safety
/// Only the fake nonnull handle returned by this fixture is accepted; never dereferenced.
pub unsafe fn whisper_vad_free(context: *mut whisper_vad_context) {
    if !context.is_null() {
        NATIVE_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

// Unused bindings are type-compatible stand-ins, needed to compile the complete
// upstream module. They perform no memory access, I/O, inference or allocation.
pub unsafe fn whisper_vad_detect_speech(
    _: *mut whisper_vad_context,
    _: *const f32,
    _: i32,
) -> bool {
    false
}
pub unsafe fn whisper_vad_n_probs(_: *mut whisper_vad_context) -> i32 {
    0
}
pub unsafe fn whisper_vad_probs(_: *mut whisper_vad_context) -> *mut f32 {
    ptr::null_mut()
}
pub unsafe fn whisper_vad_free_segments(_: *mut whisper_vad_segments) {}
pub unsafe fn whisper_vad_segments_from_probs(
    _: *mut whisper_vad_context,
    _: whisper_vad_params,
) -> *mut whisper_vad_segments {
    ptr::null_mut()
}
pub unsafe fn whisper_vad_segments_from_samples(
    _: *mut whisper_vad_context,
    _: whisper_vad_params,
    _: *const f32,
    _: i32,
) -> *mut whisper_vad_segments {
    ptr::null_mut()
}
pub unsafe fn whisper_vad_segments_n_segments(_: *mut whisper_vad_segments) -> i32 {
    0
}
pub unsafe fn whisper_vad_segments_get_segment_t0(_: *mut whisper_vad_segments, _: i32) -> f32 {
    0.0
}
pub unsafe fn whisper_vad_segments_get_segment_t1(_: *mut whisper_vad_segments, _: i32) -> f32 {
    0.0
}

fn scenario(succeed: bool) -> (usize, usize) {
    let mut freed_before_return = 0;
    let mut unexpected_results = 0;
    SUCCEED.store(succeed, Ordering::SeqCst);
    for _ in 0..ITERATIONS {
        CAPTURED_PATH.store(ptr::null_mut(), Ordering::SeqCst);
        PATH_FREES.store(0, Ordering::SeqCst);
        let result =
            actual::WhisperVadContext::new(MODEL_PATH, actual::WhisperVadContextParams::new());
        // CString must already be released when constructor returns, on either
        // success or failure, while the native context may remain alive.
        if PATH_FREES.load(Ordering::SeqCst) == 1 {
            freed_before_return += 1;
        }
        if !matches!(
            (&result, succeed),
            (Ok(_), true) | (Err(WhisperError::NullPointer), false)
        ) {
            unexpected_results += 1;
        }
        CAPTURED_PATH.store(ptr::null_mut(), Ordering::SeqCst);
        drop(result);
    }
    (freed_before_return, unexpected_results)
}

fn main() {
    let (success_freed, success_errors) = scenario(true);
    let (failure_freed, failure_errors) = scenario(false);
    let valid_paths = VALID_PATHS.load(Ordering::SeqCst);
    let native_drops = NATIVE_DROPS.load(Ordering::SeqCst);
    let unexpected_results = success_errors + failure_errors;
    let passed = success_freed == ITERATIONS
        && failure_freed == ITERATIONS
        && valid_paths == 2 * ITERATIONS
        && native_drops == ITERATIONS
        && unexpected_results == 0;
    println!("{{\"iterations\":{ITERATIONS},\"successFreed\":{success_freed},\"failureFreed\":{failure_freed},\"validPaths\":{valid_paths},\"nativeDrops\":{native_drops},\"unexpectedResults\":{unexpected_results},\"passed\":{passed}}}");
    if !passed {
        std::process::exit(1);
    }
}
