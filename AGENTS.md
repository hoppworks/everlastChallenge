# Working in this repository

This is a native macOS dictation challenge: Rust/Tauri 2 owns recording, VAD,
inference, global shortcuts and delivery; React/TypeScript owns the visible UI.
The current product uses the qualified MLX hybrid. Pure-MLX research belongs in a
separate worktree/session and does not hold up this release.

## Before a change

Read the affected path in `docs/code-map.md` before architecture or lifecycle
changes. Use `docs/guardrails.md` for enforced checks and package.json for commands.
Preserve unrelated changes and commit coherent changes with explicit file staging.

Personal correspondence, account/billing information, submission checklists,
private task instructions and unpublished planning belong under `.private/`,
which is ignored by Git. Do not add them to tracked documentation or commit
messages. Public documentation describes product behavior, reproducible evidence,
technical limitations and dependency attribution.

## Boundaries

- Keep OS and inference dependencies outside `dictation-core`. Keep native FFI in
  `macos-integration`, with safety arguments covering ownership and thread affinity.
- UI accesses native functionality through `src/ipc/`. Generated contracts come
  from Rust; change their source and regenerate instead of editing generated files.
- Keep queues bounded, callback work predictable and stop/cancel session-bound.
  Treat sending a paste event and observing target text as different evidence.
- Review recoverable defaults and fail-closed paths explicitly. Handle Result and
  Promise failures; preserve a usable error state instead of hiding failures.
- Explain necessary lint exceptions beside the rule or in `docs/guardrails.md`.
  Do not turn a disabled check into a claim that the behavior is verified.

## Verification

Use package.json as the command index. Start with `npm run check:fast`; run the
focused behavior or guardrail tests for changed contracts. Rust changes also need
format/Clippy and relevant Cargo tests. `npm run setup` installs pinned development
tools and the local hook. A successful local hook is not proof of a remote CI run.

Reuse valid source-bound evidence for unchanged paths. A documentation change
needs no new microphone or model campaign. Run broad integration checks once on a
stable candidate. Never run performance measurements alongside builds or another
benchmark. Keep failures and retries in the evidence record.

For packaging, bind the executable, frontend resources, native libraries and
complete history to their hashes. Preserve signature metadata when copying apps.
Reuse an existing signed build only after proving its runtime inputs are unchanged.

## Evidence

Keep technical observations scoped to the tested revision and environment.
Do not infer a passed remote test from a local result or a configured workflow.
Document AI-assisted implementation and actual verification in `docs/ai-review.md`.
