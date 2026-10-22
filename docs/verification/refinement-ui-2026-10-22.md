# Refinement settings and result presentation

This package follows `84b32fd`. It connects the text setup service, saved
preference and controller state to generated renderer contracts. It does not
qualify model quality or the final application bundle.

## Behavior

The Textbereinigung settings area shows local setup progress, cancellation and
retry. Activation requires verified model assets and an explicit preference
save. Existing installations remain disabled until that choice. Disabling stays
possible when setup has failed. The preference is frozen with each new session.

Both new commands require the existing local main-window authorization and
explicit capability entries. Accepted download requests count as busy before
their I/O worker starts; native starts and settings saves check that state.
Cancellation revokes the queued epoch or active setup work.

The palette distinguishes transcription from text refinement. During refinement,
the original remains readable and cancellation remains available. Failures and
aborts retain the original and explain that no insertion took place. An accepted
edit offers the original through a separate disclosure and copy action. Copy
always uses the displayed string through the existing native clipboard command.

Active session IDs become null after completion. Independent review found that
the first UI implementation incorrectly hid refinement results at that point.
Corrected tests use real idle/null-ID snapshots. Completed results retain their
own provenance; active-session displays still require matching IDs. A successful
new start resets old refinement metadata while retaining the last text. This
prevents an earlier failure from describing a later aborted recording.

## Verification

| Scope | Result |
|---|---|
| Frontend suite | 238 passed; affected IPC/result-view subset passed again after the idle-ID correction |
| Desktop contracts | 48 passed |
| Controller contracts, measurement/test-control enabled | 26 passed |
| Runtime contracts, measurement/test-control enabled | 49 passed; one existing native 300-second replay test not run |
| Application contracts | 7 passed |
| Preference transactions | 13 passed |
| Static checks | Rust formatting, host Clippy across targets, fast repository checks, generated-contract comparison and source secret scan |

The existing replay exclusion requires pinned models and audio. It is not counted
as passing. Targeted failing tests established the processing UI, setup gate,
idle-result bug and old-metadata reset. The first all-target compile identified
an observer fixture missing the newly required snapshot field; that fixture was
updated. Static checks also identified missing switch ARIA state, which is now
explicit.

A separate visual reviewer checked eight captures of the actual React components
with simulated data: settings missing/ready/downloading, processing, failure and
accepted edits, using light/dark themes and 640/480-pixel widths. The inherited
palette, system typography and progressive disclosure were preserved. Five
detector advisories concerned unchanged CSS declarations. These captures are
presentation evidence, not native microphone, keyboard, VoiceOver or inference
tests.

Helper packaging, complete offline execution, actual target receipt and the
remaining model-quality cases require subsequent verification. No new speed
claim or performance measurement follows from this package. Source hashes are
bound in the [source manifest](refinement-ui-sources-2026-10-22.json).
