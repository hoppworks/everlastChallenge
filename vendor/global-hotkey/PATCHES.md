# Local patch: global-hotkey 0.8.0

Source: the crates.io `global-hotkey` 0.8.0 archive, SHA-256
`8c386b0a4a70cb2d39fffd74480f985b6f0bfbcb934b6a6b6b7e630e448f242e`.
Upstream repository: <https://github.com/tauri-apps/global-hotkey>, package VCS commit
`2a620bf3852008b568f6d36c2baedcc3dd0822f2` (retained `.cargo_vcs_info.json`).
Version and dependencies are unchanged. Original normalized/source manifests, runtime
sources, examples, README, changelog and MIT/Apache/SPDX license files are retained.
Registry cache markers, development automation and the upstream standalone lockfile
are not runtime inputs; this application uses its root Cargo.lock.

## Changes

- `src/error.rs`: preserve numeric Carbon OSStatus in a non-exhaustive error variant.
- `src/platform_impl/macos/mod.rs`: preserve InstallEventHandler,
  RegisterEventHotKey and UnregisterEventHotKey status; retain the owned registration
  until native unregistration succeeds.
- `src/platform_impl/macos/ownership.rs`: small map-ownership helper and deterministic
  failed-removal/retry test. A failed native operation leaves the same handle available
  for retry; successful removal releases it exactly once.

The application selects this source with its root `[patch.crates-io]`. No registry
cache source was edited. macOS register/unregister calls still require the native
main thread. The patch does not establish printable-character suppression, Secure
Input behavior, layout acceptance or real OS-error recovery.

## Controlled verification without OS shortcuts

From the application root:

```sh
rustc --edition 2021 --test vendor/global-hotkey/verification/ownership_harness.rs -o /tmp/everlast-hotkey-ownership-tests
/tmp/everlast-hotkey-ownership-tests
cargo test -p everlast-dictation hotkeys::tests --lib --offline --locked
```

The harness imports the production helper directly. It performs no Carbon calls
and registers no global keys. The original remove-before-call ordering fails the
ownership test; get/call/remove passes. Application tests cover bounded dispatch,
repeat suppression, rebind rollback, selective cleanup and temporary Escape.
Real installed-app acceptance remains separate.
