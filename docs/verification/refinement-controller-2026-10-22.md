# Retained originals and controller refinement contracts

This change follows `46c15c4`. It adds refinement to the session controller's
finalization contract. It is not yet a working product feature: the runtime
still reports the refinement service as unavailable, and current desktop
settings start sessions with refinement disabled. Owner dispatch, setup, IPC
projection and UI remain separate integration work.

## Implemented behavior

The pure session machine can retain one complete transcript after audio release
without issuing a delivery effect. Final acceptance remains a separate fact.
Cancellation and failure preserve the retained text; an accepted empty result
removes that intermediate text. Foreign IDs, premature retention and repeated
retention cannot replace it.

The controller captures the refinement preference with the existing immutable
session settings. Complete ordinary, full-prefix and segmented ASR results use
the same finalization method. Partial results do not start refinement. An
enabled session retains its original before applying the existing versioned
routing policy. Skipped, checked-unchanged, edited, rejected, failed and cancelled
states remain distinguishable in the internal snapshot. Exceeding the refinement
input limit retains the complete original and sends no truncated job.

The validated result now carries the exact original and validation language.
Both must match the controller's active session. Only an accepted result reaches
the existing final transcript/delivery path; no second delivery state machine was
added. Duplicate ASR completions and late/foreign refinement results cannot
replace the active original or cause another delivery.

Client cancellation, suspension and shutdown share a cancellation bundle for
inference, refinement and native delivery. The text-worker token is revoked
before a queued controller action is handled. A paused-actor test observes this
directly. Clearing while refining cancels work before removing local text.
Clearing after refinement but before the native commit also revokes delivery.
After the native commit, clearing removes local history without claiming to
retract the already sent event.

## Findings and corrections

Focused failing tests exposed:

- The old ASR completion path delivered immediately instead of requesting
  refinement.
- A valid result from another original could be substituted into the session.
- Validation under another language could remove meaningful German `um` using
  the English filler rule.
- Cancellation lacked a refinement status, and clear did not cancel pending
  refinement or a later uncommitted delivery.
- An empty accepted result left the retained intermediate text visible.

The initial attempted retention negative run overlapped another build and did
not establish a failing baseline. A separate no-op negative control subsequently
failed on the missing retained text, then passed after restoring retention.
The immediate-cancellation test first needed its pause fixture placed in the
correct test; the subsequent negative control failed while the actor was paused
and passed when refinement revocation was restored. These setup failures are not
counted as successful behavioral tests.

Independent review inspected the original binding, finalization routes, clear
behavior and cancellation bundle. Targeted follow-ups checked the language
binding and both sides of the native commit boundary without restarting review
or repeating builds. No concrete blockers remained in this contract package.

## Verification and limits

The feature-enabled controller suite passed 25 tests. The runtime suite passed
42 tests with one existing native 300-second replay test ignored because it
requires pinned models and authorized audio fixtures. That ignored test was not
silently counted as passing. Both prefix routes have explicit enabled-refinement
fixtures. The no-default-features protocol/worker run passed 48 tests, including
22 real child-process supervisor/owner tests; its GPU-only structured tests did
not run in that configuration.

The final check record also covers 73 passing pure core tests, Rust formatting, host/core
Clippy with measurement/test-control features, fast repository checks and the
unchanged generated IPC contracts. Native microphone, GPU coexistence, offline
product setup, packaged execution and observed target receipt are not established
by these tests. Earlier model-quality failures remain open; this change does not
qualify model quality or performance.

## Source binding

| File | SHA-256 |
|---|---|
| `crates/dictation-core/src/session.rs` | `f856d65e54a4fadddc91af478715306337ec42a950e559cd87fe622649cacced` |
| `crates/dictation-core/src/refinement/mod.rs` | `88c2efaac01e4b7cfec979a4082320b53bbb0be39e17643bd34e2e224a4d9b1e` |
| `crates/dictation-core/src/refinement/validator.rs` | `848c7cabc67261f20613d52219df234c0b3c562de83de60b0d77244892076db8` |
| `crates/dictation-core/tests/session_invariants.rs` | `3bd4a3cbbec72b215c7317a09ebb39a0020f49cf43ac6e0e38092e2df405aa05` |
| `src-tauri/src/controller.rs` | `e2c36dab83cc94ae04737043c16f845c9eda4466e501192a3d3f5552822a3642` |
| `src-tauri/src/controller/refinement.rs` | `c4bf01fbdeb1573ab384a1a56fd5dd510177c9292de306bc0eae45295317399c` |
| `src-tauri/src/runtime.rs` | `22e01bced968fab383ff9d9319e2b8230c215824c39486d867d9c2fbc0bcfc68` |
| `src-tauri/src/refinement_service/owner.rs` | `ae6c23cdff1f052a98cce064e400205099d63916b0112f43cffc3d6e7ffc4e54` |
