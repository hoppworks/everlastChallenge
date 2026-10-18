//! One explicit experiment decoder and one bounded, job-bound observation receipt.
use everlast_dictation::{
    inference_service::{Decoder, InferenceService},
    model::ModelDefinition,
    transcription::{
        Engine, InferenceError, Language,
        reduced_context::{self, ObservationRecord, Transcription},
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Variant {
    Reference,
    Reduced,
    #[allow(
        dead_code,
        reason = "only corpus experiments select this shared-wrapper variant"
    )]
    FlashReference,
    #[allow(
        dead_code,
        reason = "only corpus experiments select this shared-wrapper variant"
    )]
    FlashCandidate,
}
impl Variant {
    pub fn flash_attention(self) -> Option<bool> {
        match self {
            Self::FlashReference => Some(false),
            Self::FlashCandidate => Some(true),
            Self::Reference | Self::Reduced => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    sequence: u64,
    session: u64,
    generation: u64,
    job_id: u64,
    start_frame: usize,
    end_frame: usize,
    language: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    identity: Identity,
    variant: Variant,
    selected_flash_attention: Option<bool>,
    // Absent for energy-empty/VAD-rejected/pre-decode failures: selected context
    // policy alone is not evidence that native attention executed.
    decode_attempted_with_flash_attention: Option<bool>,
    observation: Value,
    decoder_error: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub receipt: Option<Receipt>,
    pub error: Option<String>,
    pub uncompleted_request: Option<Value>,
}
#[derive(Default)]
struct Slot {
    next: u64,
    last_session: u64,
    armed: Option<Identity>,
    inflight: Option<Identity>,
    receipt: Option<Receipt>,
    error: Option<String>,
}
#[derive(Clone, Default)]
pub struct Receipts(Arc<Mutex<Slot>>);
impl Receipts {
    pub fn arm(
        &self,
        session: u64,
        generation: u64,
        job_id: u64,
        frames: usize,
        language: Language,
    ) -> Result<(), String> {
        let mut slot = self.0.lock().map_err(|_| "context receipt lock poisoned")?;
        if slot.error.is_some()
            || slot.armed.is_some()
            || slot.inflight.is_some()
            || slot.receipt.is_some()
        {
            let error = slot
                .error
                .get_or_insert("context receipt admission occupied or invalid".into())
                .clone();
            return Err(error);
        }
        if session <= slot.last_session
            || generation != 1
            || job_id != session
            || frames == 0
            || frames > 4_800_000
        {
            slot.error = Some("invalid context admission identity".into());
            return Err("invalid context admission identity".into());
        }
        slot.next = slot
            .next
            .checked_add(1)
            .ok_or("context sequence overflow")?;
        slot.last_session = session;
        slot.armed = Some(Identity {
            sequence: slot.next,
            session,
            generation,
            job_id,
            start_frame: 0,
            end_frame: frames,
            language: language.code(),
        });
        Ok(())
    }
    fn begin(&self, frames: usize, language: Language) -> Result<Identity, InferenceError> {
        let mut s = self.0.lock().map_err(|_| InferenceError::Decode)?;
        if s.error.is_some() || s.inflight.is_some() || s.receipt.is_some() {
            s.error
                .get_or_insert("context decoder invoked with occupied/invalid slot".into());
            return Err(InferenceError::Decode);
        }
        let Some(id) = s.armed.as_ref().cloned() else {
            s.error = Some("extra/unarmed context decoder invocation".into());
            return Err(InferenceError::Decode);
        };
        if id.end_frame != frames || id.language != language.code() {
            s.error = Some(format!(
                "context decoder input mismatch: expected frames={} language={}; actual frames={} language={}",
                id.end_frame,
                id.language,
                frames,
                language.code()
            ));
            return Err(InferenceError::Decode);
        }
        s.armed = None;
        s.inflight = Some(id.clone());
        Ok(id)
    }
    fn finish(
        &self,
        id: Identity,
        variant: Variant,
        output: &Transcription,
    ) -> Result<(), InferenceError> {
        let mut s = self.0.lock().map_err(|_| InferenceError::Decode)?;
        if s.inflight.as_ref() != Some(&id) || s.receipt.is_some() {
            s.error
                .get_or_insert("duplicate/foreign context receipt".into());
            return Err(InferenceError::Decode);
        }
        s.inflight = None;
        let o = &output.observation;
        let chosen = if variant == Variant::Reduced
            && o.prepared_frames <= reduced_context::MAX_PREPARED_FRAMES
        {
            1024
        } else {
            0
        };
        let ready = o.preparation == reduced_context::Preparation::Ready;
        let valid = o.input_frames == id.end_frame
            && o.prepared_frames <= id.end_frame.max(16000)
            && if ready {
                o.prepared_frames >= 16000
                    && o.audio_ctx_parameter == Some(chosen)
                    && o.effective_audio_ctx == Some(if chosen == 0 { 1500 } else { chosen })
                    && (output.result.is_err() || o.decode_attempted)
            } else {
                o.prepared_frames == 0
                    && o.audio_ctx_parameter.is_none()
                    && o.effective_audio_ctx.is_none()
                    && !o.decode_attempted
                    && if o.preparation == reduced_context::Preparation::NotPrepared {
                        output.result.is_err()
                    } else {
                        output.result.as_ref().is_ok_and(String::is_empty)
                    }
            };
        if !valid {
            s.error
                .get_or_insert("invalid context observation metadata".into());
        }
        s.receipt = Some(Receipt {
            identity: id,
            variant,
            selected_flash_attention: variant.flash_attention(),
            decode_attempted_with_flash_attention: variant
                .flash_attention()
                .filter(|_| o.decode_attempted),
            observation: record(o),
            decoder_error: output.result.as_ref().err().map(ToString::to_string),
        });
        if valid {
            Ok(())
        } else {
            Err(InferenceError::Decode)
        }
    }
    /// Called after joined cleanup: retain even a result arriving after observer timeout.
    pub fn finalize(&self) -> Collection {
        let Ok(mut s) = self.0.lock() else {
            return Collection {
                receipt: None,
                error: Some("context receipt lock poisoned".into()),
                uncompleted_request: None,
            };
        };
        if s.receipt.is_some() || s.inflight.is_some() || s.armed.is_some() {
            s.error
                .get_or_insert("unconsumed context evidence after runtime cleanup".into());
        }
        Collection {
            receipt: s.receipt.take(),
            error: s.error.clone(),
            uncompleted_request: s.inflight.as_ref().or(s.armed.as_ref()).map(|id| json!(id)),
        }
    }
    pub fn collect(&self, session: Option<u64>, frames: usize, language: Language) -> Collection {
        let Ok(mut s) = self.0.lock() else {
            return Collection {
                receipt: None,
                error: Some("context receipt lock poisoned".into()),
                uncompleted_request: None,
            };
        };
        let receipt = s.receipt.take();
        if let Some(r) = &receipt {
            if r.identity.sequence != s.next
                || Some(r.identity.session) != session
                || r.identity.generation != 1
                || r.identity.job_id != r.identity.session
                || r.identity.end_frame != frames
                || r.identity.language != language.code()
            {
                s.error
                    .get_or_insert("context receipt identity mismatch".into());
            }
        } else if session.is_some() {
            s.error.get_or_insert("missing context receipt".into());
        }
        if s.armed.is_some() || s.inflight.is_some() {
            s.error
                .get_or_insert("context decoder still armed/inflight at collection".into());
        }
        Collection {
            receipt,
            error: s.error.clone(),
            uncompleted_request: s.inflight.as_ref().or(s.armed.as_ref()).map(|id| json!(id)),
        }
    }
}
fn record(o: &ObservationRecord) -> Value {
    json!({"inputFrames":o.input_frames,"preparedFrames":o.prepared_frames,"preparation":format!("{:?}",o.preparation),"audioCtxParameter":o.audio_ctx_parameter,"effectiveAudioCtx":o.effective_audio_ctx,"decodeAttempted":o.decode_attempted})
}
trait Backend: Send + 'static {
    fn transcribe(&mut self, a: &[f32], l: Language, v: Variant) -> Transcription;
}
impl Backend for Engine {
    fn transcribe(&mut self, a: &[f32], l: Language, v: Variant) -> Transcription {
        match v {
            Variant::Reference => self.transcribe_reference_context(a, l),
            Variant::Reduced => self.transcribe_reduced_context(a, l),
            Variant::FlashReference | Variant::FlashCandidate => {
                self.transcribe_reference_context(a, l)
            }
        }
    }
}
struct Adapter<B> {
    backend: B,
    variant: Variant,
    receipts: Receipts,
}
impl<B: Backend> Decoder for Adapter<B> {
    fn decode(&mut self, a: &[f32], l: Language) -> Result<String, InferenceError> {
        let id = self.receipts.begin(a.len(), l)?;
        let output = self.backend.transcribe(a, l, self.variant);
        self.receipts.finish(id, self.variant, &output)?;
        output.result
    }
}
pub fn load(
    path: PathBuf,
    definition: ModelDefinition,
    variant: Option<Variant>,
) -> Result<(InferenceService, Option<Receipts>), Box<dyn std::error::Error>> {
    let Some(variant) = variant else {
        return Ok((InferenceService::load(path, definition)?, None));
    };
    let backend = match variant.flash_attention() {
        Some(enabled) => Engine::load_flash_attention(&path, definition, enabled)?,
        None => Engine::load(&path, definition)?,
    };
    let receipts = Receipts::default();
    let service = InferenceService::with_decoder(Adapter {
        backend,
        variant,
        receipts: receipts.clone(),
    })?;
    Ok((service, Some(receipts)))
}
#[cfg(test)]
pub(super) fn controlled_service(
    variant: Variant,
) -> Result<(InferenceService, Receipts), Box<dyn std::error::Error>> {
    struct Fixed;
    impl Backend for Fixed {
        fn transcribe(&mut self, a: &[f32], _: Language, v: Variant) -> Transcription {
            let ctx = if v == Variant::Reduced { 1024 } else { 0 };
            Transcription {
                result: Ok("controlled words".into()),
                observation: ObservationRecord {
                    input_frames: a.len(),
                    prepared_frames: 16000,
                    preparation: reduced_context::Preparation::Ready,
                    audio_ctx_parameter: Some(ctx),
                    effective_audio_ctx: Some(if ctx == 0 { 1500 } else { ctx }),
                    decode_attempted: true,
                },
            }
        }
    }
    let receipts = Receipts::default();
    let worker = InferenceService::with_decoder(Adapter {
        backend: Fixed,
        variant,
        receipts: receipts.clone(),
    })?;
    Ok((worker, receipts))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_receipts_preserve_identity_and_distinguish_selection_from_decode_attempt()
    -> Result<(), Box<dyn std::error::Error>> {
        for variant in [Variant::FlashReference, Variant::FlashCandidate] {
            let (worker, receipts) = controlled_service(variant)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !worker.ready() {
                if std::time::Instant::now() >= deadline {
                    return Err("controlled worker not ready".into());
                }
                std::thread::yield_now();
            }
            receipts.arm(1, 1, 1, 320, Language::English)?;
            worker.submit(everlast_dictation::inference_service::Job {
                session: 1,
                generation: 1,
                job_id: 1,
                sample_range: 0..320,
                timeline_range: 0..320,
                audio: Arc::new(vec![0.1; 320]),
                language: Language::English,
                token: everlast_dictation::inference_service::WorkToken::default(),
            })?;
            let completed = loop {
                if let Some(done) = worker.take_result()? {
                    break done;
                }
                if std::time::Instant::now() >= deadline {
                    return Err("controlled result missing".into());
                }
                std::thread::yield_now();
            };
            assert_eq!(completed.timeline_range, 0..320);
            assert_eq!(completed.result?, "controlled words");
            let collection = receipts.collect(Some(1), 320, Language::English);
            worker.shutdown()?;
            assert!(collection.error.is_none());
            let receipt = collection.receipt.ok_or("missing receipt")?;
            assert_eq!(receipt.variant, variant);
            assert_eq!(receipt.selected_flash_attention, variant.flash_attention());
            assert_eq!(
                receipt.decode_attempted_with_flash_attention,
                variant.flash_attention()
            );
            assert_eq!(receipt.observation["audioCtxParameter"], 0);
            assert_eq!(receipt.observation["effectiveAudioCtx"], 1500);
            assert!(receipts.finalize().error.is_none());

            // Energy-empty is successful admission with no native decode: retain
            // selected context policy, but never claim an executed Flash call.
            let receipts = Receipts::default();
            receipts.arm(1, 1, 1, 320, Language::German)?;
            let id = receipts.begin(320, Language::German)?;
            let output = Transcription {
                result: Ok(String::new()),
                observation: ObservationRecord {
                    input_frames: 320,
                    prepared_frames: 0,
                    preparation: reduced_context::Preparation::EnergyEmpty,
                    audio_ctx_parameter: None,
                    effective_audio_ctx: None,
                    decode_attempted: false,
                },
            };
            receipts.finish(id, variant, &output)?;
            let receipt = receipts
                .collect(Some(1), 320, Language::German)
                .receipt
                .ok_or("empty receipt lost")?;
            assert_eq!(receipt.selected_flash_attention, variant.flash_attention());
            assert_eq!(receipt.decode_attempted_with_flash_attention, None);
            assert!(receipts.finalize().error.is_none());
        }
        Ok(())
    }

    #[test]
    fn late_failed_flash_decode_preserves_flag_and_error_after_failed_observation()
    -> Result<(), Box<dyn std::error::Error>> {
        let receipts = Receipts::default();
        receipts.arm(9, 1, 9, 320, Language::German)?;
        let id = receipts.begin(320, Language::German)?;
        assert!(
            receipts
                .collect(Some(9), 320, Language::German)
                .error
                .is_some()
        );
        receipts.finish(
            id,
            Variant::FlashCandidate,
            &Transcription {
                result: Err(InferenceError::Decode),
                observation: ObservationRecord {
                    input_frames: 320,
                    prepared_frames: 16000,
                    preparation: reduced_context::Preparation::Ready,
                    audio_ctx_parameter: Some(0),
                    effective_audio_ctx: Some(1500),
                    decode_attempted: true,
                },
            },
        )?;
        let finalized = receipts.finalize();
        assert!(finalized.error.is_some());
        let receipt = finalized.receipt.ok_or("late Flash evidence lost")?;
        assert_eq!(receipt.identity.session, 9);
        assert_eq!(receipt.selected_flash_attention, Some(true));
        assert_eq!(receipt.decode_attempted_with_flash_attention, Some(true));
        assert!(receipt.decoder_error.is_some());
        assert!(receipts.arm(10, 1, 10, 320, Language::German).is_err());
        Ok(())
    }

    #[test]
    fn pending_receipt_admission_is_bounded_and_never_overwritten() -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(1, 1, 1, 3, Language::German)?;
        assert!(receipts.arm(2, 1, 2, 3, Language::English).is_err());
        Ok(())
    }
    struct Fixture {
        malformed: bool,
        fail: bool,
    }
    impl Backend for Fixture {
        fn transcribe(&mut self, a: &[f32], _: Language, _: Variant) -> Transcription {
            Transcription {
                result: if self.fail {
                    Err(InferenceError::Decode)
                } else {
                    Ok("controlled words".into())
                },
                observation: ObservationRecord {
                    input_frames: a.len() + usize::from(self.malformed),
                    prepared_frames: 16000,
                    preparation: reduced_context::Preparation::Ready,
                    audio_ctx_parameter: Some(1024),
                    effective_audio_ctx: Some(1024),
                    decode_attempted: true,
                },
            }
        }
    }
    #[test]
    fn malformed_receipt_is_invalid_but_actual_record_is_retained() -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(7, 1, 7, 3, Language::German)?;
        let mut decoder = Adapter {
            backend: Fixture {
                malformed: true,
                fail: false,
            },
            variant: Variant::Reduced,
            receipts: receipts.clone(),
        };
        let _result = decoder.decode(&[0.1; 3], Language::German);
        let observed = receipts.collect(Some(7), 3, Language::German);
        assert!(observed.error.is_some());
        assert!(observed.receipt.is_some());
        assert!(receipts.arm(8, 1, 8, 3, Language::German).is_err());
        Ok(())
    }

    #[test]
    fn receipt_identity_errors_and_extra_decode_never_overwrite_original() -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(3, 1, 3, 3, Language::German)?;
        let mut decoder = Adapter {
            backend: Fixture {
                malformed: false,
                fail: false,
            },
            variant: Variant::Reduced,
            receipts: receipts.clone(),
        };
        assert!(decoder.decode(&[0.1; 3], Language::German).is_ok());
        assert!(decoder.decode(&[0.1; 3], Language::German).is_err());
        let got = receipts.collect(Some(3), 3, Language::German);
        assert!(got.error.is_some());
        assert_eq!(got.receipt.ok_or("lost first receipt")?.identity.session, 3);
        assert!(receipts.arm(4, 1, 4, 3, Language::German).is_err());
        Ok(())
    }
    #[test]
    fn frames_language_and_foreign_session_mismatches_are_permanently_invalid() -> Result<(), String>
    {
        for (frames, language) in [(4, Language::German), (3, Language::English)] {
            let receipts = Receipts::default();
            receipts.arm(1, 1, 1, 3, Language::German)?;
            let mut decoder = Adapter {
                backend: Fixture {
                    malformed: false,
                    fail: false,
                },
                variant: Variant::Reduced,
                receipts: receipts.clone(),
            };
            assert!(decoder.decode(&vec![0.1; frames], language).is_err());
            let failed = receipts.collect(Some(1), 3, Language::German);
            assert!(failed.error.is_some());
            let expected = failed
                .uncompleted_request
                .ok_or("expected request lost on mismatch")?;
            assert_eq!(expected["endFrame"], 3);
            assert_eq!(expected["language"], "de");
            let finalized = receipts.finalize();
            assert_eq!(finalized.uncompleted_request, Some(expected));
            assert!(decoder.decode(&[0.1; 3], Language::German).is_err());
        }
        let receipts = Receipts::default();
        receipts.arm(1, 1, 1, 3, Language::German)?;
        let mut decoder = Adapter {
            backend: Fixture {
                malformed: false,
                fail: false,
            },
            variant: Variant::Reduced,
            receipts: receipts.clone(),
        };
        assert!(decoder.decode(&[0.1; 3], Language::German).is_ok());
        let bad = receipts.collect(Some(2), 3, Language::German);
        assert!(bad.error.is_some());
        assert!(bad.receipt.is_some());
        Ok(())
    }
    #[test]
    fn native_failure_and_late_receipt_remain_available_after_failed_observation()
    -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(1, 1, 1, 3, Language::German)?;
        let id = receipts
            .begin(3, Language::German)
            .map_err(|e| e.to_string())?;
        let timedout = receipts.collect(Some(1), 3, Language::German);
        assert!(timedout.error.is_some());
        assert!(timedout.uncompleted_request.is_some());
        let output = Fixture {
            malformed: false,
            fail: true,
        }
        .transcribe(&[0.1; 3], Language::German, Variant::Reduced);
        receipts
            .finish(id, Variant::Reduced, &output)
            .map_err(|e| e.to_string())?;
        let final_evidence = receipts.finalize();
        assert!(final_evidence.error.is_some());
        assert!(
            final_evidence
                .receipt
                .ok_or("late receipt lost")?
                .decoder_error
                .is_some()
        );
        Ok(())
    }
    #[test]
    fn missing_receipt_invalidates_next_admission() -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(1, 1, 1, 3, Language::German)?;
        assert!(
            receipts
                .collect(Some(1), 3, Language::German)
                .error
                .is_some()
        );
        assert!(receipts.arm(2, 1, 2, 3, Language::German).is_err());
        Ok(())
    }
    #[test]
    fn prepared_frames_cannot_exceed_input_or_short_padding() -> Result<(), String> {
        let receipts = Receipts::default();
        receipts.arm(1, 1, 1, 3, Language::German)?;
        let id = receipts
            .begin(3, Language::German)
            .map_err(|e| e.to_string())?;
        let mut output = Fixture {
            malformed: false,
            fail: false,
        }
        .transcribe(&[0.1; 3], Language::German, Variant::Reduced);
        output.observation.prepared_frames = 320000;
        assert!(receipts.finish(id, Variant::Reduced, &output).is_err());
        let record = receipts.collect(Some(1), 3, Language::German);
        assert!(record.error.is_some());
        assert_eq!(
            record.receipt.ok_or("lost invalid record")?.observation["preparedFrames"],
            320000
        );
        Ok(())
    }
}
