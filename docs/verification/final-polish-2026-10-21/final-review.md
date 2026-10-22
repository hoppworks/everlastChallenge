# Independent final source and evidence review — 21 October 2026

Reviewer: independent Codex subagent in the same local execution environment. No product code, tests, build configuration, or runtime state changed by this review. This report is the only written artifact.

## Scope and conclusion

Reviewed the coherent completion package over integration base `e7cabfb`, including native fix `4ac9c55`, experimental controller/decoder isolation `ac78ecc`, display-name change `d1602b5`, and the pending guardrail/coverage/documentation changes at HEAD `6599669342ad7892f6a0d3ebb1516bf52760a235`. The hash inventory below binds the material implementation files read at this review. Original external review target `2d5d0a1` is older Main, not this integrated source.

**No remaining blocking source finding in the reviewed package.** This is a source/evidence review, not a claim that the pending final integrated check, fresh release, Main merge, installation, or physical application acceptance has completed.

## Standards axis

- Rust ownership remains explicit: native clipboard state stays on the application main thread; waiting happens on the native worker. No new application unsafe block or detached cleanup worker is introduced.
- FullPrefix controller/decoder paths are excluded from ordinary builds. The retained audio helper and historical measurement programs mean this is not a claim that every experimental helper was removed.
- Typed ESLint rules reject floating/misused promises and explicit any. Existing strict TypeScript, Oxlint and generated-contract checks remain present. The source guard now inspects application scripts as well as Rust/TypeScript source, with rule fixtures excluded intentionally.
- The architecture graph rejects reverse frontend dependencies and cycles. Core dependency restrictions include build dependencies. The production feature traversal follows local aliases, cycles, incoming workspace dependency feature requests and cross-crate feature edges; the earlier direct-feature and alias bypass findings were corrected.
- Native source changes are checked against the pinned upstream inventory; the same input list is consumed by the guard and the build-script synchronization step. No further mismatch between current four changed native inputs and build ownership was found.
- CI separates portable checks from the explicitly configured native runner. Actions are pinned to real verified commits, credentials are not persisted, permissions are read-only, and secret scanning is included. Comprehensive local verification and optional native CI now include host coverage. Neither this YAML nor local evidence proves a remote CI run or GitHub branch protection.

## Spec and correctness axis

The reviewed package keeps Rust/Tauri 2 with an actual TypeScript/React surface and native microphone/hotkey/paste behavior. Display name becomes Everlast Coding Challenge; the bundle identifier remains `de.hoppworks.everlast-dictation`.

The native timeout defect is corrected at the relevant ownership seam. `SentPaste` timestamps the 500 ms interval after the actual event posts. Early cleanup returns a deferred deadline without taking the native entry. `NativeBackend::cleanup` waits off-main and retries, preserving shutdown ownership. Abandoned successful sends and deferred cleanups do not execute premature direct cleanup. Normal replies still get delivered: an initial implementation accidentally skipped them, was reported during review, and is corrected with an explicit successful/deferred-reply regression.

The retained tests exercise the actual completion/abandonment logic with controlled ordering across the timeout, plus the actual cleanup gate before and at its deadline. They do not claim an actual keyboard/clipboard OS run. A stalled GUI beyond the request timeout can still yield an explicit failure; the change does not promise successful external receipt or hard cancellation of native inference.

## Evidence read back

- `paste-green.log`: 12 native-service tests passed, including timeout-after-commit and normal/deferred acknowledgment.
- `read-window-tests.log`: both native main-delivery tests passed; the gate fixture verifies early cleanup does not consume its lease and cleanup is permitted at the exact deadline.
- `frontend-coverage.log`: 98.18% statements, 95.58% branches, 98.91% functions, 98.97% lines. This is frontend coverage only.
- `coverage/host/cargo-output.jsonl`: 345 tests passed, 3 ignored, no failure; runtime 35.97 seconds. `host-coverage-final.log` agrees with the selected summary below and all configured floors pass.

| Host file | Observed line coverage | Covered/total lines |
|---|---:|---:|
| `native_service.rs` | 70.1308% | 965/1376 |
| `controller.rs` | 95.2199% | 498/523 |
| `runtime.rs` | 89.2086% | 1984/2224 |
| `setup.rs` | 78.4491% | 870/1109 |
| `desktop.rs` | 59.9905% | 1264/2107 |

