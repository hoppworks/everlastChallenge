# Atomic validation of structural and punctuation edits

This core-only change follows `b84c4e9`. It adds an atomic validation contract for
a structural edit stage followed by punctuation and sentence-initial case edits.
It does not yet change the desktop's worker protocol or inference path.

A flat edit list cannot represent punctuation within a corrected source span
without overlap. For example, changing “Thursday, no Tuesday” to “Tuesday.” needs
both a correction and punctuation on the surviving value. The existing flat
validator correctly rejects overlapping original-source edits.

`StagedEdits` instead carries two explicit lists. The structural list permits
only filler and self-correction edits; the punctuation list permits only
punctuation/case edits. The existing full validator checks the first list against
the raw original and the second against the exact intermediate result. The
combined budget remains 32 edits. No overlap, content or protected-literal check
is removed from either validation.

The function returns a result only when both stages pass. That result retains the
raw original and language as its authority, with the combined edit count. It
never exposes a partially accepted intermediate as the final result.

Tests cover punctuation of a corrected value, second-stage failure, incorrect
stage roles, the combined limit, stale intermediate anchors, protected values,
negation, empty proposals and strict serialized fields. Independent review found
no concrete blocker in this contract. The [source manifest](refinement-stages-sources-2026-10-22.json)
binds the checked implementation and tests.

All 79 core tests passed, including six new staged-contract tests. Core Clippy,
Rust formatting, fast repository checks and the source secret scan also passed.
The serialized-shape tests use the existing workspace `serde_json` version as a
test-only dependency; the core gains no runtime or inference dependency.

This proves a software validation boundary, not model semantics. Worker/protocol
integration, the broader linguistic corpus, native delivery and performance
still need their own evidence. The application continues using its prior path
until that integration is complete.
