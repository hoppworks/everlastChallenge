# Context-bound self-corrections

This change follows `a286897` and advances the text policy to version 6.
An independently authored German/English extension exposed three failures in
23 cases: a numeric correction was withheld, and two explicit restatements
produced incorrect sentence fragments. The corpus was frozen before inference.
Its author was already familiar with the implementation; this is not a blind
study or a statistical quality estimate.

The numeric case offered an edit that could remove the preceding predicate as
well as the old number. The model selected that edit together with the shorter
numeric edit, so overlap validation correctly withheld the result. Candidate
validation now rejects that predicate-consuming span.

The statement cases exposed a different problem: a source-bound replacement
could still be linguistically incomplete. Checking that every replacement word
came from the original did not make the resulting sentence correct. The new
context check requires repeated words around the correction marker to be owned
by the correction span. It examines bounded six-word windows within the current
clauses. Complete short values and names remain usable; incidental repeated
digit values are not treated as repeated grammatical context.

Explicit complete restatements may add or remove German `nicht` or English
`not` when all other words match. English supporting `do` is handled with `not`.
Ordinary negation remains protected. This is a bounded structural policy, not a
general parser, a guarantee of semantic equivalence, or permission to generate
new prose.

Independent review caught overly broad intermediate rules that rejected spelled
numbers and prepositional time corrections. It also found incidental-number/name
regressions and a numeric-leading-clause bypass. These cases have positive or
negative regression tests. The final targeted review found no remaining
reportable issue in that change scope.

All 53 focused refinement tests passed, covering policy, source-bound candidates,
validation, punctuation and atomic staging. Core/protocol/worker/desktop Clippy
also passed. Unchanged audio and UI evidence is not rerun or broadened by this
report. Final packaged operation, observed target receipt, memory-pressure
coexistence, human linguistic acceptance and comparative performance remain
separate gates.

## Real model result

The corrected worker completed the original 41 cases and all 23 independently
authored cases in one serial run. All 64 outputs passed independent Rust
validation and matched a frozen acceptable output. The
[case record](refinement-context-cases-2026-10-22.json) retains the initial 20/23
result and corrected 64/64 result with worker/model identities. No cases were
removed or weakened after observing their outputs. These observations do not
constitute human linguistic acceptance or a comparative performance result.

## Native host startup and cancellation

A separate real-worker test exposed a host-only startup failure: temporary
directories inherited ordinary `0755` permissions, but model loading requires a
private parent. The host now creates the directory atomically with `0700`.
A failing then passing supervisor assertion checks the created directory.
This explains why raw worker tests using already-private directories passed
while the host path still failed.

After this correction, a native host-supervisor run loaded the pinned model and
started a long request. A one-second macOS call-graph sample showed
`MlxGenerator::infer`, `QuantizedMatmul::eval_gpu` and Metal dispatch calls.
Cancellation then returned `Cancelled` without accepted text. The child exited
without forced termination, its scratch directory was removed, and a fresh
worker completed the following request and cleaned up normally. The
[native observation](refinement-native-lifecycle-2026-10-22.json) preserves the
observed interval and selected call-graph entries. Its roughly 97-ms
cancellation-to-join interval is one observation, not a latency guarantee or a
comparison with another product.

The [source manifest](refinement-context-sources-2026-10-22.json) binds the changed
validation and host-startup paths. This test is not the full desktop bundle,
actual microphone/recipient acceptance or an OS memory-pressure campaign.
