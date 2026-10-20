//! Scoped full-context prefix reuse, selected only by an explicit pipeline policy.
//! The owning decoder fixes the Engine/model/config for the cache's lifetime.
use super::{Engine, InferenceError, Language, decode, speech_gate, transcribe_using};
use crate::{
    inference_service::{DecodeContext, Decoder},
    model::ModelDefinition,
};
use sha2::{Digest, Sha256};
use std::path::Path;

#[cfg(feature = "measurement")]
mod evidence;
#[cfg(not(feature = "measurement"))]
use super::NoObservation as NativeObservation;
#[cfg(feature = "measurement")]
use evidence::NativeObservation;
#[cfg(feature = "measurement")]
pub use evidence::{PreparedReuse, ReuseOutcome};

pub struct PrefixCacheDecoder {
    engine: Engine,
    cache: SessionCache,
}

impl PrefixCacheDecoder {
    pub fn load(path: &Path, definition: ModelDefinition) -> Result<Self, InferenceError> {
        Ok(Self {
            engine: Engine::load(path, definition)?,
            cache: SessionCache::default(),
        })
    }
}

impl Decoder for PrefixCacheDecoder {
    fn decode(&mut self, samples: &[f32], language: Language) -> Result<String, InferenceError> {
        // Unscoped calls always execute normally and cannot seed a session cache.
        self.cache = SessionCache::default();
        self.engine.transcribe(samples, language)
    }

    fn decode_with_context(
        &mut self,
        samples: &[f32],
        language: Language,
        context: &DecodeContext,
    ) -> Result<String, InferenceError> {
        let Engine {
            context: native,
            gate,
        } = &mut self.engine;
        self.cache.transcribe(
            samples,
            language,
            context,
            gate,
            |prepared, language, observer| decode::<false, _>(native, prepared, language, observer),
        )
    }

    #[cfg(feature = "measurement")]
    fn take_prepared_reuse(&mut self) -> Option<PreparedReuse> {
        self.cache.take_receipt()
    }
}

#[derive(Default)]
struct SessionCache {
    entry: Option<Entry>,
    scope: Option<DecodeContext>,
    #[cfg(feature = "measurement")]
    receipt: Option<PreparedReuse>,
}

#[derive(PartialEq, Eq)]
struct PreparedKey {
    frames: usize,
    sha256: [u8; 32],
    language: Language,
}

struct Entry {
    key: PreparedKey,
    text: String,
    #[cfg(feature = "measurement")]
    origin_job_id: u64,
}

fn prepared_key(samples: &[f32], language: Language) -> PreparedKey {
    let mut hash = Sha256::new();
    // Constant 4-KiB staging, never a second full PCM byte vector.
    let mut bytes = [0_u8; 4096];
    for chunk in samples.chunks(1024) {
        for (sample, output) in chunk.iter().zip(bytes.as_chunks_mut::<4>().0) {
            output.copy_from_slice(&sample.to_le_bytes());
        }
        hash.update(&bytes[..chunk.len() * 4]);
    }
    PreparedKey {
        frames: samples.len(),
        sha256: hash.finalize().into(),
        language,
    }
}

fn bounded_text_copy(text: &str) -> Result<String, InferenceError> {
    if text.len() > 256 * 1024 {
        return Err(InferenceError::OutputLimit);
    }
    let mut copy = String::new();
    copy.try_reserve_exact(text.len())
        .map_err(|_| InferenceError::Memory)?;
    copy.push_str(text);
    Ok(copy)
}

