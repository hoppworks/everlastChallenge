# Local text refinement worker

A dedicated Apple Silicon macOS process owns Qwen3 inference through MLX.
The desktop invokes it after ASR when local text refinement is enabled and the
processing policy requires it. The [single-generation qualification](../../docs/verification/refinement-single-pass-2026-10-22.md) records the current finite text tests; app-package verification is a separate boundary.

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

A refinement request invokes the local model at most once. It returns a complete
text, constrained to the original word sequence and source-bound correction
alternatives. Rust reconstructs the corresponding edits and validates the
result before deterministic finishing. That finishing covers explicit digit-to-
digit corrections with strong correction markers, comma-delimited initial
hesitations, and permitted sentence casing
and terminal punctuation. The final result is reconciled against the raw ASR
text again; the host independently validates the returned stages for the active
original and session. Invalid, incomplete or conflicting results never authorize
automatic insertion.

Validation anchors stay in Rust. Repeated source spans are identified to the
model by their one-based occurrence. Grammar validity is not semantic acceptance:
a model can still leave a necessary correction unapplied. Historical findings
are recorded in the [pipeline report](../../docs/verification/refinement-pipeline-2026-10-22.md).

Protocol 2 carries structural and punctuation edits as separate validation
stages; these are not separate model calls. Current prompt and policy identities
come from the shared [protocol constants](../text-refinement-protocol/src/lib.rs).
The complete request shares a 30-second deadline, a 4,096 generated-token budget
and a 32-KiB output-byte budget. The model prompt is limited to 8,192 tokens and
the generated grammar to 512 KiB. The tokenizer vocabulary is reusable, while
every request creates an independent parser and matcher. The existing ASR
inference path does not use this grammar.

The grammar offers finite punctuation separators, retains literal atoms, and
preserves separators between bare digits and the following word. Capitalization
alternatives require an allowed sentence boundary. The Rust validator separately
checks quotes, values, whitespace, overlaps and combined edit counts. It protects
these boundaries even if a future grammar change admits an invalid candidate.
A failure retains the original through the existing host error path; there is
no second-model repair attempt or partially accepted result.

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
