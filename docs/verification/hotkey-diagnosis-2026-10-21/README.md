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

## Installed candidate and initial observation

Diagnostic source commit: `e4ea631beb737adc82148ce051e5c7df99d40181`.
Installed executable SHA-256:
`f8e27c8f7b825c4aed2e90d5e94cbbd817cc58e9bd48df6c9291ec7693d2c1d7`.
The prior installed bundle is retained at
`artifacts/hotkey-diagnosis-2026-10-21/before-diagnostics.app`.
The signed bundle passed strict signature verification. The diagnostic unit test,
`check:fast`, and release-target library Clippy with `-D warnings` passed.
Independent review found no remaining blocking issue after its corrections.

The initial installed trace records successful toggle registration, followed by
Escape registration and arming during a UI-started recording, then removal after
an in-app Escape cancellation. The synthetic Option+D and Escape actions did
not generate `native_pressed` records, and the trace has no `incomplete` marker.
This confirms that this automation route does not exercise the global shortcut
callback. It does **not** prove how a physical keypress behaves. The test recording
was cancelled; the installed app was left idle with diagnostics enabled.

Detailed local receipts and the initial trace are retained under
`artifacts/hotkey-diagnosis-2026-10-21/`. The earlier delivery ZIP remains the
previous release candidate and has not been relabeled as a hotkey-fixed release.

## Physical result and removal

The user subsequently confirmed that Option+D still starts/stops. The physical
trace contains both complete toggle callbacks and a complete Escape press,
acceptance and callback, followed by release and unregistration. There is no
`incomplete` marker. The live UI read-back showed **Diktat abgebrochen**. The user
reported that Escape did not insert into Notes. That is the specified cancellation
behavior, not evidence of a failure: the second Option+D ends a recording for
transcription/insertion; Escape discards the current recording.

The initial report was misinterpreted as Escape failing to cancel. This observed
sequence establishes that global delivery and cancellation worked. It does not
establish every possible shortcut scenario or independently inspect Notes.
The structured result and log hash are in `physical-result.json`; the private raw
technical trace is retained in the ignored local artifact directory.

The opt-in marker and temporary instrumentation have now been removed. The
runtime code is restored to its pre-diagnosis version. The diagnostic commits and
this result remain in history rather than being squashed or rewritten.
