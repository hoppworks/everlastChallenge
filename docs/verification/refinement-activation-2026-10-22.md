# Activation after explicit model setup

The text-model setup action now requests activation as well as installation.
After successful file verification, the native desktop owner saves the enabled
preference through the existing atomic settings transaction. The settings window
can be closed while this completes. An existing model found during application
startup does not enable the feature by itself, and an explicit saved preference
supersedes an outstanding activation request.

Activation waits while setup, dictation, settings changes or suspension make it
unsafe. The request is tied to a newer setup revision and a unique ticket. A
cancelled ticket cannot be reused by a later setup attempt. The native owner
claims the ticket immediately before beginning the settings transaction;
cancellation before this point prevents the queued save from executing.
Failures, including those before the transaction starts, remain visible and do
not silently change the preference.

The activation decision and enable validation read current setup state and queue
occupancy together. They do not infer readiness from a potentially delayed UI
snapshot. This matters when a previous attempt's ready or failed notification
is still displayed while a retry is being processed.

## Verification and scope

Independent review found two errors in the initial change: queued activation
survived cancellation, and early save errors could disappear into a discarded
response. Failing tests reproduced both at the queue/operation boundary before
the correction. A separate negative control substituted the delayed published
snapshot for current setup state; the status regression failed as expected.

The focused desktop suite passed 45 tests. It covers ticket revocation, retry
identity, busy deferral, explicit preference precedence and error publication.
The application test additionally exercises current-versus-published setup state
and queue occupancy. The settings suite passed 23 tests; it verifies the setup
action and rendering of the preference subsequently saved by the native owner.
These are software tests, not a new native-window interaction or physical device
removal test. The operation-boundary error fixture injects the same class of
early failure without claiming to unplug hardware.

Rust formatting, Clippy, fast repository checks and frontend coverage passed.
The [source and check record](refinement-activation-2026-10-22.json) binds the
changed paths and measured host coverage. The new activation module has an
explicit per-file coverage floor; line counts include its inline tests.

Ready at this boundary means the model files were verified. GPU model loading
remains lazy, on the first required refinement request; this change does not
establish a separate GPU startup preflight during installation. Final packaged
interaction, human text review and comparative performance remain separate
evidence. The inference worker and its 66-case record are unchanged by this
activation change.
