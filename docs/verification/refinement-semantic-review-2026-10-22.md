# Semantic review of local text refinement

Two AI-assisted readings examined all **68 recorded input/output pairs** from
prompt 9: 60 distinct pairs, 28 changed outputs and 40 unchanged outputs. No
blocking semantic defect was identified in this corpus. This is not human
approval or a general accuracy guarantee.

The review compared the originals with the actual validated outputs, rather
than treating agreement with an expected-output fixture as sufficient evidence.
It covered final corrected values, names, numbers, negation and uncertainty,
quotations, URLs, identifiers, code, language and paragraph boundaries. Long
cases were read in full. Duplicate cases remain individually accounted for.
The [record](refinement-semantic-review-2026-10-22.json) binds the worker and raw
corpus evidence by hash and includes a rationale and text hashes for every case.
The underlying machine run is described in the
[numeric-context report](refinement-numeric-context-2026-10-22.md).

## Retained qualifications

- `en-correction` treats the unpunctuated phrase `Thursday, no Saturday` as an
  explicit correction to Saturday. This matches the intended fixture, but the
  sentence alone admits another reading. It does not establish general recovery
  of ambiguous intent.
- `observed-long-asr` preserves the source words while adding an awkward comma
  after the quotation introduction. The result preserves the proposition but
  does not fully repair the quotation's punctuation. The existing ASR spelling
  `ab 204` is also preserved; refinement is not credited with restoring `AB-204`.

The finite corpus includes development-exposed and synthetic cases. These
readings do not measure spontaneous-speech accuracy, statistical error rates,
actual application delivery or comparative performance. No model inference,
audio campaign or broad test suite was rerun for this document-only review.
