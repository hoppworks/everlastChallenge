# Bounded autonomous job runner

`scripts/overnight-runner.mjs` runs only commands declared in a JSON plan. It
does not discover tests, launch an app, ask for a hotkey, record audio, download
models, retry failures, or change permissions. It requires Node and macOS/Linux
process groups. The runner's own tests use short, harmless Node child processes:

```sh
node --test scripts/overnight-runner.test.mjs
node scripts/overnight-runner.mjs --plan /absolute/plan.json --state /absolute/private-run-state
```

Prepare the plan before leaving. Commands must already be noninteractive; the
runner closes stdin and never opens a shell. This is process orchestration, not
a sandbox: declare trusted executables and scripts only. A child that opens a GUI
permission dialog cannot be made unattended by this runner. Its timeout will
end the owned process group and independent work can continue.

## Plan contract

```json
{
  "schemaVersion": 1,
  "deadline": "2026-10-21T07:00:00Z",
  "jobs": [
    {
      "id": "runner-contract",
      "argv": ["/absolute/path/to/node", "--test", "scripts/overnight-runner.test.mjs"],
      "cwd": "/absolute/path/to/checkout",
      "timeoutMs": 30000,
      "dependsOn": [],
      "inputs": ["scripts/overnight-runner.mjs", "scripts/overnight-runner.test.mjs"],
      "outputs": [],
      "resourceGroup": "software"
    }
  ]
}
```

This example is a template, not a runnable campaign or benchmark permission.
Replace paths, choose an unexpired deadline, and declare the exact bounded jobs.
The example timestamp is 21 October 2026, 09:00 Europe/Berlin; it must be
replaced with a future deadline for a new run.

Every job needs a unique safe ID, absolute executable and existing absolute cwd,
positive integer timeout, dependencies, at least one explicit regular input file,
and an explicit output list (which may be empty). Paths in input/output lists are
relative to cwd unless absolute. Directories and automatic source scans are not
supported. Include all relevant scripts, source files, configuration, toolchain
pins, fixtures, model files and linked libraries in `inputs`; include generated
artifacts required for subsequent work in `outputs`. An incomplete input list
cannot prove complete dependency coverage. A checked manifest file alone does
not bind its referenced files: list those files too.

`env` optionally adds string environment values to the inherited environment.
The entire effective environment is fingerprinted without writing its values to
the journal. Commands and logs may contain sensitive content; use a private
state directory and keep secrets out of argv. `resourceGroup` is an optional
descriptive label. **All jobs run serially**, including Cargo and GPU jobs, so
there is no overlapping resource use within one run. Stop other external builds
and measurements separately; this runner does not claim ownership of unrelated
processes. Distinct state directories are independent and must not be used to
start competing campaigns. One state directory has an exclusive owner lock.

Dependencies are ordered before use, regardless of their order in the plan.
Missing dependencies, cycles, duplicate/unsafe IDs, shells, invalid timeouts and
unknown job fields are rejected before any command starts. Missing files or
failed launches block/fail only the affected job and its descendants.

## Evidence and resume

The state directory contains immutable `plan-<sha256>.json` snapshots, a current
`summary.json`, an owner `runner.lock` while active, and a new unique attempt
directory for every actual execution. Every attempt stores `result.json` and
`output.log`. Before launching, a running receipt is persisted; after spawn it
also records the PID/process-group identity. Completed receipts retain argv,
cwd, timeout, timestamps, dependency names, declared file hashes, fingerprint,
PID, exit code/signal, cleanup status, log hash and output hashes on success.
Summaries/results are replaced atomically; prior attempt directories remain.

Resume uses the same CLI and state directory. Successful evidence is reused
only if the job declaration, all declared file bytes/resolved paths/modes,
executable, effective environment, runner, platform/architecture and dependency
fingerprints/output hashes match. Logs and declared outputs must still exist
with matching hashes. Output tampering or changed inputs creates a new attempt,
preserving old results. A successful command that changes a declared input or
fails to produce a declared output is recorded as failed.

Unchanged failures, timeouts and cleanly interrupted attempts retain their
original result; they are not automatically retried. Diagnose and change the
relevant input/command before the next attempt. Missing evidence for an unchanged
failure is an inspection blocker, not permission to retry. A prior running or
inspection-required attempt is never blindly restarted, even on subsequent
resumes. A live/stale lock is never stolen. After an abnormal runner termination,
inspect its recorded owner and child process identities and the domain-specific
session status; do not infer termination from a journal file or PID alone. Only
after confirming ownership, process cleanup and the recording/controller state
may an operator archive the old state and prepare an explicit new attempt. This
inspection is a read-only agent task during the night, never a request to the
user to press a key.

