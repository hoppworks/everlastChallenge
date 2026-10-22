# Recovery of abandoned text-worker snapshots

This change follows `d592afc`. It extends temporary model ownership across
abrupt process exits. The supervisor still waits for a child before ordinary
cleanup; the worker still terminates when its parent pipe closes.

## Ownership and recovery

The host creates a private, versioned owner directory with a fixed lease marker.
Host and worker hold separate shared locks on that file. The worker acquires its
lease before model loading and retains it until process exit or normal model
teardown. Application startup and subsequent worker creation scan for abandoned
owner directories. Removal requires the matching marker and an exclusive lease;
a live host or child therefore prevents removal.

A persistent runtime-directory lock serializes recovery and creation. It is not
removed on release, avoiding an unlink/recreate lock race. Recovery examines at
most 1,024 entries and reports I/O errors. Unknown markers, unmarked directories
from older versions and directory symlinks are preserved. Ownership does not
depend on timestamps or PID reuse. Like the existing model cache, this mechanism
assumes other processes running as the same user do not maliciously replace paths.

A recovery failure leaves text refinement unavailable with a visible error while
ordinary ASR remains usable. A later explicit startup can try again. Recovery and
filesystem work run on service threads, never in the microphone callback.

## Verification

The initial regression demonstrated that a released, marked snapshot survived
worker startup. After the correction, the same test removes it while preserving
an active shared lease and an unmarked directory.

- 45 worker tests passed: 8 lifecycle, 7 manifest/lease, 4 selection and 26
  supervisor/owner tests. One separately listed ignored test is a subprocess
  fixture invoked by the passing abrupt-host-exit test, not an omitted acceptance
  scenario. MLX-only structured tests were not part of this fixture suite.
- The abrupt-host test kills a real supervisor subprocess, observes its child
  finish after pipe closure, and recovers the abandoned directory afterward.
- Seven application tests passed, including startup recovery, invalid runtime
  paths and a symlinked lock. The optional failures preserve ASR availability.
- Host and worker Clippy checks, Rust formatting and fast repository checks passed.
- The updated native MLX worker was built and exercised outside the checkout with
  the new lease marker, pinned model and existing kernel library. It loaded,
  returned no edits for a clean German sentence and shut down with no model
  snapshot left inside its owner directory.

Independent review found no concrete correctness or specification blocker.
The [source manifest](refinement-recovery-sources-2026-10-22.json) identifies the
reviewed code and native smoke scope.

The real-process crash test uses a controlled generator; it does not measure
termination during a particular Metal kernel. The native smoke covers normal
model load and shutdown, not abrupt GPU termination. This change also does not
qualify memory-pressure behavior, linguistic quality, offline app execution,
actual target receipt or comparative latency. The previously built application
bundle predates this change and must be rebuilt for final application acceptance.
