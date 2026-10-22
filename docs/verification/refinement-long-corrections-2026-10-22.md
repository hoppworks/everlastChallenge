# Corrections within long dictated text

Two additional synthetic German and English texts contain several corrections
across three paragraphs, alongside names, negation, an identifier and a quoted
number. The earlier 64-case corpus included long unchanged text but did not
establish this combination of length and required edits.

The previous worker rejected the German case and left the English weekday
correction unresolved. A diagnostic localized two causes: unsafe candidate spans
around spelled-out numbers, and a model decision that skipped a valid weekday
correction. Policy 7 recognizes bounded cardinal words when protecting the
surrounding phrase. Prompt 8 makes the independent assessment of each local
correction explicit.

Independent review found two additional boundary errors in the initial change:
ordinary `no one` could be mistaken for a number correction, and a subject such
as `Two tickets` could allow the following predicate to disappear. Both were
reproduced by failing public contract tests and corrected. Weak spoken-number
corrections now require a comma after the marker; number-leading subjects do
not exempt intervening lexical context from protection. Local valid corrections
remain covered, including German compound and English multiword numbers.

## Model and software results

A freshly built production MLX worker completed all 66 cases in one serial run.
A separately built validator using the production Rust core accepted all 66
proposals. The existing 64 expected outputs remained unchanged and matched.
Both new long cases removed the two fillers and corrected the weekday and
quantity while preserving the remaining content.

The initial expectations for those two cases contained a test-authoring error:
they left sentence starts lowercase after deleting the preceding filler. The
worker correctly capitalized those starts. Evaluation against the original
expectations was therefore 64/66. Correcting only those four initial letters
produced 66/66. The inputs, initial mismatch results and original expectations
are retained. This correction occurred after diagnostic outputs had been seen;
the resulting expectation is not an independently blinded oracle.

The [measurement record](refinement-long-corrections-2026-10-22.json) binds the
worker, model, source files and both corpus versions, and includes the two long
input/output pairs. The focused core suite passed 57 tests; the portable
protocol/worker suite passed 60, with one separately scoped test ignored. Rust
formatting, Clippy and the fast repository checks passed. Core line coverage was
97.39%, above the unchanged 90% floor; frontend coverage also passed.

These are finite text-processing observations. They do not establish human
linguistic acceptance, quality on arbitrary dictations, final signed-app
operation, observed target receipt or comparative performance. No speedup is
inferred from this run.

A [later numeric-context check](refinement-numeric-context-2026-10-22.md) adds the
captured ASR form with digits and records prompt 9, preserving these 66 outputs.
