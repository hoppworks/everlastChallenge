use super::contract::{Expected, Role};
use everlast_dictation::{
    inference_service::{DecodeContext, Decoder, InferenceService, Job, WorkToken},
    transcription::{InferenceError, Language, prefix_cache::PreparedReuse},
};
use serde::Serialize;
use std::sync::{Arc, mpsc};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerEvidence {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub input_frames: usize,
    pub input_pcm_sha256: String,
    pub language: String,
    pub token_valid: bool,
}
pub struct Observed<D> {
    decoder: D,
    sender: mpsc::SyncSender<WorkerEvidence>,
}
pub fn observe<D: Decoder>(decoder: D) -> (Observed<D>, mpsc::Receiver<WorkerEvidence>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (Observed { decoder, sender }, receiver)
}
impl<D: Decoder> Decoder for Observed<D> {
    fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
        Err(InferenceError::InvalidDecodeContext)
    }
    fn decode_with_context(
        &mut self,
        audio: &[f32],
        language: Language,
        context: &DecodeContext,
    ) -> Result<String, InferenceError> {
        let result = self.decoder.decode_with_context(audio, language, context);
        let current = std::thread::current();
        let worker = WorkerEvidence {
            session: context.session,
            generation: context.generation,
            job_id: context.job_id,
            thread_id: format!("{:?}", current.id()),
            thread_name: current.name().map(str::to_owned),
            input_frames: audio.len(),
            input_pcm_sha256: pcm_sha(audio),
            language: language.code().into(),
            token_valid: context.is_valid(),
        };
        self.sender
            .try_send(worker)
            .map_err(|_| InferenceError::Observation)?;
        result
    }
    fn take_prepared_reuse(&mut self) -> Option<PreparedReuse> {
        self.decoder.take_prepared_reuse()
    }
}
pub fn pcm_sha(samples: &[f32]) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for sample in samples {
        hash.update(sample.to_le_bytes());
    }
    (hash.finalize())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

pub struct PairInput {
    pub prefix_id: String,
    pub whole_id: String,
    pub language: Language,
    pub prefix: Arc<Vec<f32>>,
    pub whole: Arc<Vec<f32>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub role: Role,
    pub input_id: String,
    pub input_frames: usize,
    pub input_pcm_sha256: String,
    pub generation: u64,
    pub completed_sample_range: Option<std::ops::Range<usize>>,
    pub completed_timeline_range: Option<std::ops::Range<usize>>,
    pub error: Option<&'static str>,
    pub text_bytes: Option<usize>,
    pub decoder_elapsed_nanos: Option<u128>,
    pub session: u64,
    pub job_id: u64,
    pub status: &'static str,
    pub text_sha256: Option<String>,
    pub worker: Option<WorkerEvidence>,
    pub prepared_reuse: Option<PreparedReuse>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRun {
    pub rows: Vec<Row>,
    pub joined: bool,
    pub cleanup_error: Option<&'static str>,
    pub final_receipt: Option<PreparedReuse>,
}
const WAIT: std::time::Duration = std::time::Duration::from_secs(120);

pub fn unavailable(input: &PairInput, pair_index: usize, error: &'static str) -> PairRun {
    PairRun {
        rows: planned_rows(input, pair_index)
            .into_iter()
            .map(|mut row| {
                row.error = Some(error);
                row
            })
            .collect(),
        joined: false,
        cleanup_error: None,
        final_receipt: None,
    }
}

fn planned_rows(input: &PairInput, pair_index: usize) -> Vec<Row> {
    let base = (pair_index as u64) * 3;
    [Role::Prefix, Role::SameSessionWhole, Role::NewSessionWhole]
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            let (id, samples) = if role == Role::Prefix {
                (&input.prefix_id, &input.prefix)
            } else {
                (&input.whole_id, &input.whole)
            };
            Row {
                role,
                input_id: id.clone(),
                input_frames: samples.len(),
                input_pcm_sha256: pcm_sha(samples),
                generation: 1,
                session: pair_index as u64 * 2 + if role == Role::NewSessionWhole { 2 } else { 1 },
                job_id: base + index as u64 + 1,
                completed_sample_range: None,
                completed_timeline_range: None,
                status: "unavailable",
                error: None,
                text_sha256: None,
                text_bytes: None,
                decoder_elapsed_nanos: None,
                worker: None,
                prepared_reuse: None,
            }
        })
        .collect()
}

