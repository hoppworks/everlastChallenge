# Punctuation provenance in accepted edits

A local model diagnostic proposed replacing `Uhr` with `Uhr.` in text that
already ended with `Uhr.`. The validator accepted `Uhr..`. A public regression
reproduced this before the fix and passed afterward.

Independent review found a second path: in `Hallo äh. Welt`, adding a point after
`Hallo` and removing `äh` produced `Hallo.. Welt`. The initial boundary check
missed the later deletion. A second regression reproduced that failure.

## Corrected behavior

The validator now observes the actual complete composition. Repeated full stops,
commas, colons and semicolons are accepted only if both characters already
neighbored in the original, with at most whitespace between them. Inserted
punctuation has no original position. Case/punctuation replacements retain
positions only for unchanged punctuation gaps; correction suffixes and a retained
filler comma refer to their original positions.

This preserves existing ellipses through capitalization, filler deletion and
explicit correction. Removing an old ellipsis cannot authorize a new repeated
point elsewhere. A failure rejects the entire candidate; no partial text is
returned. The worker policy version is now 4; prompt/input version remains 6.

Eight added public contract tests cover the original failure, both boundaries,
internal and spaced repeats, combinations of punctuation edits and filler
deletion, original ellipses, lack of transferable punctuation allowances, and
UTF-8 origins after multibyte prefixes. The independent targeted follow-up found
the mixed-edit issue closed and no further concrete blocker in the revised guard.

## Scope and evidence

Local checks passed: `cargo test -p dictation-core --locked` (70 tests), the
worker/protocol test run with the explicit process-fixture feature (41 tests),
format, and Clippy for core/worker with all targets and warnings denied. The
optimized worker build also passed. No microphone or performance campaign was
repeated for this deterministic acceptance change.

The deterministic rule does not establish that a proposed sentence boundary is
linguistically correct. The alternate model representation tried during the
diagnostic was not integrated: it still omitted an English correction and
proposed an inappropriate sentence break. Existing model-quality failures remain
open. Earlier model reports are historical observations of their bound policies;
this fix does not turn them into successful quality acceptance.

Source SHA-256:

| File | SHA-256 |
|---|---|
| `crates/dictation-core/src/refinement/punctuation.rs` | `fcb67c9537f3fa7fb6f685b9b24abbd1c9c10fdd2df13b67344a3ec70b5b92bf` |
| `crates/dictation-core/src/refinement/validator.rs` | `1e5cc3c44d24326625dbd63317f0c0e36a4f4bd5cdf8a1eca9aaf7eabf4d62d3` |
| `crates/dictation-core/tests/refinement_contract.rs` | `8d9b5bb1ec22a1807fa9d352b4ff0e43044cdb4e257d20e7c9ad13452c9156b5` |