## Time limits and cleanup

The runner starts no job unless its complete timeout plus a one-second cleanup
reserve fits before the absolute deadline. This is checked again after hashing.
Hashes stream through a bounded buffer and check the deadline. A per-job timer
terminates only that job's new process group, first with SIGTERM and then
SIGKILL after 150 ms, with a bounded 500 ms confirmation window. Unexpected
descendants after a successful parent exit are cleaned and recorded as a failed
job. SIGINT/SIGTERM clean the current group and prevent later starts. Unconfirmed
cleanup prevents subsequent work and requires inspection. No tool can guarantee
an exact wall-clock finish while the machine is suspended or the OS is stalled.

Jobs must keep descendants in their owned process group. Do not daemonize or
use `open` to hand off execution to LaunchServices: such processes escape this
cleanup contract and require their own explicit owned lifecycle wrapper.

## Native test batches and real hotkeys

This runner does not extend or implement the native controller protocol. Use a
separate short, validated batch wrapper with fresh private control storage, its
own recipient and developer process, readiness/focus checks, an ACK-bound SID,
bounded fresh recipient observations and explicit cleanup. Existing test control
expires after 15 minutes; each accepted recording is at most 60 seconds; the
recipient reporter expires after 30 minutes. Finish safely before these limits
and create fresh processes for another batch. Never push a 300-second recording
through the 60-second channel. Unknown start outcomes require status inspection
before any further start.

The controlled controller path is software regression evidence. It does not
prove physical keyboard delivery, accessibility permissions on a new build, or
an app-to-app speedup. Missing native prerequisites are documented as blocked;
independent software work continues. A necessary final physical-hotkey check
remains an honest morning follow-up and never blocks the night on user input.

## One prepared controlled native smoke

`scripts/controlled-native-smoke.mjs` implements one bounded native batch for an
**already prepared** developer app/own recipient. It launches neither application,
changes no focus/rights/preferences, and accepts no arbitrary target/text:

```sh
node scripts/controlled-native-smoke.mjs \
  --control-dir /private/tmp/own-fresh-control \
  --recipient-report /absolute/own-recipient/snapshot.json \
  --fixture /absolute/corpus/audio/fleurs-de-dev-1517.wav \
  --output-dir /absolute/private-evidence/new-smoke \
  --endpoint-started-at 2026-10-20T22:00:00Z
```

Replace the timestamp with the actual freshly launched endpoint's start time;
the driver refuses an endpoint older than 12 minutes. The output parent must
already be canonical; the output directory must be new. Config, report and audio
must be private same-user regular files; the control directory is canonical
and private and its config contains only the own recipient PID. Server-side AX
checks remain authoritative. A fresh reporter with zero existing occurrences,
actual foreground/window/focused-field readiness and a live matching PID is
required before start.

Only the historical public FLEURS German sentence 1517 is accepted: SHA-256
`b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137`,
10.56 seconds. Its existing original path on this Mac is
`/Users/hoppworks/projects/everlastChallenge/artifacts/corpus/audio/fleurs-de-dev-1517.wav`.
No audio is collected by the runner implementation tests. Actual CLI execution
explicitly starts the prepared app's system-default microphone controller and
plays this verified public file through `/usr/bin/afplay`.

The driver uses exactly one `start-host`, requires acknowledgement and matching
controlled SID in actual Recording, plays the public file, issues an explicit
stop for that SID, waits for terminal Idle, and then waits for the independent
recipient's later exact-once observation. Actor `sent` alone cannot pass. The
40-second work budget plus at most five seconds for cleanup remains below the
native endpoint's 60-second recording bound. Transport calls, playback, phase
waits and recipient reads all have bounds. SIGINT/SIGTERM stop playback and
attempt cleanup of only an acknowledged matching SID. A lost start reply is
reported as unknown/blocked and is never retried, adopted or blindly cancelled.
The outer generic runner should allow the driver's complete 45-second budget
plus startup/cleanup reserve; prematurely SIGKILLing a prepared client does not
prove cleanup of a separately prepared app. Verify domain cleanup separately
after an abnormal client exit; native TTLs are unchanged.

`result.json` records controlled start/playback/terminal boundaries, status
events, exact owned SID, cleanup outcome and source/config/fixture bindings.
Exact recipient snapshot bytes are preserved as separate numbered files before
the periodic publisher can replace them. A passed real run is microphone and
controlled-delivery evidence, with `physicalHotkey=false`; injected-transport
tests prove the driver logic only. A failed prerequisite produces a blocked
result or CLI failure and never asks the user to click, speak, or press a key.