fn ready(service: &InferenceService) -> Result<(), &'static str> {
    let start = std::time::Instant::now();
    while !service.ready() {
        if service.is_finished() {
            return Err("inference worker stopped before readiness");
        }
        if start.elapsed() >= WAIT {
            return Err("inference readiness observation timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}

fn execute(
    service: &InferenceService,
    receiver: &mpsc::Receiver<WorkerEvidence>,
    input: &PairInput,
    row: &mut Row,
    token: WorkToken,
    origin: Option<&Row>,
    parent_thread: &str,
) -> Result<(), &'static str> {
    let samples = if row.role == Role::Prefix {
        Arc::clone(&input.prefix)
    } else {
        Arc::clone(&input.whole)
    };
    service
        .submit(Job {
            session: row.session,
            generation: row.generation,
            job_id: row.job_id,
            sample_range: 0..samples.len(),
            timeline_range: 0..samples.len(),
            audio: samples,
            language: input.language,
            token,
        })
        .map_err(|_| "actual inference admission failed")?;
    let start = std::time::Instant::now();
    let mut completed = loop {
        if let Some(result) = service
            .take_result()
            .map_err(|_| "actual result observation failed")?
        {
            break result;
        }
        if service.is_finished() {
            return Err("inference worker stopped before result");
        }
        if start.elapsed() >= WAIT {
            return Err("inference result observation timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    row.completed_sample_range = Some(completed.sample_range.clone());
    row.completed_timeline_range = Some(completed.timeline_range.clone());
    row.prepared_reuse = completed.prepared_reuse.take();
    row.worker = receiver.try_recv().ok();
    row.decoder_elapsed_nanos = completed
        .decode_timing
        .map(|timing| timing.elapsed.as_nanos());
    if completed.session != row.session
        || completed.generation != row.generation
        || completed.job_id != row.job_id
        || completed.sample_range != (0..row.input_frames)
        || completed.timeline_range != (0..row.input_frames)
        || completed
            .decode_timing
            .is_none_or(|t| t.input_frames != row.input_frames)
    {
        return Err("completed inference identity/range/timing differs");
    }
    let worker = row
        .worker
        .as_ref()
        .ok_or("actual worker evidence missing")?;
    if worker.session != row.session
        || worker.generation != row.generation
        || worker.job_id != row.job_id
        || worker.thread_id == parent_thread
        || worker.thread_name.as_deref() != Some("local-inference")
        || worker.input_frames != row.input_frames
        || worker.input_pcm_sha256 != row.input_pcm_sha256
        || worker.language != input.language.code()
        || !worker.token_valid
    {
        return Err("actual worker identity/PCM/thread/token differs");
    }
    let text = completed
        .result
        .map_err(|_| "actual decoder returned an error")?;
    if text.len() > 256 * 1024 {
        return Err("actual result exceeded bounded text limit");
    }
    row.text_bytes = Some(text.len());
    row.text_sha256 = Some(super::input::digest(text.as_bytes()));
    let receipt = row
        .prepared_reuse
        .as_ref()
        .ok_or("actual prepared reuse receipt missing")?;
    super::contract::validate_receipt(
        &Expected {
            session: row.session,
            generation: row.generation,
            job_id: row.job_id,
            role: row.role,
        },
        receipt,
        origin.and_then(|r| r.prepared_reuse.as_ref()),
    )?;
    if receipt.outcome == everlast_dictation::transcription::prefix_cache::ReuseOutcome::Hit
        && origin.is_none_or(|r| r.text_sha256 != row.text_sha256 || r.text_bytes != row.text_bytes)
    {
        return Err("cache hit text fingerprint differs from original prefix result");
    }
    Ok(())
}

fn shutdown(service: InferenceService) -> (bool, Option<&'static str>) {
    // A timeout is unverified cleanup, never a claimed native interrupt. Main
    // fails, no further model is loaded, and the external supervisor owns exit.
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("prefix-probe-join".into())
        .spawn(move || {
            let result = service
                .shutdown()
                .map_err(|_| "inference owner shutdown/join failed");
            let _delivery = sender.send(result);
        });
    let Ok(handle) = thread else {
        return (false, Some("cleanup observer could not start"));
    };
    match receiver.recv_timeout(WAIT) {
        Ok(result) => {
            if handle.join().is_err() {
                return (false, Some("cleanup observer panicked"));
            }
            match result {
                Ok(()) => (true, None),
                Err(error) => (false, Some(error)),
            }
        }
        Err(_) => (
            false,
            Some("cleanup join not verified within observation budget"),
        ),
    }
}

pub fn run_pair(
    service: InferenceService,
    receiver: mpsc::Receiver<WorkerEvidence>,
    input: &PairInput,
    pair_index: usize,
) -> PairRun {
    let mut rows = planned_rows(input, pair_index);
    let mut blocked = ready(&service).err();
    let token = WorkToken::default();
    let fresh_token = WorkToken::default();
    let parent_thread = format!("{:?}", std::thread::current().id());
    for index in 0..rows.len() {
        if let Some(error) = blocked {
            rows[index].error = Some(error);
            continue;
        }
        let (before, after) = rows.split_at_mut(index);
        let row = &mut after[0];
        let work_token = if row.role == Role::NewSessionWhole {
            fresh_token.clone()
        } else {
            token.clone()
        };
        match execute(
            &service,
            &receiver,
            input,
            row,
            work_token,
            before.first(),
            &parent_thread,
        ) {
            Ok(()) => row.status = "ok",
            Err(error) => {
                row.status = "failed";
                row.error = Some(error);
                blocked = Some(error);
            }
        }
    }
    let final_receipt = rows
        .iter()
        .rev()
        .find(|row| row.completed_sample_range.is_some())
        .and_then(|row| row.prepared_reuse.clone());
    let (joined, cleanup_error) = shutdown(service);
    PairRun {
        rows,
        joined,
        cleanup_error,
        final_receipt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everlast_dictation::transcription::prefix_cache::ReuseOutcome;
    struct Controlled {
        session: Option<u64>,
        receipt: Option<PreparedReuse>,
        origin: u64,
    }
    impl Decoder for Controlled {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            Err(InferenceError::InvalidDecodeContext)
        }
        fn decode_with_context(
            &mut self,
            _: &[f32],
            _: Language,
            c: &DecodeContext,
        ) -> Result<String, InferenceError> {
            let hit = self.session == Some(c.session);
            if !hit {
                self.origin = c.job_id;
            }
            self.session = Some(c.session);
            self.receipt = Some(PreparedReuse {
                session: c.session,
                generation: c.generation,
                job_id: c.job_id,
                outcome: if hit {
                    ReuseOutcome::Hit
                } else {
                    ReuseOutcome::Decoded
                },
                origin_job_id: Some(self.origin),
                prepared_frames: Some(16000),
                prepared_pcm_sha256: Some([7; 32]),
                decoder_invoked: !hit,
                native_full_attempted: !hit,
            });
            Ok("private test transcript never exported".into())
        }
        fn take_prepared_reuse(&mut self) -> Option<PreparedReuse> {
            self.receipt.take()
        }
    }
    #[test]
    fn actual_service_observes_worker_identity_and_fresh_session_without_text_export()
    -> Result<(), Box<dyn std::error::Error>> {
        let (decoder, receiver) = observe(Controlled {
            session: None,
            receipt: None,
            origin: 0,
        });
        let service = InferenceService::with_decoder(decoder)?;
        let input = PairInput {
            prefix_id: "prefix".into(),
            whole_id: "whole".into(),
            language: Language::German,
            prefix: Arc::new(vec![0.25, 0.5]),
            whole: Arc::new(vec![0.25, 0.5, 0.75]),
        };
        let run = run_pair(service, receiver, &input, 0);
        assert!(run.joined);
        assert_eq!(run.rows.len(), 3);
        assert!(run.rows.iter().all(|row| row.status == "ok"));
        assert_eq!(
            run.rows.iter().map(|r| r.session).collect::<Vec<_>>(),
            vec![1, 1, 2]
        );
        assert_eq!(
            run.rows.iter().map(|r| r.job_id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let current = format!("{:?}", std::thread::current().id());
        for row in &run.rows {
            let worker = row.worker.as_ref().ok_or("missing actual worker")?;
            assert_ne!(worker.thread_id, current);
            assert_eq!(worker.thread_name.as_deref(), Some("local-inference"));
        }
        let serialized = serde_json::to_string(&run)?;
        assert!(!serialized.contains("private test transcript"));
        Ok(())
    }
    struct Fails;
    impl Decoder for Fails {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            Err(InferenceError::Decode)
        }
    }
    #[test]
    fn decoder_failure_preserves_all_planned_rows_and_actual_worker_then_joins()
    -> Result<(), Box<dyn std::error::Error>> {
        let (decoder, receiver) = observe(Fails);
        let service = InferenceService::with_decoder(decoder)?;
        let input = PairInput {
            prefix_id: "p".into(),
            whole_id: "w".into(),
            language: Language::English,
            prefix: Arc::new(vec![0.25]),
            whole: Arc::new(vec![0.25, 0.5]),
        };
        let run = run_pair(service, receiver, &input, 1);
        assert!(run.joined);
        assert_eq!(
            run.rows.iter().map(|row| row.status).collect::<Vec<_>>(),
            vec!["failed", "unavailable", "unavailable"]
        );
        assert_eq!(
            run.rows.iter().map(|row| row.job_id).collect::<Vec<_>>(),
            vec![4, 5, 6]
        );
        assert!(run.rows[0].worker.is_some());
        assert!(run.rows[0].error.is_some());
        assert!(run.rows[1].worker.is_none());
        Ok(())
    }
    struct MissingReceipt(Controlled);
    impl Decoder for MissingReceipt {
        fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
            self.0.decode(audio, language)
        }
        fn decode_with_context(
            &mut self,
            audio: &[f32],
            language: Language,
            context: &DecodeContext,
        ) -> Result<String, InferenceError> {
            self.0.decode_with_context(audio, language, context)
        }
        fn take_prepared_reuse(&mut self) -> Option<PreparedReuse> {
            self.0
                .take_prepared_reuse()
                .filter(|receipt| receipt.job_id != 2)
        }
    }
    #[test]
    fn last_completed_without_receipt_does_not_export_an_older_prefix_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let (decoder, receiver) = observe(MissingReceipt(Controlled {
            session: None,
            receipt: None,
            origin: 0,
        }));
        let service = InferenceService::with_decoder(decoder)?;
        let input = PairInput {
            prefix_id: "p".into(),
            whole_id: "w".into(),
            language: Language::German,
            prefix: Arc::new(vec![0.25]),
            whole: Arc::new(vec![0.25, 0.5]),
        };
        let run = run_pair(service, receiver, &input, 0);
        assert!(run.joined);
        assert_eq!(
            run.rows.iter().map(|row| row.status).collect::<Vec<_>>(),
            vec!["ok", "failed", "unavailable"]
        );
        assert!(run.rows[0].prepared_reuse.is_some());
        assert!(run.rows[1].completed_sample_range.is_some());
        assert!(run.rows[1].prepared_reuse.is_none());
        assert!(run.final_receipt.is_none());
        Ok(())
    }
}
