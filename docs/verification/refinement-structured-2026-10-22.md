# Grammar-constrained local text output

Policy 3 constrains the experimental MLX text worker during generation. A fresh
grammar enforces exactly one boolean per source option, known object fields,
bounded punctuation lists and string lengths. The complete Rust validator still
checks actual UTF-8 bytes, exact source spans, overlaps and the combined edit
limit. The grammar does not decide whether an edit is meaningful or required.

The additive MLX generation entry points own their request processors and reuse
the existing sampling, cache and text-stream implementation. Default callers use
an empty processor list. No model arithmetic or ASR policy was changed.
The schema uses the existing upstream
[llguidance JSON-schema support](https://github.com/guidance-ai/llguidance/blob/main/docs/json_schema.md).

## Software verification

A deliberately ignored processor first reproduced tokens `[3,3]` where the
contract required `[2,2]`. After wiring processors before sampling, that test
passed. Its first history assertion incorrectly included the already-prefilled
prompt prefix; it was corrected to the decode-tail contract. Additional tests
exercise a recoverable processor error and actual grammar masking through the
text stream with a tiny byte tokenizer and deterministic model. Empty and
two-decision outputs must complete correctly despite the model preferring an
early list close. The ordinary unprocessed path still produces `[3,3]`.

The focused run passed 9 protocol, 17 existing worker, 3 structured-generation
and 12 supervisor tests. Rust format, focused Clippy, `check:fast` and the source
secret scan passed. Clippy initially rejected two redundant test casts; they
were removed and the affected checks passed. The optimized worker build passed.

The initial license check rejected the newly introduced MIT-0 utility
`borrow-or-share 0.2.4`. Its original permission/disclaimer text was inspected and
retained, and a version-scoped exception was added. License/source checks then
passed; the existing unused webpki exception warning remains. The updated runtime
inventory includes both the desktop and the worker. Independent combined review
found no concrete code blocker and prompted correction of an inherited comment
that incorrectly claimed the tokenizer JSON bridge eliminated a second version.

## Real model diagnostic

The [recorded inputs and responses](data/refinement-structured-2026-10-22.json)
contain all 18 diagnostic cases, including failures. The release worker used
prompt/input 6, policy 3 and the pinned Qwen3 manifest. It exited successfully
with no stderr output. Sixteen candidates passed complete Rust validation;
two punctuation cases were rejected. Empty-choice cases now return valid empty
decisions. A separate throwaway decode loop accepted all eighteen structurally;
that loop is not the worker and its results do not supersede the worker results.

Content acceptance still fails: the English weekday correction is omitted,
only the first of two German corrections is applied, and sentence-boundary
punctuation remains incomplete or rejected. Removing a sentence-initial filler
also leaves a lowercase first word in the diagnostic outputs. Validator acceptance
therefore must not be reported as complete cleanup success.

No performance comparison, application integration, offline package acceptance
or remote CI result is established by these checks. Diagnostic timing is not a
calibrated product latency measurement. Quality qualification remains open.

The measured worker executable SHA-256 is
`3fb8b7747a04dc3caebe9fe438b3eebfc049b90ca61171e5a343f9f3efe844b3`.
The data file binds the relevant runtime source files by SHA-256. Subsequent
edits in this package only added the default-path test and corrected comments;
the runtime logic used by the diagnostic is unchanged.
