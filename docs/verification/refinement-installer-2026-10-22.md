# Complete-set text model installer

This report covers the installer module added after `da6a397`. It is not yet
connected to application setup or the dictation runtime. Model-quality failures
reported in the earlier worker diagnostics remain open.

## Implemented boundary

`RefinementModel::bundled` reads the worker's exact embedded manifest. The fixed
revision contains 12 files totaling 2,278,970,666 bytes. Installation uses the
existing bounded HTTP transfer, size/SHA-256 checks, redirect policy and private
verified-file publication. A separate directory lock covers complete-set
inspection, staging, recovery and publication. Cache inspection constructs no
network client; a verified cache hit needs no transport or capacity query.

Only a fully reverified staging directory is renamed into its final location.
Cancellation before publication leaves no new active set. After the rename
commit point the complete cache remains; the future setup owner must separately
prevent activation of a cancelled setup request. A corrupt known set is retained
in a backup. Failed publication attempts restoration; a failed restoration
reports both errors and retains the backup. Unknown entries, symlinks and
hardlinks fail closed. Valid older revision directories are left alone.

Capacity preflight requires two complete sets plus 64 MiB (4,625,050,196 bytes)
for staging and a later private worker snapshot. Native `statfs` uses available
blocks with checked arithmetic. This is a preflight on the cache filesystem,
not a reservation, a guarantee about another volume, or an ENOSPC immunity.

Recovery scans at most 1,024 root entries. It removes only recognized staging
directories owned by a definitely absent PID whose contents are recognized
regular, singly linked files. Live owners, unknown contents, redirected entries
and retained backups survive. This does not implement recovery of worker
snapshots after abrupt host termination. Cache parents are application-owned;
protection against malicious same-user path swaps is not claimed.

## Executed checks and failures

Local macOS checks passed:

- `cargo test -p everlast-dictation --lib --locked --offline model::`:
  49 tests, including 12 new installer tests and existing cache/transport tests.
- `cargo test -p macos-integration --lib --locked --offline storage::`:
  capacity, missing-path and NUL-path checks in one test.
- `cargo clippy -p everlast-dictation -p macos-integration --lib --tests
  --locked --offline -- -D warnings`, Rust format and `npm run check:fast`.

Installer cases use real temporary directories, hashes and publication/rollback
operations. They cover complete-set visibility, partial failure, cancellation,
capacity failure, cache reuse, corruption, unknown entries, redirected files,
dead/live staging owners, publication failure and failed restoration. A bounded
loopback HTTP fixture transfers two files through the production body path;
only that fixture permits plaintext loopback URLs. Production remains HTTPS.

Independent review found that the extracted lock rejected the existing
`silero-v5.1.2` ID. A failing regression using the actual VAD definition
reproduced this; safe dotted names now retain contention and release behavior.
The first combined model test run passed 48 tests and failed the new HTTP
fixture with `WouldBlock`. Accepted sockets inherited nonblocking mode on
Darwin; explicitly restoring bounded blocking I/O corrected the fixture. The
repeat passed all 49 tests. Both corrections received independent read-back.

A header-only probe followed the pinned weight redirect from
`huggingface.co` to the already allowed `us.aws.cdn.hf.co`; the final response
reported the expected 2,263,022,417-byte weight length. No complete multi-GB
download was performed for this installer report. No integrated consent UI,
offline product run, model-loading acceptance, microphone, target receipt,
performance comparison or new bundle acceptance is asserted here. Broad
integration checks remain for a stable integrated candidate.

## Source binding

| File | SHA-256 |
|---|---|
| `src-tauri/src/model/refinement.rs` | `65bbc23610700bba9474fd4cf0c18c475a281de7c31135fc6fed3fc9609a0c08` |
| `src-tauri/src/model/refinement/tests.rs` | `8ebbcba67d4015a04683804b70e9c57e787675b7fde1ace17e4927a26154af78` |
| `src-tauri/src/model/cache.rs` | `2287a54368e6a512b010f1a9bc3a0a32ac41cf82df20bcd840c7ecad50c10fd3` |
| `crates/macos-integration/src/storage.rs` | `8dadad1ac54120b554d407e533073d97b8dcc452d6403c4e2f300c5db2b01e52` |

The separate [provenance receipt](../licenses/third-party/qwen3-model-provenance.json)
binds the preserved original license and model card. License attribution and
asset integrity do not establish linguistic quality or runtime performance.
