# Local text refinement worker

A dedicated Apple Silicon macOS process owns Qwen3 inference through MLX.
The desktop invokes it after ASR when local text refinement is enabled and the
processing policy requires it. Model-quality qualification remains incomplete.

The executable takes three positional arguments: the installed model directory,
a nonzero worker generation, and a private absolute directory owned by its
supervisor. The supervisor must remove the last directory **after waiting for
process exit**, including forced termination. It must not rely on child-process
destructors for cleanup.

The directory contains a versioned `.owner.lock` marker. Host and worker hold
separate shared file locks while they own it. At application startup and before
spawning a worker, the host serializes recovery and creation with a directory
lock. It removes recognized abandoned directories only after acquiring their
exclusive lease. Live leases, unmarked legacy directories and unknown markers
are preserved; cleanup does not guess ownership from a PID or timestamp.
Losing the parent pipe terminates the dedicated process even during a blocked
GPU operation. An abrupt process exit can leave the snapshot for later recovery.

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

The first model pass selects a complete boolean vector over source-bound filler
and self-correction options. The second pass adds punctuation and sentence-initial
case to the exact intermediate text using a grammar that fixes the word sequence.
Rust derives local punctuation proposals and validates both stages atomically.
The host independently repeats this validation for its active original/session;
the final accepted value remains bound to the raw ASR text. Current
model-quality limitations are recorded in the
[pipeline report](../../docs/verification/refinement-pipeline-2026-10-22.md).

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

Protocol 2, prompt 7 and policy 6 carry both stages explicitly. They share the
30-second request deadline, 4,096 generated-token budget and 32-KiB output-byte
budget. Each model prompt is limited to 8,192 tokens. No first model call is
needed when the structural option list is empty. The punctuation grammar retains
existing whitespace and offers bounded punctuation insertion at plain spaces;
its generated pattern is capped at 512 KiB. A failed or incomplete second pass
does not return the first pass as a fallback.

## Host service

The host's [asynchronous owner](../../src-tauri/src/refinement_service/owner.rs)
wraps the process supervisor on a dedicated thread. Its single slot includes a
pending/active job and an unconsumed completion. Loading is deferred until a job
arrives; a healthy process can serve later sessions with fresh request context.
The idle deadline and explicit unload close and reap the process before removing
its snapshot. Unload also revokes an unconsumed completion. A clean child idle
exit permits a new process for a later explicit job; an idle crash closes the
service without a restart loop. The caller must still recheck session authority
after dequeuing a result and before delivery.

Public macOS memory-pressure warnings also request unload. Active refinement
retains the original without automatic insertion; ordinary ASR can continue.
New refinement jobs are held while pressure remains elevated. A return to Normal
allows later explicit jobs and never retries a discarded job automatically.

`npm run test:refinement:process` exercises the supervisor and owner with real
fixture children. These fixtures do not load MLX or prove dictation/UI integration.
The [owner report](../../docs/verification/refinement-owner-2026-10-22.md) records
its original verification scope. Later runtime, UI and packaging reports extend
that evidence; none alone proves final model quality or end-to-end delivery.

Policy 6 checks repeated source phrases around a correction so a partial
restatement cannot leave them behind. It also rejects spans that absorb an
unrelated predicate into a numeric correction.
Complete explicit restatements may add or remove negation only when the remaining
words match; this does not authorize deleting ordinary negation.
