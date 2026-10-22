# Bundled local text worker

This package follows `25330d7`. The release builder now includes the text worker
beside the application executable. It builds both native targets from the locked
workspace and requires identical Metal kernel bytes before packaging. Runtime
and model-quality acceptance remain separate from this packaging check.

The copied kernel library and worker are signed before the outer application.
The local-identity signer follows the same order; the helper has no microphone
entitlement. Bundle verification checks nonempty regular runtime files, executable
permissions, ARM64 architecture, system-only linked libraries, signatures and the
main application's microphone declarations. Unexpected executable payloads remain
rejected by the local signer.

## Verification

- 17 focused packaging, inventory and signing tests passed, together with the
  fast repository checks. No Rust implementation changed in this package.
- One release bundle was built successfully. The builder compared the app and
  worker kernel artifacts and verified the packaged result.
- A copy made with `ditto --rsrc --extattr` was verified outside the checkout,
  signed with the configured local identity and verified again. Preserving
  extended attributes matters because the Metal library's signature uses them.
- The copied worker ran with its working directory outside the checkout and
  without development library-path variables. It loaded the pinned model,
  returned an empty edit set for “Das ist ein einfacher Test.” and exited with
  code zero on shutdown. Its scratch directory was empty afterward.

Independent review caught that the previous local signer rejected the newly
bundled worker. A failing regression preceded the correction; targeted read-back
closed the finding. Tests retain rejection of extra code, a missing helper,
non-Mach-O helper content and missing executable permissions.

The first relocation probe passed a temporary path containing macOS's `/var`
symlink. Structural validation rejected that path before signing. The probe then
used its canonical path and passed; the original failure remains in local logs.

The [artifact manifest](refinement-package-sources-2026-10-22.json) binds the
packaging sources, pre-change commit ancestry, original bundle file bytes and
smoke-test scope. Local re-signing changes executable bytes; those results are
kept separately from the original bundle inventory.

This check does not prove offline application startup, ASR/refinement coexistence,
model quality, cancellation under load, actual text receipt or comparative
performance. It also does not grant Gatekeeper distribution approval: the bundle
has no Developer ID notarization. No remote CI result or release is implied.
