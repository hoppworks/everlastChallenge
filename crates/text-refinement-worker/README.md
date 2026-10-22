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