These initial per-file regression floors are 70/95/89/78/59 percent. Instrumentation includes inline test code and uses measurement/test-control features. It is not production-only branch coverage, coverage of all dependencies, or a safety proof. Removing forced link-dead-code fixed an unused Swift-FFI linker problem in instrumented Tauri build helpers; the preserved first failure is not represented as a passing run.

Portable/tooling receipts were read as scoped prior evidence. The integrated check was not complete at the time of this review. This review did not rerun builds, product tests, microphone tests, or hotkeys.

## Pure-MLX follow-up evidence

Independently verified the frozen probe SHA, both metadata corpus hashes, all six retained WAV hashes, all result texts, time/RSS receipts, and exact recomputation of both quality JSONs from the pinned evaluator. Four German outputs are identical; the fifth changes German edits from 11 to 10 over 119 reference words. English changes from 0 to 2 errors over 34 reference words, including decertification becoming the certification.

Total single-run process times are 4.053284459 s (hybrid) and 2.955995417 s (MLX); maximum RSS is 1,343,356,928 and 1,736,900,608 bytes. Profiles and checkpoints differ; order is fixed; OS caches may differ. The pure-ASR profile has no CPP ASR fallback but still shares native Silero VAD. The report correctly retains the hybrid as product and makes no universal speed, memory, semantic-quality, or new final-build latency guarantee.

## Scope limits

This source review does not establish the result of later integrated checks or package verification. GUI and physical-key observations apply only to the binaries identified in their respective reports.

## Reviewed file hashes

| File | SHA-256 |
|---|---|
| `crates/macos-integration/src/main_delivery.rs` | `4dc8b402d10f284a5546f5e0a38755b560cf90a4a7594b9fcf6d685d68a34b5d` |
| `crates/macos-integration/src/paste.rs` | `4ca640e59abd6c3c31a40d7ceb5e96fb2375cea96dd87608c5436f8a53d1c714` |
| `src-tauri/src/native_service.rs` | `f734b33eef253f833b627a58a3ff98e642a0e89e939fd0f1b1876d3367c29e1a` |
| `src-tauri/src/controller.rs` | `4008a6cd58511818bd9df4013d460a2bf70dbcc9969f91c00d5044f4526390d3` |
| `src-tauri/src/inference_service.rs` | `e892c43d2a179ec04bdaa00da8af649aedff571d72a8fe4463839162fa599901` |
| `src-tauri/src/pipeline.rs` | `a4b73193b0db5f68b7f135ed0b8c48e03c7cb1ed854df35aa3dda1ad2598fd64` |
| `src-tauri/src/runtime.rs` | `df306a4473e8500f04462a36f723492a2d83b5409ce751fa405a6e1f507fcfef` |
| `src-tauri/src/transcription.rs` | `923c44f155aa0adca7636a9f45970067c7656d71682d38fe3288f0ca0578696c` |
| `scripts/architecture.mjs` | `39cb9ebfbdd97ed6d6edf430265399f9b8bfe61497941210c51b90df5ae7d03c` |
| `scripts/architecture.test.mjs` | `9ffa15e7d1345b264c6b601e355554313bfbbb02e7e4826d324d79ca3586e43f` |
| `scripts/coverage.mjs` | `3a697c2161186bbd4d21cb30fbdb5f36c673af039d45f39a008cca5a04c2ffc0` |
| `scripts/coverage-host.mjs` | `82de53e76f16b670a37cc62372395cf0866f2a23dd3c49393b91489f619ffce8` |
| `scripts/guardrails.mjs` | `923f193a039771b91995972f00cffe3b36f71b4125f799f564897aa571042357` |
| `eslint.config.mjs` | `13fd1496c97107662d903f6a9c49063fc554a9773438790385274547b39d129d` |
| `config/coverage.json` | `e8cb58b2c175031ef8dd9110744c524614b6729e3e3296abc80d8f86287f125b` |
| `vendor/whisper-rs-sys/build.rs` | `67b02d2c3603a69aef992ebc8acffd4ebccc574ed4cfeda675a0e71a70c62761` |
| `vendor/whisper-rs-sys/native-inputs.txt` | `d38dd6f52f48346400b1a2e07ef0520f934f1c077e5d760fee06be152021b122` |
| `.github/workflows/quality.yml` | `3d47ec8266ab4a049deb25fec7fe4de87a1c9adc136610979caa5b0ec3129825` |
| `package.json` | `08205010e51320ad04752ded9574b0b5f17c81a92fdd608de32c2c68fff39d86` |
