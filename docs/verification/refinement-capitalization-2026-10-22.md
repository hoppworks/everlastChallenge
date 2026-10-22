# Sentence capitalization in text refinement

A native recording produced the transcript `der termin ist am dienstag nein am donnerstag um 10 uhr`. The packaged text worker rejected
its proposed result with `invalid_candidate`. The application retained the
original and withheld automatic delivery. Replaying the exact transcript in the
same worker reproduced the rejection; capitalized variants succeeded.

The punctuation grammar offered title case for every word, while the Rust
validator permits a case change only at a sentence start. Policy 8 makes these
alternatives conditional on an existing sentence boundary or an immediately
preceding generated period, question mark or exclamation mark. A comma, colon or
semicolon does not grant permission to change case. The validator is unchanged.

A regression uses controlled MLX logits that prefer uppercase letters. Before
the fix, the grammar admitted `Der: Termin: Bleibt`, which the validator rejected.
The corrected grammar passes this test and the existing punctuation and grammar
checks, including the long-input grammar compilation check.

The production worker and independent Rust validator accepted all four variants
of the observed transcript after the fix. The existing 68-case corpus also
passed with exactly the same outputs as before. This is bounded fixture evidence,
not a general linguistic guarantee. Non-initial words retain their original case
unless a sentence boundary precedes them; lowercase German nouns can therefore
remain lowercase. This conservative restriction protects names and identifiers.

The implementation and regression were AI-assisted and independently reviewed.
Real-model replay is separate from native app delivery; the revised package still
needs its own native recording and receipt check. Machine-readable source and
result hashes are in the [evidence record](refinement-capitalization-2026-10-22.json).
