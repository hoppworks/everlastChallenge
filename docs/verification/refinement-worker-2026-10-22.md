# Local text worker: implementation checks

This records checks of the isolated worker, not acceptance of text refinement in
the desktop application. The host supervisor, model installer and UI integration
are still pending.

## Executed checks

- Thirteen focused tests passed: eight lifecycle tests and five manifest/adapter
  tests. They cover bounded admission, job-scoped cancellation, malformed input,
  parent EOF, shutdown during loading, blocked output, template read limits,
  symlink rejection and invalid/cancelled snapshots.
- Worker/protocol Clippy, Rust formatting, `check:fast`, and Rust dependency
  checks passed. A successful local check is not a remote CI result.
- Independent review found output backpressure blocking control input, a gap
  between verified and loaded bytes, and snapshot cleanup relying on child
  destructors. The changes separate bounded output, retain a verified private
  snapshot and require a supervisor-owned cleanup directory. Targeted re-review
  found no remaining concrete blocker in this worker scope.

A test initially detected a duplicate disconnect notification after parent EOF;
shutdown now suppresses the late result. A later Clippy pass rejected an ignored
send result in a test callback; it is explicitly handled. These failed attempts
are not counted as passing runs.

## Real model smoke test

The release worker ran serially with the pinned Qwen3 model, no concurrent build
or benchmark. It loaded, emitted its manifest identity, handled three requests,
and exited successfully after Shutdown. The supervisor test harness removed its
owned snapshot directory after process exit.

Worker SHA-256:
`fb77ee78dc4913582cb8bf3076fe2f6f8f4be687fb96b58424d6dd3aab82ebef`.
Manifest SHA-256:
`fea7cc2a764e60d910da77c44549b7f123e681edb0c84f03658011eb86519352`.
Prompt/policy versions: 1/1.

| Input | Observed proposal | Status |
|---|---|---|
| Wir treffen uns am Montag, nein Dienstag um neun Uhr. | No edits | Required correction missed |
| We meet on Monday, no Tuesday at nine. | No edits | Required correction missed |
| Das ist, äh, ein einfacher Test. | Remove `, äh,` | Expected filler proposal |

The two missed corrections mean **model quality has not passed**. An empty edit
list is not evidence that required cleanup succeeded. These three smoke cases
are neither a quality corpus nor a performance comparison. No native text
receipt, desktop cancellation, offline package operation or product performance
claim follows from them.

## Ownership contract still to integrate

The supervisor must create the private snapshot parent before launching the
worker, retain ownership, enforce deadlines and remove it only after confirmed
process exit, including a hard kill. Model copies consume additional disk space;
installation and startup must account for it. Another process with the same
user privileges is inside the trust boundary.
