# Inference-only mlxrs 0.2.0

## Request-owned text grammar processors

Additive generation entry points accept owned logits processors after configured
penalties and before sampling. They reuse the existing token/stream machinery;
ordinary callers pass an empty list. ModelContext creates a fresh KV cache per
request. This connects the upstream grammar processor to the application's text
worker without changing model arithmetic or the default Whisper/ASR path.
Stateful grammar objects must be constructed per request. Errors retain the
existing fused iterator behavior. Application tests exercise forced token
selection, processor failure and exact-length JSON lists through a text stream.

Imported from crates.io; upstream git revision and archive checksum recorded in EVERLAST-PATCH.json. Retains upstream MIT/Apache licenses. Normalized Cargo.toml removes unused optional CPAL0.18 dependency and its audio activation. Playback module omits hardware player/reexport. PlaybackConfig omits CPAL conversion. Tokenizer, sampling and GPU kernels are unchanged; text-only token exhaustion now fails explicitly as described below. The Whisper-only frontend correction and optional native encoder checkpoint table are described below. Cargo.toml.orig retains historical upstream source.

Existing capture uses CPAL0.15.3. Cargo cannot resolve its native ALSA links together with playback CPAL0.18, including for macOS. Upgrading capture solely to add inference would expand regression scope. This local inference patch has no hardware playback API and needs maintenance until upstream makes playback optional.

Other files preserve released archive bytes. This is an application patch, not a claim of upstream-supported inference-only configuration. whisper.cpp remains for Silero. Native C++/Metal and added tokenizer/audio dependencies still require license, advisory, build, lifecycle and packaging validation.

## Whisper periodic Hann correction

The released Whisper frontend calls the generic symmetric DSP STFT (denominator399 for400points). OpenAI Whisper, Apple's MLX example at commit796f5b53cab69a3d48a44233ce21aae889e94a08 and mlx-audio at commit1aa1bfb71cdf5b5be2636f8866a32b0944a1a868 use a periodic Hann (denominator400). A crate-private periodic STFT shares framing/validation/FFT with the existing implementation and returns raw FFT data, so it cannot enter the public symmetric Spectrum/ISTFT contract. The generic public APIs remain symmetric. The existing Whisper composition test is updated and explicitly labelled internal consistency, not independent numerical authority.

An independent CPU direct-DFT golden uses exact pinned OpenAI mel filters, periodic window, reflection, final-frame removal and log normalization. Tone/broadband, boundary-impulse/padding and silence cases cover80/128bins. The original helper failed with max absolute error0.07384147; corrected helper passes tolerance0.0002. A generic symmetric window/roundtrip guard also passes. Fixture provenance and generator are in tests/fixtures/mlx and scripts/generate-whisper-mel-oracle.py. Actual transcription quality must be rerun; this numerical fix alone does not qualify WER, latency or product release.

## Optional encoder checkpoint positions

For native checkpoints only, the model builder now consumes an explicitly supplied encoder.positional_embedding after the same config-derived shape check and dtype cast as other tensors. When absent, the existing computed sinusoidal table is retained; the published FP16 model has no such key. HF sanitization retains its existing behavior. This supports an exact-coefficient Q5-to-F32 control rather than silently replacing a supplied reference table. Public WhisperModel::from_weights/AutoregressiveStt::encode is checked using a zero-weight model and independently worked LayerNorm output: the old loader ignored the [0,1,2,3] table and failed, the new loader must use it. The existing constructor/table contract remains intact for other callers.

## Text-only completion guard

Text-only generation without EOS cannot advance the audio cursor over a whole window and claim that window is fully transcribed. The selected single/best-of result's token tail is checked before materialization. Missing EOS returns typed CapExceeded with context Whisper text-only completion. Existing native timestamp mode is unchanged. The adapter catches only this specific incomplete-text error, synchronizes abandoned GPU work, then decodes the original entire mel from the beginning using native timestamps/fresh caches; other errors propagate. Public DecodingTask.run fixtures prove cap failure versus real EOS success. The complete-decoder module contract separately proves that a looping no-timestamp fixture is replaced by its successful native full-input retry. No partial first-pass text is inserted.

## Owned reusable-buffer cache policy

The additive memory API sets a process-wide reusable-buffer budget through the existing C API. A model-lifetime guard holds exclusive ownership, rejects competing safe setters, clears unused buffers and restores the previous policy on explicit finish or unwinding. This is a soft pool budget, not an active-memory limit; one returned buffer may exceed it. Explicit finish reports native failures; Drop uses only teardown-safe raw cleanup. Counter documentation now distinguishes active allocation from RSS and documents the native peak reset to zero. Model tensors, arithmetic, decoding and kernels are unchanged.

## Literal control-token spellings in constrained text

The private HuggingFace tokenizer inside the grammar adapter now encodes special
token spellings as ordinary bytes. Previously, forced literal bytes such as
`<|im_start|>` could be encoded as an added special token, which the trie
represents with a special marker instead of those literal bytes. The next
`consume_token` then failed despite the preceding mask allowing it. The model
prompt tokenizer is unchanged; the trie retains its explicit special/EOS tokens.
A controlled-logit text-stream regression reproduces the failure and verifies
complete, byte-exact JSON output after the correction.