impl SessionCache {
    #[cfg(feature = "measurement")]
    fn take_receipt(&mut self) -> Option<PreparedReuse> {
        self.receipt.take()
    }
    fn transcribe<D: speech_gate::Detector>(
        &mut self,
        samples: &[f32],
        language: Language,
        context: &DecodeContext,
        gate: &mut speech_gate::SpeechGate<D>,
        native: impl FnOnce(&[f32], Language, &mut NativeObservation) -> Result<String, InferenceError>,
    ) -> Result<String, InferenceError> {
        #[cfg(feature = "measurement")]
        {
            self.receipt = Some(PreparedReuse {
                session: context.session,
                generation: context.generation,
                job_id: context.job_id,
                outcome: ReuseOutcome::NotAdmitted,
                origin_job_id: None,
                prepared_frames: None,
                prepared_pcm_sha256: None,
                decoder_invoked: false,
                native_full_attempted: false,
            });
        }
        if !context.is_valid()
            || context.session == 0
            || context.generation == 0
            || context.job_id == 0
        {
            self.entry = None;
            #[cfg(feature = "measurement")]
            if let Some(receipt) = &mut self.receipt {
                receipt.outcome = ReuseOutcome::InvalidContext;
            }
            return Err(InferenceError::InvalidDecodeContext);
        }
        if let Some(previous) = &self.scope {
            if previous.same_epoch(context) {
                if context.job_id <= previous.job_id {
                    self.entry = None;
                    #[cfg(feature = "measurement")]
                    if let Some(receipt) = &mut self.receipt {
                        receipt.outcome = ReuseOutcome::InvalidContext;
                    }
                    return Err(InferenceError::InvalidDecodeContext);
                }
            } else {
                self.entry = None;
            }
        }
        self.scope = Some(context.clone());
        let mut reached = false;
        #[cfg(feature = "measurement")]
        let mut observer = NativeObservation::default();
        #[cfg(not(feature = "measurement"))]
        let mut observer = NativeObservation;
        let result = transcribe_using(
            samples,
            language,
            gate,
            &mut observer,
            |prepared, language, observer| {
                reached = true;
                let key = prepared_key(prepared, language);
                #[cfg(feature = "measurement")]
                if let Some(receipt) = &mut self.receipt {
                    receipt.prepared_frames = Some(key.frames);
                    receipt.prepared_pcm_sha256 = Some(key.sha256);
                }
                if let Some(entry) = self.entry.as_ref().filter(|entry| entry.key == key) {
                    #[cfg(feature = "measurement")]
                    if let Some(receipt) = &mut self.receipt {
                        receipt.outcome = ReuseOutcome::Hit;
                        receipt.origin_job_id = Some(entry.origin_job_id);
                    }
                    return bounded_text_copy(&entry.text);
                }
                // A miss has no fallback. Release the old text before native work
                // and before allocating the replacement entry's bounded copy.
                self.entry = None;
                #[cfg(feature = "measurement")]
                if let Some(receipt) = &mut self.receipt {
                    receipt.decoder_invoked = true;
                }
                let text = native(prepared, language, observer)?;
                self.entry = Some(Entry {
                    key,
                    text: bounded_text_copy(&text)?,
                    #[cfg(feature = "measurement")]
                    origin_job_id: context.job_id,
                });
                #[cfg(feature = "measurement")]
                if let Some(receipt) = &mut self.receipt {
                    receipt.outcome = ReuseOutcome::Decoded;
                    receipt.origin_job_id = Some(context.job_id);
                }
                Ok(text)
            },
        );
        #[cfg(feature = "measurement")]
        if let Some(receipt) = &mut self.receipt {
            receipt.native_full_attempted = observer.full_attempted;
            if result.is_err() {
                receipt.outcome = ReuseOutcome::Failed;
                receipt.origin_job_id = None;
            }
            if !context.is_valid() {
                receipt.outcome = ReuseOutcome::Revoked;
                receipt.origin_job_id = None;
            }
        }
        if !reached || result.is_err() || !context.is_valid() {
            self.entry = None;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_service::WorkToken;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Positive {
        probabilities: Vec<f32>,
        calls: Arc<AtomicUsize>,
    }
    impl speech_gate::Detector for Positive {
        fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.probabilities.resize(samples.len().div_ceil(512), 0.9);
            Ok(&self.probabilities)
        }
    }

    fn positive_gate() -> speech_gate::SpeechGate<Positive> {
        speech_gate::SpeechGate::from_detector(Positive {
            probabilities: Vec::new(),
            calls: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn context(job_id: u64, token: &WorkToken) -> DecodeContext {
        DecodeContext {
            session: 7,
            generation: 9,
            job_id,
            token: token.clone(),
        }
    }

    #[test]
    fn generation_token_owner_and_language_are_not_interchangeable() -> Result<(), InferenceError> {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        let other = WorkToken::default();
        let requests = [
            (context(1, &token), Language::German),
            (context(2, &token), Language::English),
            (
                DecodeContext {
                    generation: 10,
                    ..context(1, &token)
                },
                Language::English,
            ),
            (
                DecodeContext {
                    generation: 10,
                    ..context(2, &other)
                },
                Language::English,
            ),
        ];
        let mut calls = 0;
        for (expected, (request, language)) in requests.into_iter().enumerate() {
            let text =
                cache.transcribe(&[0.125; 3200], language, &request, &mut gate, |_, _, _| {
                    calls += 1;
                    Ok(format!("fresh {calls}"))
                })?;
            assert_eq!(text, format!("fresh {}", expected + 1));
        }
        assert_eq!(calls, 4);
        Ok(())
    }

    #[test]
    fn duplicate_job_is_rejected_and_cannot_leave_a_reusable_result() -> Result<(), InferenceError>
    {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(11, &token),
            &mut gate,
            |_, _, _| Ok("first".into()),
        )?;
        let duplicate = cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(11, &token),
            &mut gate,
            |_, _, _| Err(InferenceError::Decode),
        );
        assert!(matches!(
            duplicate,
            Err(InferenceError::InvalidDecodeContext)
        ));
        let fresh = cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(12, &token),
            &mut gate,
            |_, _, _| Ok("fresh after rejected duplicate".into()),
        )?;
        assert_eq!(fresh, "fresh after rejected duplicate");
        Ok(())
    }

    #[test]
    fn native_error_and_invalid_audio_never_fall_back_to_cached_text() -> Result<(), InferenceError>
    {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(1, &token),
            &mut gate,
            |_, _, _| Ok("old".into()),
        )?;
        let failed = cache.transcribe(
            &[0.25; 3200],
            Language::German,
            &context(2, &token),
            &mut gate,
            |_, _, _| Err(InferenceError::Decode),
        );
        assert!(matches!(failed, Err(InferenceError::Decode)));
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(3, &token),
                &mut gate,
                |_, _, _| Ok("fresh after decode error".into())
            )?,
            "fresh after decode error"
        );
        let invalid = cache.transcribe(
            &[f32::NAN],
            Language::German,
            &context(4, &token),
            &mut gate,
            |_, _, _| Err(InferenceError::Decode),
        );
        assert!(matches!(invalid, Err(InferenceError::InvalidAudio)));
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(5, &token),
                &mut gate,
                |_, _, _| Ok("fresh after invalid audio".into())
            )?,
            "fresh after invalid audio"
        );
        Ok(())
    }

    #[test]
    fn different_raw_lengths_can_reuse_only_the_same_final_padded_pcm() -> Result<(), InferenceError>
    {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(1, &token),
            &mut gate,
            |_, _, _| Ok("same native input".into()),
        )?;
        let mut with_tail = vec![0.125; 3200];
        with_tail.resize(3210, 0.0);
        let hit = cache.transcribe(
            &with_tail,
            Language::German,
            &context(2, &token),
            &mut gate,
            |_, _, _| Err(InferenceError::Decode),
        )?;
        assert_eq!(hit, "same native input");
        with_tail[3200] = -0.0;
        let miss = cache.transcribe(
            &with_tail,
            Language::German,
            &context(3, &token),
            &mut gate,
            |_, _, _| Ok("different exact f32 bits".into()),
        )?;
        assert_eq!(miss, "different exact f32 bits");
        Ok(())
    }

    #[test]
    fn cancellation_during_native_work_cannot_seed_a_later_cache() -> Result<(), InferenceError> {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        let old = cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(1, &token),
            &mut gate,
            |_, _, _| {
                token.cancel();
                Ok("completed after revocation".into())
            },
        )?;
        // The real service discards this result at its existing post-call gate.
        assert_eq!(old, "completed after revocation");
        let revoked = cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(2, &token),
            &mut gate,
            |_, _, _| Err(InferenceError::Decode),
        );
        assert!(matches!(revoked, Err(InferenceError::InvalidDecodeContext)));
        let next = WorkToken::default();
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(3, &next),
                &mut gate,
                |_, _, _| Ok("new actual work".into())
            )?,
            "new actual work"
        );
        Ok(())
    }

    #[test]
    fn current_vad_rejection_and_failure_invalidate_an_old_positive_result()
    -> Result<(), InferenceError> {
        struct Switchable {
            mode: Arc<AtomicUsize>,
            scores: Vec<f32>,
        }
        impl speech_gate::Detector for Switchable {
            fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
                let mode = self.mode.load(Ordering::Relaxed);
                if mode == 2 {
                    return Err(InferenceError::Vad);
                }
                self.scores.resize(samples.len().div_ceil(512), 0.0);
                self.scores.fill(if mode == 1 { 0.0 } else { 0.9 });
                Ok(&self.scores)
            }
        }
        let mode = Arc::new(AtomicUsize::new(0));
        let mut gate = speech_gate::SpeechGate::from_detector(Switchable {
            mode: Arc::clone(&mode),
            scores: Vec::new(),
        });
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(1, &token),
            &mut gate,
            |_, _, _| Ok("old positive".into()),
        )?;
        mode.store(1, Ordering::Relaxed);
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(2, &token),
                &mut gate,
                |_, _, _| Err(InferenceError::Decode)
            )?,
            ""
        );
        mode.store(0, Ordering::Relaxed);
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(3, &token),
                &mut gate,
                |_, _, _| Ok("fresh after negative VAD".into())
            )?,
            "fresh after negative VAD"
        );
        mode.store(2, Ordering::Relaxed);
        assert!(matches!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(4, &token),
                &mut gate,
                |_, _, _| Err(InferenceError::Decode)
            ),
            Err(InferenceError::Vad)
        ));
        mode.store(0, Ordering::Relaxed);
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(5, &token),
                &mut gate,
                |_, _, _| Ok("fresh after failed VAD".into())
            )?,
            "fresh after failed VAD"
        );
        Ok(())
    }

    #[test]
    fn cached_text_is_bounded_and_oversize_results_cannot_restore_an_old_entry()
    -> Result<(), InferenceError> {
        let mut gate = positive_gate();
        let mut cache = SessionCache::default();
        let token = WorkToken::default();
        let maximum = cache.transcribe(
            &[0.125; 3200],
            Language::German,
            &context(1, &token),
            &mut gate,
            |_, _, _| Ok("x".repeat(256 * 1024)),
        )?;
        assert_eq!(maximum.len(), 256 * 1024);
        let oversize = cache.transcribe(
            &[0.25; 3200],
            Language::German,
            &context(2, &token),
            &mut gate,
            |_, _, _| Ok("x".repeat(256 * 1024 + 1)),
        );
        assert!(matches!(oversize, Err(InferenceError::OutputLimit)));
        assert_eq!(
            cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context(3, &token),
                &mut gate,
                |_, _, _| Ok("fresh after oversize".into())
            )?,
            "fresh after oversize"
        );
        Ok(())
    }

    #[test]
    fn identical_fixture_cannot_reuse_a_previous_session_result() -> Result<(), InferenceError> {
        let mut gate = speech_gate::SpeechGate::from_detector(Positive {
            probabilities: Vec::new(),
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let mut cache = SessionCache::default();
        let mut native_calls = 0;
        let mut texts = Vec::new();
        for session in [7, 8] {
            let context = DecodeContext {
                session,
                generation: 9,
                job_id: session,
                token: WorkToken::default(),
            };
            texts.push(cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context,
                &mut gate,
                |_, _, _| {
                    native_calls += 1;
                    Ok(format!("session {session}"))
                },
            )?);
        }
        assert_eq!(texts, ["session 7", "session 8"]);
        assert_eq!(native_calls, 2);
        Ok(())
    }

    #[test]
    fn identical_prepared_pcm_in_one_session_reuses_only_after_current_vad()
    -> Result<(), InferenceError> {
        let vad_calls = Arc::new(AtomicUsize::new(0));
        let mut gate = speech_gate::SpeechGate::from_detector(Positive {
            probabilities: Vec::new(),
            calls: Arc::clone(&vad_calls),
        });
        let mut cache = SessionCache::default();
        let mut native_calls = 0;
        let token = WorkToken::default();
        let mut texts = Vec::new();
        for job_id in [11, 12] {
            let context = DecodeContext {
                session: 7,
                generation: 9,
                job_id,
                token: token.clone(),
            };
            texts.push(cache.transcribe(
                &[0.125; 3200],
                Language::German,
                &context,
                &mut gate,
                |prepared, language, _observer| {
                    native_calls += 1;
                    assert_eq!(language, Language::German);
                    assert_eq!(&prepared[..3200], &[0.125; 3200]);
                    assert_eq!(prepared.len(), 16000);
                    assert!(prepared[3200..].iter().all(|sample| *sample == 0.0));
                    Ok(format!("native result {native_calls}"))
                },
            )?);
        }
        assert_eq!(texts, ["native result 1", "native result 1"]);
        assert_eq!(native_calls, 1);
        assert_eq!(vad_calls.load(Ordering::Relaxed), 2);
        #[cfg(feature = "measurement")]
        {
            let receipt = cache.take_receipt().ok_or(InferenceError::Observation)?;
            assert_eq!(
                (receipt.session, receipt.generation, receipt.job_id),
                (7, 9, 12)
            );
            assert_eq!(receipt.outcome, ReuseOutcome::Hit);
            assert_eq!(receipt.origin_job_id, Some(11));
            assert_eq!(receipt.prepared_frames, Some(16000));
            assert!(receipt.prepared_pcm_sha256.is_some());
            assert!(!receipt.decoder_invoked);
            assert!(!receipt.native_full_attempted);
            assert!(
                cache.take_receipt().is_none(),
                "receipt must be consumed exactly once"
            );
        }
        Ok(())
    }
}
