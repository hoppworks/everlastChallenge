# Asynchronous local text-worker owner

This package adds a runtime-facing service around the existing process
supervisor after `db1857c`. It does not yet connect text refinement to the
controller, settings, IPC or UI. The model-quality failures in earlier reports
remain open.

## Service boundary

`Owner::spawn` creates a dedicated host thread; it does not load the model.
`submit` admits one strictly increasing, nonzero session. A pending or active
job and an unconsumed completion occupy the same bounded slot. A second job
cannot overwrite either. Submitted text is the bounded `OriginalTranscript`
type; only the existing validator can construct a successful
`AcceptedTranscript`. The service does not log transcript contents.

The service lazily starts the fixed `WorkerSpec` supplied by trusted host code.
A healthy child is reused; each replacement consumes a new generation without
wrapping. Each invocation carries the exact session, generation and increasing
request identity. The existing supervisor continues to enforce identity,
bounded pipes, startup/request deadlines, output validation and process cleanup.

Cancellation is independent of the owner's mutex and can reach a blocked model
startup or generation. `unload` revokes current work and unconsumed output, then
requests cleanup without waiting on the caller. `take_result` rechecks revocation
before releasing a stored success. Cancellation after dequeue still requires
the controller's final authority check; dequeue is not a delivery commit.

Idle close uses the explicitly supplied duration; the intended product setting
is 120 seconds. A clean child idle exit is reaped even before that host deadline.
The next explicit job may create a new generation. A nonzero exit during idle
closes the service, preserves its shutdown error and does not start a retry loop.
An ordinary failed job cleans up its child and reports the failure; a later
explicit session may try again. `shutdown` and `Drop` revoke work, wake the host
thread and join it after child cleanup. Cleanup failures during cancellation are
reported rather than converted to successful shutdown.

## Executed evidence

Local environment: Apple Silicon (`arm64`), macOS 27.0.1.

`cargo test -p text-refinement-worker --no-default-features --features
test-fixture --locked --offline --test supervisor` passed **22 tests**: the
existing 12 supervisor cases and ten added owner cases. They use real child
processes and temporary directories. Fixture traces contain only PID,
generation and session identifiers. Short configured idle deadlines exercise
the same timer path without waiting 120 seconds per test.

The added cases cover deferred load, process reuse, occupied-slot backpressure
both during work and after a valid completion has been stored,
idle unload/new generation, unload during startup and generation, revocation
after completion but before dequeue, cleanup failure during shutdown, clean
idle child exit, idle crash without retry, explicit retry/replay rejection and
both explicit shutdown and `Drop` during active startup/generation.

Failing executable tests preceded the corresponding implementations:

- The initial owner returned `Closed` instead of servicing the job.
- A no-op unload left the active cancellation token valid.
- An already stored success was released after revocation.
- Shutdown incorrectly returned success after snapshot cleanup failed.
- A clean child idle exit was not reaped until the host's long idle deadline.

The first test build also required an explicit submodule path because the
supervisor source is included from the isolated fixture crate. The corrected
focused runs and final 22-test run passed. The original failures remain part of
the development evidence; they are not successful runs.

Rust formatting, `npm run check:fast`, worker Clippy with no default features
and the explicit fixture feature (all targets), and host Clippy (library/tests)
passed with warnings denied. Independent combined review read the source and
fixtures and reported no concrete blocker; it did not repeat those commands.

## Limits

These are service/process tests, not a GPU, microphone or native receipt
campaign. No performance comparison ran alongside the builds. They do not
qualify model semantics, memory-pressure observation, abrupt-host snapshot
recovery, offline setup, the final packaged worker or the integrated
controller's original-retention and delivery decisions. The existing ASR
release is not a package acceptance result for this feature.

## Source binding

| File | SHA-256 |
|---|---|
| `src-tauri/src/refinement_service.rs` | `db2886fc5cb125669aad13a22ba30d4673e67befd52c98bcc395c91e87c7cb74` |
| `src-tauri/src/refinement_service/owner.rs` | `b2280edd54f3ff4d3858875fef32e444145f178855bd1d05adee2d78ca70a77f` |
| `crates/text-refinement-worker/tests/supervisor.rs` | `f0a38bf48cefd476ca8a056886703cc972ca486ad2e86e139b625dce68c89e32` |
| `crates/text-refinement-worker/tests/fixtures/process.rs` | `f0c1135b6812abb998d12da9281cb95d75c05861a277488a7bf2a6914b44deb4` |
