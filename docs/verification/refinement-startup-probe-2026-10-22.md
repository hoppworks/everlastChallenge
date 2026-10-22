# Explicit local text-model startup test

Explicit text-model setup now verifies the files, starts the bundled MLX worker,
processes a fixed synthetic sentence and shuts the worker down before permitting
new activation. The expected result is checked exactly. No user dictation,
clipboard content or target application is involved in this test.

Passive startup inspection still checks files without loading the model. A
separate, nonpersistent `probePassed` flag distinguishes verified files from a
successful startup test. Newly enabling refinement requires that flag in both
the settings UI and native settings validation. An already enabled preference
keeps lazy loading after restart. Cancellation, retry and failure clear the flag.

The setup owner shares the supervisor's private-directory preparation and crash
recovery. It propagates cancellation to downloads and the child process, preserves
the original failure if cleanup also fails, and reports incomplete shutdown.
Memory pressure cancels pending activation and active setup before forwarding
the pressure event to the normal runtime. Setup is refused while pressure is
already elevated.

## Verification

Failing regressions established that cached files previously skipped the probe
and could enable refinement without it. Focused lifecycle, native settings and
UI/IPC tests now exercise loading, cancellation, retry, probe failure and
file-only readiness. The stricter generated contract rejects a
successful probe flag outside the Ready state.

Additional tests distinguish startup timeout, invalid output, clean cancellation
and cancellation with a cleanup failure. The latter remains Failed, never a
successful cancellation or an activation grant. Native adapter tests reject a
runtime path that is a file and an absent worker executable, and verify private
directory permissions. The first coverage run fell below the existing setup
floor; these targeted failure checks were added without reducing that floor.
The cache-repair test additionally verifies that passive startup does not repair
damaged assets or grant activation, while explicit setup repairs and probes them.
An initial missing-executable test expectation was corrected from Spawn to
InvalidPath; review then isolated that case by providing a valid model directory.
These were test corrections, not production fixes.

The final host run passed 429 tests with three ignored; portable supervisor
tests passed 28 with one ignored. All existing per-file coverage floors passed,
as did formatting, Clippy, generated contracts and fast repository checks.
Host line measurements include inline test code; they are regression checks,
not production-only coverage or a correctness guarantee. The record retains
the three earlier below-floor measurements rather than presenting only the
successful run.

A native harness used the production worker and pinned model. Passive inspection
started no child; explicit setup processed the synthetic sentence; cancellation
after observing the real child reached Cancelled. These three runs preceded the
separate flag addition. A subsequent run on the final source exercised explicit
setup while another worker was already warm, reached Ready with `probePassed`
true, and verified that both processes exited and temporary job directories were
removed. Its source hashes are recorded in the accompanying
[verification record](refinement-startup-probe-2026-10-22.json).
The subsequent failure-test additions change test code only; removing those
additions reproduces the native run's exact setup-source hash.

The overlap run observed a maximum combined sampled RSS of 5,311,184 KiB for the
two text workers on a 32 GiB Mac. Sampling can miss peaks; this is not total
application or GPU memory accounting. It neither establishes an acceptable
low-memory budget nor tests simultaneous residency with the ASR models.

The worker and edit validator are unchanged from the
[66-case text verification](refinement-long-corrections-2026-10-22.md); that corpus
was not rerun for this setup-only change. The checks here do not establish
human linguistic acceptance, full packaged UI interaction, actual OS pressure
delivery, target text receipt or comparative performance.
