# Prepared-prefix cache developer diagnostic

Prospective measurement-only probe. Production Engine loading and application
starts do not select this decoder. No cache, quality, latency or resource
qualification follows from building this program.

## Invocation and input contract

`prefix_cache_probe MODEL MANIFEST NEW_OUTPUT` selects the pinned reference
model, with the pinned VAD sibling. The manifest uses the existing bounded
corpus schema and contains exactly two or four cases in consecutive
prefix/whole pairs. Each pair has one language, nonempty PCM16 mono16k input,
and a prefix no longer than the whole file. Every prefix f32 sample must match
the corresponding whole sample bit for bit after the common WAV reader; no
normalization, splice, resampling, transcription or inferred partial reference
is introduced. Extra manifest reference fields are not used as a semantic
oracle; the prefix relation is an independently checked audio property.

Each pair owns a fresh PrefixCacheDecoder and serial InferenceService. It
submits prefix then whole with the same session/generation/WorkToken, then the
identical whole with a fresh session and fresh token. Job IDs increase globally.
The middle job may hit or miss: the probe does not force an expected cache hit.
A fresh-session whole job must report a true native decode, not a reused result.

The model is loaded explicitly on the developer process's main thread, then
moved into InferenceService's actual worker. This differs from the asynchronous
production initializer and cannot establish cold-start latency equivalence.
No pacing, microphone, native text delivery or realtime-L claim is made.

## Evidence contract

One private atomically published no-replace JSON contains all planned three/six
rows, errors/unavailable rows included. It exports only text SHA256/UTF8 byte
length, never raw text or PCM. Actual current-job/session/generation and
original cached-job identity come from worker receipts, not assumed hits.
Worker evidence includes the real serial-worker thread tag and raw input PCM
fingerprint; prepared frame/hash evidence comes from the actual decoder.
Cleanup join is observed before successful final publication. Source hashes,
model/VAD/binary/manifest/input bindings and declared fixed parameters are
recorded, including the actual `ObservationSink` implementation in
`transcription/observation.rs` and deployment-target build configuration `.cargo/config.toml`; a separate prospective run freeze and independent review are required
before Root may run actual fixtures. No measurement is authorized here.

## Model-free verification

Actual TDD commands used the measurement-only Example target:

`cargo test -p everlast-dictation --example prefix_cache_probe --features measurement --locked`

- CLI stub: exit101, one test failed with "not implemented"; then one passed.
- Receipt/prefix relation stubs: exit101, two failed; then four passed.
- Actual serial-worker seam stub: exit101, failed cleanup/row assertion; then
  ten passed including inherited input tests.
- Initial integrated Example:13 passed (0.03s).
- Independent review found a final-receipt evidence error: skipping a later
  Completed with no receipt incorrectly returned an older prefix receipt.
  The controlled missing-receipt test failed at `final_receipt.is_none()`
  (exit101), then passed after selecting the last actual Completed before
  taking its optional receipt. No older-receipt fallback remains.
- Final integrated Example: **14 passed**, zero failed/ignored (0.02s).
- `cargo clippy -p everlast-dictation --example prefix_cache_probe --features measurement --locked -- -D warnings`:
  exit0. Owned Rust sources formatted with rustfmt; no foreign source formatted
  or edited by this task.

Controlled tests use the real InferenceService worker with a synthetic Decoder
and receipt source to verify current tags, off-caller thread, bit fingerprint,
three-row retention, failure propagation, no text serialization and actual join.
They are not native Whisper/cache-hit or GPU-kernel evidence. Hash/format/path
bounds and private no-overwrite output are independently exercised. A report is
limited to1MiB; the diagnostic may preload at most four bounded input vectors
(76,800,000 PCM payload bytes), which is not a production buffer-budget claim.

A120s result/readiness budget and a separate120s cleanup observation budget
never claim a forced native interrupt. Unverified cleanup is a failure and
prevents later model loads. The exported final receipt is the last actual
Completed receipt retained through owner join, not a new cache inspection after
shutdown. Decoder elapsed time includes the diagnostic worker wrapper and is
not pure native/cache cost or Runtime stop-to-text latency.

No model job, application, permission, scheduling-policy action, bundle/release
build or actual diagnostic run has been performed by this task. Root must bind
the then-current source revision, binary, fixed parameters, pinned model/VAD,
manifest and inputs in a separate prospective freeze before any native run.
