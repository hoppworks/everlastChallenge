# Local whisper-rs 0.16.0 wrapper patch

Prepared separately from the Engine/VAD integration, then selected through the
workspace Cargo patch table. The root lockfile changes only this package from
registry to local path; versions and dependency edges remain unchanged.

## Provenance and completeness

- Original cached registry package: `whisper-rs 0.16.0`, **Unlicense**. Original
  `LICENSE`, normalized `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock`, build script,
  examples, hidden metadata and all source files are preserved.
- Registry archive / existing workspace lock checksum:
  **`2088172d00f936c348d6a72f488dc2660ab3f507263a195df308a3c2383229f6`**.
  The cached `whisper-rs-0.16.0.crate` was hashed locally and matches that pin.
- `.cargo_vcs_info.json`: upstream commit
  **`7558e1b72f54f2f22a53589afb77e65681834c36`**, repository
  `https://codeberg.org/tazz4843/whisper-rs`, package path at repository root.
- All **33 published regular files, 173137 bytes** were read from the pinned
  cached archive and compared byte-for-byte by SHA256 with the original file
  manifest. The cached source additionally contained Cargo's seven-byte
  `.cargo-ok` extraction marker. It is intentionally omitted from this vendor
  copy because it is not a published source file.
- `ORIGINAL_FILES.json` records all **33 published original paths**, their byte
  counts and hashes. At that VAD-only stage the **only changed original file** was
  `src/whisper_vad.rs`. Added files are this note, the original-file inventory,
  and the standalone regression launcher under `regression/`.
- Original module SHA256:
  `d8033831a1b5aff45e8cdb27970bae5d492d1c92d9bf5b5001b91f32fe57be4a`.
- Patched module SHA256:
  **`a4e481fac0b423ab532e3f4b8e3348a23e3134a10e4581ea3f383d6a81e6438d`**.

The original `.gitignore` ignores `Cargo.lock`. Integration must explicitly
include this preserved original lockfile, for example with `git add -f
vendor/whisper-rs/Cargo.lock`; do not silently omit original package contents.

## Minimal behavioral change

The VAD constructor retains ownership of its `CString` throughout the
synchronous FFI call, passing its borrowed pointer with `as_ptr()`. It releases
the string when the constructor returns, including initialization failure.
The former `into_raw()` surrendered ownership without any matching reclamation.
The unused `c_char` import is removed. No model choice, ABI, inference, error
variant, existing NUL-byte panic policy, or context-drop behavior changes.

```diff
-use std::ffi::{c_char, CString};
+use std::ffi::CString;
@@
-        let model_path = CString::new(model_path)
-            .expect("VAD model path contains null byte")
-            .into_raw() as *const c_char;
-        let ptr =
-            unsafe { whisper_vad_init_from_file_with_params(model_path, params.into_inner()) };
+        let model_path = CString::new(model_path).expect("VAD model path contains null byte");
+        let ptr = unsafe {
+            whisper_vad_init_from_file_with_params(model_path.as_ptr(), params.into_inner())
+        };
```

The native ownership assumption was checked in the existing pinned
`whisper-rs-sys 0.15.0` source: `whisper_vad_init_from_file_with_params` opens
the file and reads its model synchronously. Before returning a successful
context, it assigns `path_model` to `ctx->path_model`, a `std::string`, making
an owned copy. It does not retain the caller's C-string pointer. The native
error path also returns synchronously. This code is unchanged by the local
SOT and VAD-error-status patches.
[Pinned native loader](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L4719-L4767),
[native context owns a string](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L4400-L4418).

## Actual-constructor RED / GREEN evidence

The standalone native fixture is
`crates/macos-integration/tests/fixtures/whisper_vad_lifetime.rs`.
It imports the **entire actual module** selected through `WHISPER_VAD_SOURCE`
with `include!`; no copied constructor and no string-pattern assertion.
A controlled synchronous native-call stand-in reads the supplied C string
during the call and returns either a nonnull opaque handle or null. An observing
`System` allocator tracks deallocation of exactly the pointer passed to that
call. It checks release **before the Rust constructor returns**, for both paths;
the successful context itself is dropped afterwards. The stand-in retains no
pointer. Its fake handle is never dereferenced or freed as real native memory.

All new unsafe allocator/FFI test code is confined to the native crate's new
fixture. The vendor runtime adds no unsafe operation beyond the existing FFI
call. The launcher uses only local Python stdlib and `rustc`, creates temporary
test binaries and cleans them afterwards; no Cargo build, external service,
model load, ASR, audio I/O or release build is involved.

From repository root, offline and without a registry-cache dependency:

```sh
python3 vendor/whisper-rs/regression/check_lifetime.py
```

By default the launcher reverses only the guarded minimal patch and requires
the reconstructed entire module to match the pinned original SHA256 exactly.
An optional positional path accepts the read-only cached original instead.
Both modes were actually executed on 2026-10-17. Compiler and fixture processes
have bounded timeouts. The repository quality suite runs the offline mode.


| Constructor module | Success-path CString releases | Failure-path CString releases | Valid paths observed inside FFI | Native context drops | Result |
|---|---:|---:|---:|---:|---|
| Original pinned registry | 0 / 100 | 0 / 100 | 200 / 200 | 100 / 100 | **RED**, exit 1 |
| Patched vendor | 100 / 100 | 100 / 100 | 200 / 200 | 100 / 100 | **GREEN**, exit 0 |

The launcher rejects an original-module hash mismatch, a crash or unexpected
return code, unexpected constructor results, or incorrect FFI-call/drop counts.
Original RED is specifically the missing CString reclamation in both cases,
not a compilation or linking failure. The original registry files were never
modified.

Fixture SHA256:
`1acefca99a2a275e058a42953be4132bf02829b62b0cb7d8945ae1f2268980e8`.
Launcher SHA256:
`f53c35de556575b4aafe0dbf75ffe9637238fca194da1661075cb936a162dca2`.

**Limits:** This precisely verifies Rust-side constructor CString lifetime at
the actual wrapper boundary, with a controlled FFI implementation. It does not
measure native model memory, exercise the real loader, prove whole-process RSS
stability, or qualify P05 lifecycle behavior. Integrated Cargo linkage, checks
and real native smoke evidence are recorded in
[wrapper-lifetime-fix.md](../../docs/verification/wrapper-lifetime-fix.md).

## Isolated optional encoder bridge (2026-10-19)

`WhisperState::full_with_encoder` is an additive safe wrapper around the separately
recorded native extension. It borrows PCM/features only for the synchronous call,
checks length/finiteness before FFI, and returns actual external/native window
counts. The native side checks the remaining bounded policy. The caller must bind
features to this exact prepared PCM and encoder model; this is a semantic caller
contract and no cryptographic proof is invented. Existing `full` remains unchanged.
`src/lib.rs` additionally reexports `EncoderUsage`; `src/whisper_state/mod.rs`
contains the new wrapper/type. These are the two additional original-file changes
beyond the preserved VAD ownership patch. Exact prior/current hashes are in
[the native bridge receipt](../whisper-rs-sys/EVERLAST-ENCODER-PATCH.json).
The VAD lifetime fixture and all historical limits above remain applicable.
