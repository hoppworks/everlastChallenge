# Temporary installed-app hotkey diagnosis

Status: investigation, **not a fix or a passed acceptance test**.

The installed app initially passed the user's Option+D stop test, but Escape did
not cancel (the foreground application is unknown). During a later attempted
physical check the user also reported Option+D unavailable. A targeted process
sample showed a responsive main event loop and desktop owner; only one installed
challenge process was found. These observations do not identify a root cause.

Automated checks can start a recording and cancel it with Escape while the app
is foreground. With TextEdit foreground the automation did not deliver Escape;
automated Option+D also did not trigger. Consequently synthetic input is not
accepted as evidence about real global keyboard delivery.

## Opt-in tracing

Explicitly authorized by the user on 2026-10-21. Create the empty file
`~/Library/Application Support/de.hoppworks.everlast-dictation/hotkey-diagnostics.enabled`
before launching the diagnostic build. Each hotkey-owner installation creates a
new `hotkey-diagnostics-<pid>-<time>.log` in that directory, with mode 0600.
Remove the marker and restart to disable tracing. This is temporary diagnostic
code to remove after the failure has been understood.

Only elapsed microseconds, `toggle`/`cancel` and fixed technical stage names are
written: registration, native press/release, gate accepted/ignored, main-thread
queue/callback, revocation and errors. No audio, transcription, arbitrary pressed
keys, application title or target process identity is recorded.

The native callback performs no file IO and never waits for the diagnostic lock.
Its preallocated queue holds at most 128 records; total event output is capped at
2048 records. Dropped events or saturation produce one additional
`diagnostic / incomplete` line: such a trace must not be used to conclude that an
absent line proves an absent native event. The background owner drains the queue
and performs a final drain at shutdown. Open/write errors are surfaced through
the existing application error path.

## Verification scope

A standalone Rust test checks opt-in, bounded output, saturation reporting and
fresh log creation on reinstall. Source guardrails and a complete diagnostic
app build are required before installation. No ASR corpus, timing campaign or
model comparison needs repeating because of these diagnostic-only changes.
A new trace and a known physical keypress are still needed to identify the
reported global failure.
