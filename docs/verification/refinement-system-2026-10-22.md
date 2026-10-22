# Packaged refinement: system observations

These checks use source revision `5be1a62` and its locally signed macOS bundle.
They extend the [model and supervisor checks](refinement-context-2026-10-22.md).
They do not establish comparative speed or complete product acceptance.
The [observation record](refinement-system-2026-10-22.json) binds the binaries,
source history and diagnostic records. Private paths and signing identities are
excluded. The diagnostic harnesses are not part of the distributed application.

## Bundle and offline operation

The complete application was built, locally signed and relocated with signature
metadata preserved. Bundle inventory, executable architectures, library linkage,
microphone declarations and strict nested signature verification passed after
relocation. Local signing does not constitute Apple notarization.

A second copy outside the repository matched every file in the signed manifest.
It started and completed native recording, ASR and refinement of the same
synthetic sentence, with the same original and result stated below. This checks
the relocated application's processing path; recording was started from its
window, so it is not evidence of automatic target delivery.

The actual bundle then ran with a sandbox profile denying network operations.
Connection probes in the sandbox and an inherited child both failed with
`Operation not permitted`. The text model had been downloaded and verified
before this run; this is an offline inference check, not offline installation.

A synthetic German sentence entered through BlackHole 2ch, exercising native
recording, the qualified hybrid ASR and optional text refinement. The visible
original was “Wir treffen uns am Montag, nein, am Dienstag um 9 Uhr.”; the result
was “Wir treffen uns am Dienstag um 9 Uhr.” The original remained available.
This observation used the application window to start and stop recording.

The sandbox launch did not retain Accessibility trust. Consequently this run
does **not** prove global-shortcut delivery or receipt in another application.
The ordinary signed launch retained its existing permissions, but the separate
recipient test has not yet met its foreground-focus prerequisite. No successful
paste is inferred from an event request or an empty recipient field.

The real model-download UI also recovered from cancellation and subsequently
completed a new download and verification. These are observed UI paths, not a
network-fault or disk-failure campaign.

## Process loss and recovery

Two separate native-supervisor experiments used the signed packaged text worker.
Before each injection, a macOS call-graph sample showed model inference and
`QuantizedMatmul::eval_gpu`, rather than merely listing a loaded Metal library.

- Killing the worker returned a transport failure without an accepted result.
  The supervisor removed its scratch directory; a new worker completed a request.
- Killing the owning host closed the worker's input pipe. The worker disappeared
  without an additional kill from the harness. Its scratch directory remained,
  as expected after abrupt host death. Starting the next supervisor recovered
  that abandoned directory, completed a request and cleaned its own directory.

The observed intervals are recorded as diagnostic samples. They are not
worst-case termination bounds or an end-to-end performance benchmark. This
worker-only supervisor harness does not prove behavior after every possible
desktop or operating-system crash.

## Model coexistence and memory-pressure limits

The real FP16 hybrid ASR and text model were resident together on a 32-GiB Mac.
Serial ASR runs before and after text inference produced identical text. A
process-footprint snapshot after both models had run showed approximately
2.1 GiB for the ASR host and 2.8 GiB for the text worker. These are sampled
process footprints, not measured peaks, a system-memory sum or a lower-memory
device qualification. Both owners closed successfully.

An attempted OS warning simulation using `memory_pressure -S -l warn -s 1`
failed because the kernel trigger required additional permission. It emitted
no observed pressure event. This is a recorded unsuccessful check; the existing
[injected-pressure tests](refinement-pressure-2026-10-22.md) do not substitute
for actual OS event delivery. No allocation-stress test was performed.

Human linguistic acceptance, actual target receipt, OS-pressure delivery and
the current complete-app performance comparison remain unverified by this record.

## Software checks and advisory disposition

The full `npm run verify` invocation passed its check, tests, core/frontend and
host coverage, negative guardrail fixtures and source-secret scan. It then exited
with code 1: `cargo deny` rejected the unmaintained `paste 1.0.15` macro used by
the stable grammar adapter. The original failed run is retained.

The [dependency note](../dependencies.md) records why the compatible adapter
still needs this compile-time dependency and the narrowly scoped maintenance
exception. A package ban rejects new direct users other than `tokenizers`; a
negative fixture confirmed that rejection. `cargo deny check` with version
0.20.2, the affected `check:fast` and the previously unreached npm audit then
passed. Runtime dependencies and product source did not change. This is a
documented continuation of the failed check, not a second successful invocation
of the entire suite or proof of a remote CI run. Setup and the portable workflow
now install the same pinned advisory checker.
