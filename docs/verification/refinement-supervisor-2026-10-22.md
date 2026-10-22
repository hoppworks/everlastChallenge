# Local text worker process ownership

The host now has a process supervisor for the experimental text worker. It
launches an explicit executable path, checks the complete readiness identity,
bounds pipe queues and stderr, enforces startup/request deadlines, and validates
returned edits against the original transcript. Cancellation terminates the
owned worker before removing its model snapshot. This module is not yet wired
into the application's dictation or delivery flow.

## Executed checks

On macOS, twelve tests exercised the actual supervisor with a separate Rust
fixture process: normal completion, deadlines, cancellation and late responses,
readiness mismatch, malformed input, stdout/stderr floods, blocked stdin,
replayed requests, invalid edits, owner drop and failed snapshot cleanup.
The fixture is available only through an explicit test feature; a new architecture
regression rejects enabling it through production feature aliases.

Independent review identified a misleading cleanup error flag. A test made the
snapshot directory non-writable, reproduced the incorrect flag, and passed after
the correction. Both initial and repeated close now report cleanup failure.
The targeted independent follow-up found no remaining issue in that correction.
An earlier malformed-startup test used an unnecessarily short startup deadline;
it was corrected to distinguish malformed output from intentional timeout.

The following local checks passed:

- `cargo fmt --all -- --check`
- `npm run check:fast`
- `npm run test:refinement` (9 protocol, 17 worker, 12 supervisor tests)
- `npm run check:refinement`
- `cargo clippy -p everlast-dictation --lib --locked --offline -- -D warnings`
- `node --test scripts/architecture.test.mjs` (7 tests)
- `node scripts/secrets.mjs`

Source SHA-256 bindings for the tested process behavior:

| File | SHA-256 |
|---|---|
| `src-tauri/src/refinement_service.rs` | `16d5a7b94b897021a35f3f071af7d42ee140beaf6452ea156e78b0235e4675a3` |
| `crates/text-refinement-worker/tests/supervisor.rs` | `eeaa05ba85fceb975c80945f1612c7ffebc8729a906afeb264d4bd1271e8fc5d` |
| `crates/text-refinement-worker/tests/fixtures/process.rs` | `54313ddeb7d0518cddc3aa71a29e87fb503d873617839b69c8fbf85c6e5dad72` |

## Limits

These tests exercise real process and pipe behavior with deterministic fixtures,
not GPU inference or native text delivery. They cover controlled shutdown and
owner drop; abrupt host termination and startup recovery of orphaned directories
are not established by these tests. Application integration, model-quality
acceptance, offline bundle acceptance and performance measurements remain open.
The workflow now includes these checks, but no remote CI result is claimed.
