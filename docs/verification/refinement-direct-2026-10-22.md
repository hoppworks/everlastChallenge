# Deterministic refinement routing

The host can complete a narrow subset of cleanup entirely in Rust. Other inputs
continue through the [single-generation local model](refinement-single-pass-2026-10-22.md).
The model, prompt, grammar, and model-result validation are unchanged.

The [source-bound record](refinement-direct-2026-10-22.json) contains all 71 frozen
text cases: 49 complete directly, including 34 previously unchanged inputs;
22 require the model. All 49 direct results match an existing allowed output.
Fallback results are not counted as successful direct cleanup.

## Contract

- Existing skip decisions preserve the exact original bytes.
- Direct edits require explicit comma-delimited correction markers and agreement
  between overlapping validated candidates. Other than strong digit corrections,
  the correction must end at a sentence boundary or a narrowly parsed clock adjunct.
- A complete quantity can also finish directly: one supported cardinal token on
  each side, explicit delimiters, and exactly one known unit or count noun before
  the sentence boundary. The vocabulary is closed; modifiers, trailing predicates,
  ambiguous abbreviations and multiword number phrases still require the model.
- Initial hesitation, casing, and final punctuation use the existing restricted
  finishing rules. Any remaining cleanup signal requires the model.
- The final result is validated against the complete raw original and language.
  An uncertain route passes the untouched original to the model; a validation
  error retains the original without inserting a partial result.
- Delivery still uses the existing session-bound token. Controller tests cover
  one delivery, revocation, and exactly one unchanged-original fallback job.

Independent review found that a comma-delimited negation can start a separate
assertion, and that a period inside a clock value is not a sentence boundary.
Both were reproduced by failing tests before correction. Inputs such as
`We can meet on Monday, no, Tuesday at 14.30 is unavailable.` require the model.
Dotted clock forms deliberately fall back; the direct clock grammar accepts only
ASCII digits and colons.

A frozen long transcript additionally verifies independent weekday and quantity
corrections together. The complete output matches the existing expected text;
names, negation and unrelated sentences remain unchanged. Eight direct-route tests
cover positive completion and negative boundaries. The existing 71-case result is
unchanged. This additional text replay does not establish native audio latency.

## Limits

These are finite text fixtures, including duplicates, not a general linguistic
accuracy rate. A source-bound edit can still require semantic judgment; the direct
route deliberately declines many valid corrections. These checks do not establish
microphone accuracy, package behavior, or a measured speed improvement.
The earlier model qualification and native comparison remain historical records
of their own source and runtime identities.
