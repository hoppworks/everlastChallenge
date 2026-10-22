# Numeric corrections in long transcripts

A captured transcript of the synthetic German long-dictation fixture exposed a
context-dependent omission. The previous worker changed the weekday but retained
`60, Entschuldigung, 16 Kartons`. The same quantity correction worked in a short
sentence. Both valid correction options were present, so candidate generation
and source matching were not the cause of the omission.

Prompt 9 separates instructions for hesitation removal and explicit corrections,
explains that digit and number-word corrections use the same rule, and preserves
the occurrence marker for repeated source spans. Protocol 2, policy 7, the model,
JSON serialization, validation and execution flow remain unchanged. The host and
worker share the prompt version and reject an incompatible worker identity.

## Verification

A freshly built production worker processed the 66 existing cases and two added
numeric-context regressions in one serial run. An independently built validator
using the production Rust core accepted all 68 proposals. All 68 outputs matched
the declared acceptable outputs; all 66 previous outputs were unchanged.

The added cases are the short quantity sentence and the captured long transcript.
The latter now corrects both the weekday and the quantity while preserving the
other words, including the original ASR spelling. An earlier prompt candidate
fixed the quantity but omitted a later English hesitation removal; it was rejected.
The final prompt also passes that unchanged long English case.

The [measurement record](refinement-numeric-context-2026-10-22.json) includes the
new input/output pairs, previous outputs, worker/model/source hashes and corpus
identity. It reuses the existing corrected capitalization expectations described
in the [earlier long-text report](refinement-long-corrections-2026-10-22.md).
For the added long case, both the unchanged punctuation and the already observed
comma after `Zitat` were allowed before the final run.

Rust formatting, the fast repository checks and scoped Clippy checks passed.
The portable refinement suite passed 61 tests, with one separately scoped test
ignored; that ignored test is not counted as executed evidence.

These are finite text-processing observations. They do not establish human
linguistic acceptance, arbitrary-input correctness, a newly distributed app's
microphone or delivery path, or comparative performance.

The later [packaged checks](refinement-packaged-2026-10-22.md) add actual native
receipt of the long numeric correction and separately scoped offline processing.
