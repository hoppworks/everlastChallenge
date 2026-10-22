# Local text refinement worker

A dedicated Apple Silicon macOS process owns Qwen3 inference through MLX.
This crate is not yet connected to the desktop dictation flow.

The executable takes three positional arguments: the installed model directory,
a nonzero worker generation, and a private absolute directory owned by its
supervisor. The supervisor must remove the last directory **after waiting for
process exit**, including forced termination. It must not rely on child-process
destructors for cleanup.

The worker copies the pinned assets into that directory, checking size and SHA-256
while copying. It loads only this private snapshot, retains it through model
cleanup, and applies the hash-bound tokenizer adapter in memory. Cache changes
cannot change the model being loaded. Processes running as the same user are
within the trust boundary.

Standard input/output carry the bounded `text-refinement-protocol` frames.
Standard error contains only static failure messages. One request can be active;
requests carry session, generation and monotonically increasing request IDs.
The input thread never performs inference or blocking output. One output frame
can be queued behind the writer; further backpressure terminates the process.
The host must enforce startup/job deadlines and kill an unresponsive worker.

The model selects a complete boolean vector over source-bound Rust options and
may propose additional punctuation edits. The selection decoder checks vector
length and validates the complete combination before returning proposals. The
host must still validate the response for its active original/session. Current
model-quality limitations are recorded in the
[candidate report](../../docs/verification/refinement-candidates-2026-10-22.md).

Validation anchors stay in Rust and are not exposed as option context. If a
source span occurs repeatedly, model options give its one-based occurrence.
This representation does not change the exact-anchor checks on returned edits.

Policy version 3 constrains generation with a request-owned JSON grammar:
exact decision count, known fields and bounded punctuation lists/string lengths.
The complete Rust validator still checks source spans, UTF-8 byte limits,
overlaps and combined edit counts. Grammar-valid output can still omit a needed
correction; it does not establish semantic quality. The existing ASR generation
path does not use this processor.

Policy version 4 additionally rejects punctuation edits that introduce doubled
full stops, commas, colons or semicolons inside a replacement or at a source
boundary. Existing ellipses can remain unchanged. This is a structural check;
it does not decide whether a sentence boundary is linguistically appropriate.
