# Single-generation text refinement

The local worker passed **71/71 text cases**, with every result accepted by a
separately built Rust validator and matching a previously specified output.
The complete inputs, allowed outputs, actual results and binary/source hashes
are in the [qualification record](refinement-single-pass-2026-10-22.json).
The model assets remain fixed by the worker's
[manifest](../../crates/text-refinement-worker/src/model-manifest.json).

## Processing contract

Each refinement request invokes the local model at most once. The model returns
one complete text, constrained to original words and source-bound correction
options. Rust validates that result, performs narrowly defined deterministic
finishing, then validates the complete result against the raw transcript again.
The host independently validates the returned structural and punctuation stages.
Those stages represent validation boundaries, not additional model calls.

Finishing handles comma-delimited initial hesitation, explicit digit corrections
with strong correction markers, and permitted sentence casing/terminal
punctuation. Quotes, ordinary negation, literal values and unrelated words remain
protected. Paragraph whitespace stays at its original word boundaries. Invalid,
conflicting or incomplete output retains the original without automatic insertion.

The immutable tokenizer vocabulary is reused. Each request still receives a
fresh grammar parser and matcher; generated state is not reused across requests.

## Verification

| Check | Result and scope |
| --- | --- |
| Production MLX worker | 71 sequential requests; 71 accepted, 71 exact allowed outputs |
| Independent Rust validation | All 71 returned proposals validated against their complete raw input |
| Core tests | 95 passed, including source binding, staged edits and session invariants |
| Single-generation contract tests | 11 passed: one call, no retry, invalid output, literals, correction conflicts, exact initial position and paragraph protection |
| Native grammar tests | 9 passed with controlled logits, including fresh parser state, long gaps, numeric punctuation and leading/trailing paragraphs |
| Process supervisor | 28 passed, one existing test ignored; unchanged supervisor implementation |
| Static checks | Focused Rust Clippy, formatting and `npm run check:fast` passed |

Regression work found and fixed lost paragraph breaks, punctuation duplicated
beside a locked numeric atom, and an initial-hesitation check that relied on
truncated context. Tests exercise the actual worker boundary and the grammar
processor; a successful grammar parse alone is not treated as correct text.

## Limits

This is a finite, synthetic text corpus, not an accuracy percentage for arbitrary
speech. It includes German and English, independent corrections, long texts,
quoted material, numbers, identifiers, negation and instruction-like dictation.
The source transcript is the input: the worker cannot establish what was spoken
or safely repair an unrelated ASR substitution.

The record qualifies these exact worker sources and model assets. It does not
prove native microphone capture, app-package integration, target receipt,
offline delivery or a performance improvement. Earlier package and
[comparison results](native-comparison-2026-10-22.md) remain bound to their own
revisions. This change does not reinterpret or extend that comparison campaign.
