# Text-model provisioning and preference migration

This package follows `df3b0b7`. The application owns a separate text-model setup
service and prepares the lazy text worker. It does not yet expose setup or the
refinement preference through the UI. Existing desktop sessions still request
refinement disabled. The release bundle has not yet been updated or qualified
for this helper.

## Contracts

Settings schema 2 adds a boolean refinement preference. Version 1 files migrate
with refinement disabled; existing microphone, language and shortcut values
survive. The existing qualified-engine migration remains in effect. Startup
persists and reads back a migrated file. Reading alone does not rewrite it.
Current-schema missing or malformed flags, unknown fields and duplicate fields
are rejected. Storage and rollback use the same typed decoder. A failed save
restores the original bytes even when they used the legacy schema.

The separate setup owner calls the existing complete-set installer. Startup only
inspects local assets. Explicit download requests may install or repair known
incomplete/hash-invalid assets; arbitrary cache entries are not removed.
Progress is bounded by the manifest total of 2,278,970,666 bytes. Cancellation
revokes pending work; another operation cannot start until the previous I/O
owner has finished. Shutdown joins it. Ready describes verified model files,
not GPU loading or successful inference.

The application keeps separate request epochs for ASR and text setup. A queued
download cancelled before dispatch cannot start later. It constructs the lazy
text owner using the fixed executable beside the app executable, the app-owned
model cache and a private runtime directory. No renderer paths or custom backend
addresses participate. Prompt and policy versions now come from shared protocol
constants; the model fingerprint uses the same manifest bytes as the worker.

An invalid optional runtime directory must not prevent ordinary ASR startup.
Independent review identified this regression in the first implementation.
The corrected ASR-ready startup path returns a normal runtime and a visible
text-refinement error when optional preparation fails. It preserves foreign
files and symlink targets.

## Evidence and limits

Focused checks passed: 19 settings/startup tests with default features, 15 with
default features disabled, 13 preference tests, 6 application tests and 4 text
setup tests. The optional-directory and legacy-rollback regressions failed before
their respective corrections. Earlier fixture compile errors were corrected
and are not counted as behavioral evidence.

The 9 protocol tests, host/worker Clippy, Rust formatting, fast repository checks,
generated-contract comparison and source secret scan also passed.

The setup tests cover local-only inspection, cancellation with I/O still held,
single-owner retry, bounded progress, and joining before shutdown returns. The
application tests use a real inference service with a controlled decoder;
they do not open a microphone or load an ASR model. The existing installer
tests remain the evidence for download/hash/publication behavior.

This is not a full download, GPU coexistence, UI, offline bundle or target-receipt
test. Model-quality failures recorded in earlier reports remain open. No new
performance measurements were made.

## Source binding

| File | SHA-256 |
|---|---|
| `crates/text-refinement-protocol/src/lib.rs` | `34bbd52cf028b5f96cab5dfe48aa9758d39ed8a098ffeb7dfb418e444f858432` |
| `crates/text-refinement-worker/src/model.rs` | `6e0009529f5763db35cde819d6c08b76ef0a2ab93c00f793d495f4d00c1c7392` |
| `src-tauri/src/application.rs` | `5e066ac716969d51d5ab73fef35764c6aa0c50bdd928e2ab2deaed0a5b175bc3` |
| `src-tauri/src/refinement_setup.rs` | `fe24e3f0a2baace4ae3fc9582677fdc85c9bd0279e524d2418c4fac42fdcea63` |
| `src-tauri/src/model/refinement.rs` | `9b7db1fffc94b874aaf2f428664d59945a169082ff0f035237d56c5704dc5b44` |
| `src-tauri/src/settings.rs` | `a40c6a62cde62c45779b59cd213c692acadaabd729741410afc9a8be4f4ee4ba` |
| `src-tauri/src/startup_settings.rs` | `7928efab86dc2097471ea4ef723e0eca06fa0f9b29ef31a4e0f1eb67eae870f9` |
| `src-tauri/src/preferences.rs` | `7621d518c233ac82bbb8905e7bbebe53580f51a6d9b638788e66f6f622cf5135` |
| `src-tauri/src/setup.rs` | `7ee6b98372c75a78f9912566d1ccb9226bb6d245d6d29d56dc6cd62e2dba5c9f` |
| `src-tauri/src/desktop.rs` | `2bd7922b6d02f242b417f8c254b4508c103cc20811a8af76ac1690156e5efd9d` |
| `src-tauri/src/lib.rs` | `30361110f0406ceee453f336746fd4e710e8059bf555fbd5920a4de93852eceb` |
