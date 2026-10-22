# Two-stage local text refinement

This integration follows `596855c`. Protocol 2 carries separate structural and
punctuation proposals. The worker and desktop host both validate the complete
result against the original dictation; a failed second stage cannot release the
first stage as a successful result.

The first model pass selects source-bound filler and self-correction options.
The second works on the validated intermediate text. Its finite anchored grammar
preserves the word sequence, existing whitespace and protected punctuation,
allowing bounded punctuation insertion and initial capitalization. Rust derives
local edits and applies the existing content and sentence-position checks.
Capitalization permitted by the grammar is not automatically accepted.

Both passes share a 30-second deadline, 4,096 generated tokens and a 32-KiB output
budget. Each prompt is limited to 8,192 tokens; grammar source to 512 KiB. The
combined accepted edit count remains 32. Empty structural option lists require
no first inference pass. Any failed, invalid or incomplete response retains the
original and withholds automatic insertion.

## Corrections found during verification

- Unchanged opening quotes beside a capitalization edit initially contaminated
  its source span. Trimming only unchanged surrounding gaps preserves the whole
  word anchor and lets the existing validator check the intended edit.
- Literal chat-token spellings could be encoded as special tokens by the grammar
  adapter, then rejected by its byte trie. The private adapter now tokenizes
  these spellings as literal bytes. Prompt tokenization and explicit EOS handling
  remain unchanged. A controlled-logit regression failed before this correction
  and completed with the exact literal afterward.
- A long fixed-word pattern intersected with a redundant `maxLength` automaton
  exceeded grammar compilation limits. The finite pattern already bounds the
  language, so the extra grammar constraint was removed. Runtime byte/token/time
  limits and independent validation remain enforced. The regression compiles a
  70-section transcript; the real-model diagnostic preserved all 70 sections.

The initial 41-case native model run returned 39 successful, validated outputs
and two generation failures. Those failures are retained in the evidence;
the corrected worker subsequently completed all 41 cases. Separate validation
against the original text accepted all responses, and exact-output checks matched
the expected preserved or corrected texts. The [case record](refinement-pipeline-cases-2026-10-22.json)
contains inputs, expected/actual outputs and worker/model identities. This is an
AI-assisted assessment of this fixed corpus, not human linguistic acceptance or
an independent unseen-corpus result. Diagnostic timings are not a benchmark.

## Software checks

All 83 core tests, 58 protocol/worker tests and six native grammar/processor tests passed.
One existing separately invoked test remains ignored and is not counted as passed. The native tests
include exact decision cardinality, terminal handling, processor failure, literal
control-token output and long-pattern compilation. Repository fast checks and the
source secret scan passed. Clippy passed for the core, protocol, worker and desktop
with their measurement/test-control/test-fixture features.

## Scope

The controlled tests cover real grammar processing, strict protocol decoding,
process fixtures, atomic validation and protected text. Independent review found
no remaining actionable issue in the reviewed changes. These checks do not prove
linguistic correctness for arbitrary dictation, native recipient delivery,
memory-pressure behavior during GPU work or a performance advantage. The larger
independent corpus and final packaged-application acceptance remain required.

The [source manifest](refinement-pipeline-sources-2026-10-22.json) binds the
implementation and regression tests discussed here.
