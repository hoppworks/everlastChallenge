# Asynchronous refinement in the runtime

This package follows `b7e348c`. The runtime now accepts an optional text owner,
submits complete transcripts without waiting for GPU work and consumes validated
results through the existing controller. The desktop setup does not yet supply
that owner, and desktop sessions still disable refinement. This is an integration
contract, not product, model-quality or performance acceptance.

## Behavior and failure boundaries

The original remains visible while the owner works. Only a source-, language-
and session-bound accepted result can reach native delivery. An unavailable
service retains the original and fails closed; it does not silently release
unprocessed text. A failed text owner is retired while the ASR runtime remains
available. A later explicit session can still use dictation without refinement.

Caller cancellation revokes the inference and text tokens immediately. An
internal text-owner failure can also revoke its text token, so that token alone
does not establish user cancellation. The controller uses the caller's inference
revocation to preserve that distinction. A concurrent real user cancellation
still wins. Sleep unloads the text owner once per observed suspended transition;
late results cannot affect the next recording.

Shutdown closes admission and joins each owner even if another cleanup fails.
All observed cleanup errors remain in the runtime snapshot while the returned
error preserves the primary cause. Normal submission and result polling do not
join the worker; retirement and final shutdown do.

Independent review found an additional race: an idle child crash after successful
completion but before dequeue revoked the stored result and could be reported
as user cancellation. A real-process test reproduced this failure. Its pipe-write
marker did not guarantee the required host-side ordering, so the final regression
constructs a stored success and invokes the real fatal-exit guard before dequeue.
It failed with the old ordering and passed with the correction. Existing process
tests still cover idle crash and cleanup. Fatal owner closure now takes precedence
over completion dequeue; joining the owner retains the concrete exit cause.

## Verification and limits

- Runtime suite with `measurement,test-control`: **49 passed**, one existing
  native 300-second test ignored because it requires pinned models and audio.
- Controller suite with those features: **25 passed**.
- Supervisor/owner suite: **23 passed**: 22 real-process contracts and the new
  deterministic fatal-close regression.
- Seven new runtime adapter tests passed again after the correction. They cover
  validated delivery, failure isolation, sleep, cancellation versus failure,
  missing service, stale results and joining after an audio cleanup failure.
- Rust formatting, host/worker Clippy, fast repository checks, generated-contract
  checks and the source secret scan passed alongside these focused suites.

The runtime fixtures control audio, inference and native delivery. Sending a
native command in those fixtures does not prove receipt in another application.
The process suite uses an executable protocol fixture, not the MLX model.
Actual model coexistence, setup, UI, offline bundle execution and observed target
receipt remain unverified here. Earlier model-quality failures remain open.
No performance measurements were taken during these builds.

The first shutdown-fixture compile used incorrect helper names and was corrected;
it is not counted as behavioral evidence. Clippy subsequently caught a nested
condition and an ignored test-channel result; both were corrected. Existing
test and model evidence for unchanged paths was reused. The combined host/worker
Clippy run also caught an unread retained-original field in the separately
compiled process-test module; its result helper now verifies that original.

## Source binding

| File | SHA-256 |
|---|---|
| `src-tauri/src/runtime.rs` | `904a8303d9a7953041c9d89803ccea8f95179a276cf0807e75e8b08430bc24c1` |
| `src-tauri/src/runtime/refinement.rs` | `7c60e6ce13996dce93786057ab095ea6fb1e3bddb2278543c2ec0a5cedf7a3b0` |
| `src-tauri/src/runtime/measurement.rs` | `5dbb3eee429f659f74dd1f159bc9d2aad9c3b1a092632a4ec54aaba1aa0118f5` |
| `src-tauri/src/runtime/tests/refinement_contract.rs` | `709d81f8e1e0593d466bc13e1feb6a447cc1407200c2734eb67a5acda57606dd` |
| `src-tauri/src/controller.rs` | `e721104e7636f6eb3f2293cf0eb3c9d174ca8a5d9120c8f2d10df44f1899333f` |
| `src-tauri/src/controller/refinement.rs` | `10e4b9698587ee63adc3148b90e8283e0513cc191fcdbcc6291f4616ff8bd2e6` |
| `src-tauri/src/refinement_service/owner.rs` | `fd70266e8917a267efaefba83d70d4370b9a6f1fa16f59408b4e0da843a13acb` |
| `crates/text-refinement-worker/tests/supervisor.rs` | `6bc67f8065f554feb07e494774204d3f5ba1bc154c42aa33034f8e8cc7ce1671` |
| `crates/text-refinement-worker/tests/fixtures/process.rs` | `f0c1135b6812abb998d12da9281cb95d75c05861a277488a7bf2a6914b44deb4` |
