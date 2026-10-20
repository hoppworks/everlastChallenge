# Local patches: whisper-rs-sys 0.15.0 — SOT logits and VAD failure status

## Provenance and retained source

Source is the complete locally cached crates.io `whisper-rs-sys` **0.15.0**
archive. Its SHA256 matches the application lockfile checksum:
`6986c0fe081241d391f09b9a071fbcbb59720c3563628c3c829057cf69f2a56f`.
All 786 regular package files were compared byte-for-byte with that archive
before this patch. The copy retains original normalized/source Cargo manifests,
standalone package lockfile, build script, bindings, all packaged whisper.cpp/ggml
sources and original license notices. Only Cargo's `.cargo-ok` extraction-cache
marker is omitted. No global registry file was edited.

The retained `.cargo_vcs_info.json` identifies the wrapper repository commit
`7558e1b72f54f2f22a53589afb77e65681834c36` (`sys/`). Its whisper.cpp submodule
pin is **`2eeeba56e9edd762b4b38467bab96c2517163158`**:
[upstream submodule metadata](https://api.github.com/repos/tazz4843/whisper-rs/contents/sys/whisper.cpp?ref=7558e1b72f54f2f22a53589afb77e65681834c36),
[pinned C++ source](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp).
The source audit and upstream-reference evidence are recorded in
[whisper-no-speech-source.md](../../docs/verification/whisper-no-speech-source.md).

`whisper.cpp/src/whisper.cpp` SHA256:

- Original: `d1e041704c12e20b8eb9596632db3b3b31046348dbb0e493f8a5ab8edd5f4376`.
- SOT-only patch (historical v3 measurement): `f369a8ba8e4180642967e373334781a49a253d59a7b0f4b9a03f85b4040dfecc`.

## Licenses

The sys package declares **Unlicense** in both original Cargo manifests. Its
published package does not include a separate root license-text file. For a
self-contained local copy, `LICENSE-Unlicense` is copied unchanged from the
crates.io `whisper-rs` 0.16.0 package at the **same wrapper Git commit**. That
archive's SHA256 is
`2088172d00f936c348d6a72f488dc2660ab3f507263a195df308a3c2383229f6`;
the retained license-text SHA256 is
`f0510d3b9d1223c45e8fdd2b49270ec8734fff491c7f7d1b948ee70162e1ceff`.
Bundled whisper.cpp retains its **MIT** license in `whisper.cpp/LICENSE`; all
packaged ggml/backend source notices are retained unchanged. This patch does
not replace their license declarations with the wrapper's Unlicense.

## Why this changes inference behavior

`whisper_batch_prep_legacy` requests only the last prompt row. Initial prompts
include SOT before language/task tokens and may include historical context
before SOT. The old no-speech calculation nevertheless reads row zero, which
can be zero-initialized or stale. The softmax helper also computes its maximum
across its entire input vector, making an unbounded multi-row input incorrect.
Selecting the last row would still use language/task-conditioned logits instead
of the SOT row required by the reference algorithm.

After normal legacy preparation, this patch additionally requests the SOT row
at `prompt.size() - prompt_init.size()`, with index/token assertions. The legacy
last-row flag remains enabled. After that initial decode and before any logit
filters, exactly one `n_vocab`-wide SOT row is copied into a local vector. Both
unchanged existing softmax helpers consume only that vector. The text decoder
continues to select its original last prompt row. No C/Rust ABI, bindings,
header, model manifest, temperature, threshold or logit-filter behavior changes.

Reference semantics: OpenAI Whisper commit
`86098128c0b4f24f0e2aa2994de830614b474227`,
[SOT selection](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/whisper/decoding.py#L535-L537),
[raw initial SOT probability before text-row selection](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/whisper/decoding.py#L685-L697).

## Minimal production diff

```diff
--- a/whisper.cpp/src/whisper.cpp
+++ b/whisper.cpp/src/whisper.cpp
@@ -7143,6 +7143,12 @@
 
                 whisper_batch_prep_legacy(state->batch, prompt.data(), prompt.size(), 0, 0);
 
+                // No-speech is conditioned on SOT, before language/task tokens.
+                const int i_sot = (int) (prompt.size() - prompt_init.size());
+                WHISPER_ASSERT(i_sot >= 0 && i_sot < state->batch.n_tokens);
+                WHISPER_ASSERT(prompt[i_sot] == whisper_token_sot(ctx));
+                state->batch.logits[i_sot] = 1;
+
                 if (!whisper_decode_internal(*ctx, *state, state->batch, params.n_threads, false, params.abort_callback, params.abort_callback_user_data)) {
                     WHISPER_LOG_ERROR("%s: failed to decode\n", __func__);
                     return -8;
@@ -7152,11 +7158,16 @@
                 // This has to be done before any logit filtering. Hence we cannot use the probs from the whisper_process_logits.
                 {
                     const int n_logits = ctx->vocab.id_to_token.size();
+                    const size_t sot_offset = size_t(i_sot) * n_logits;
+                    WHISPER_ASSERT(sot_offset + n_logits <= state->logits.size());
+                    const std::vector<float> sot_logits(
+                            state->logits.begin() + sot_offset,
+                            state->logits.begin() + sot_offset + n_logits);
                     std::vector<float> logprobs(n_logits);
                     std::vector<float> probs(n_logits);
 
-                    whisper_compute_logprobs(state->logits, n_logits, logprobs);
-                    whisper_compute_probs(state->logits, n_logits, logprobs, probs);
+                    whisper_compute_logprobs(sot_logits, n_logits, logprobs);
+                    whisper_compute_probs(sot_logits, n_logits, logprobs, probs);
                     state->no_speech_prob = probs[whisper_token_nosp(ctx)];
                 }
 
```

Only `whisper.cpp/src/whisper.cpp` differs from the published source. The two
additional documentation/license files are `PATCHES.md` and `LICENSE-Unlicense`.
The published package version and dependency declarations are unchanged. The
application selects this copy through its root `[patch.crates-io]` entry. The
root lockfile changes only this package from registry to local-path selection;
all package versions and all other dependency edges are preserved.

## Verification scope and remaining risk

Preparation verified package hashes, retained source equality and the bounded
source diff. `cargo metadata --offline --locked --filter-platform
aarch64-apple-darwin` resolves this local 0.15.0 package; no other lockfile package
block changed. Per the bounded task instructions it performed **no compilation,
ASR/model run or model/cache mutation**. Executable Red/Green evidence is therefore
not claimed by this preparation. Cargo selection, builds and regression runs are
owned by the integrating application task.

Before release, exercise differing SOT/last rows, context containing an earlier
SOT, fresh/reused state, and the unchanged 42-case calibration manifest. Recompute
no-speech measurements instead of reusing numbers from the faulty implementation.
Actual language WER, no-speech separation, cancellation and memory/latency effects
remain measurement requirements; a corrected probability is not itself an
acceptance decision or a guarantee against hallucination.

The additional backend row transfer and local float vector are bounded by the
vocabulary size. They can change execution cost, particularly across fallback
iterations and audio windows. The text continuation path retains its last-row
input, but model-level effects of corrected no-speech decisions must be measured.
This local fork must be re-audited before any upstream crate/submodule update.

## Required source-switch build hygiene

At the historical SOT/VAD stage the upstream `build.rs` (around lines 103–116) watched `wrapper.h`
and copied `whisper.cpp` into `OUT_DIR` only when that directory did not already
exist. Editing the vendored C++ source alone can therefore leave a previously
copied implementation in use, even if Cargo subsequently builds an example.
Before **each** temporary original/patched C++ switch in a Red/Green regression,
run `cargo clean --release -p whisper-rs-sys` (or the matching `--profile` / `--target` for that build), then rebuild the requested example/profile.
Verify the SHA-256 of the actual
`target/<profile>/build/whisper-rs-sys-*/out/whisper.cpp/src/whisper.cpp` copy
against the intended source hash recorded above; registry and local-path build
directories may coexist. Do not infer the active C++ implementation from a
successful build or the repository source hash alone.

## Integrated execution evidence (2026-10-17)

The integrating task subsequently exercised original/patched Red/Green on
Reference/Metal and Small/CPU, plus both full 42-case calibration runs.
See [calibration-v3.md](../../docs/verification/calibration-v3.md) for exact
criteria, artifact hashes, commands and limitations. Small/Metal retains two
strict singleton-oracle mismatches out of eight; its unchanged repeat is
identical. No tolerance was relaxed and no P05 qualification is claimed.

## Additional VAD failure-status patch (2026-10-17)

Combined C++ SHA256 at the VAD-only stage: `fc7f40c5d5022f45920164bfe8261de110d98a4fb88fd1ee97ef526ab00e682e`.
The SOT-only hash above remains the immutable v3 measurement source, not the
current combined file. In `whisper_vad_detect_speech`, compute failure now
propagates false after common scheduler cleanup and clears partial/stale
probabilities. Graph-allocation failure also clears probabilities and resets
the scheduler. Successful numerical computation and all ABI/header declarations
are unchanged. The actual scheduler reset tolerates the compute helper having
already reset it.

See [vad-failure-fix.md](../../docs/verification/vad-failure-fix.md) for actual-
function fault-injection RED/GREEN, unchanged 42-case score vectors per variant,
build-source hashes and limits. At that VAD-only stage only the same C++ file differed from the original
published package; original license/source files are still untouched. Neither
this patch nor the developer VAD experiment enables a product gate.

## Isolated MLX encoder bridge (2026-10-19)

The optional additive `whisper_full_with_state_and_encoder` entry consumes a
borrowed finite contiguous F32 [1500,1280] array for the first `seek==0` only,
with bounded original PCM and explicit de/en/no-context/greedy policy. The caller
binds features to the same actual prepared PCM and pinned model; the native API
validates dimensions and options, not a cryptographic feature/audio identity.
Subsequent seeks use the native encoder. The original Full controller, decoder,
filters, sampling, no-speech, timestamps and temperature fallback are shared.
Existing public structs retain their layouts. The default Full entry passes no
features. This does not claim universal numerical equivalence of MLX/CPP encoding.

The bridge uses the existing external-encoder graph/input form. Its borrowed
features remain alive throughout synchronous full processing and are not stored
after the call. Build.rs now explicitly watches and synchronizes the two modified
C++/header inputs into its existing OUT_DIR tree before CMake. Existing historical
source-switch evidence above remains valid for that historical build. Actual
release copies were checked byte-for-byte against both current vendor files.
Pregenerated bindings carry the same new declaration for builds without Bindgen.

Current exact local changes relative to this task's frozen c35fd595 base are
recorded in [EVERLAST-ENCODER-PATCH.json](EVERLAST-ENCODER-PATCH.json), alongside
the preserved earlier SOT/VAD patches. No original license text was changed.
Public native contract: expected one external window first failed at runtime
(0/0 receipt); the implemented path passes two repeated calls and invalid policy
rejection. Evidence is in `artifacts/mlx-qualification/encoder-bridge-contract-v1`.
The initial throwaway compute-skip prototype failed its second-call scheduler
assertion; it is retained only under artifacts and is not this implementation.
Quality/latency/resource qualification is recorded separately and is not implied
by the native contract.


## 2026-10-20: bounded startup and residency shutdown

The macOS application build sets `GGML_METAL_TENSOR=OFF`; other direct CMake
consumers keep the upstream default. On the qualified M5 / SDK27 host the
original 8x8 capability shader fails compilation and already selects the SIMD
path. Avoiding that unsuccessful optional probe preserves this host's effective
CPP computation path. It does not disable the independent MLX GPU encoder.
This is a measured application policy, not a claim that all Metal hardware or
compiler revisions have identical capability results.

The residency collection keeps its existing 500ms heartbeat interval. Its wait
is now a dispatch semaphore with a timeout, signalled on destruction before the
existing dispatch-group join. Destruction still waits for the worker and releases
both synchronization objects afterwards. A native lifecycle fixture exercises
this actual public init/free boundary: `scripts/native-tests/residency-shutdown.m`.
The old library waited 480ms in that fixture; the modified library returned in
about 0.02ms. GPU synchronization and resource ownership are unchanged.

`build.rs` now synchronizes and watches the modified Metal source and its CMake
file as well as the existing bridge files. Otherwise an incremental build could
silently compile stale copies from `OUT_DIR`.
