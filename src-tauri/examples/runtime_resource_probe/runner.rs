//! Serial resource workload at the actual measured-runtime boundary.

use crossbeam_channel::{Receiver, Sender, bounded};
use dictation_core::session::Phase;
use everlast_dictation::{
    audio::replay::clock::{FeedEvidence, feed},
    controller::SessionSettings,
    inference_service::Decoder,
    runtime::measurement::{MeasuredRuntime, MeasuredTerminal},
    transcription::{InferenceError, Language},
};
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const WARMUP_RUNS: u32 = 5;
pub const MEASURED_RUNS: u32 = 100;
pub const WAIT: Duration = Duration::from_secs(120);
#[derive(Clone, Copy)]
pub struct Config {
    pub warmups: u32,
    pub cycles: u32,
    pub timeout: Duration,
}
impl Config {
    pub fn production() -> Self {
        Self {
            warmups: WARMUP_RUNS,
            cycles: MEASURED_RUNS,
            timeout: WAIT,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Operation {
    StopComplete,
    CancelCapture,
    CancelDecoderEntry,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusyObservation {
    pub error: String,
    pub count: u64,
    pub first_micros: u128,
    pub last_micros: u128,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub warmup: bool,
    pub operation: Operation,
    pub session: Option<u64>,
    pub status: &'static str,
    pub error: Option<String>,
    pub elapsed_micros: u128,
    pub start_busy_observations: Vec<BusyObservation>,
    pub terminal: Option<String>,
    pub reference_match: Option<bool>,
    pub text_absent: bool,
    pub decoder_started: u64,
    pub decoder_completed: u64,
    pub decoder_compared: u64,
    pub decoder_mismatches: u64,
    pub decoder_error: Option<String>,
    pub controller_idle_and_wrapper_idle: bool,
    pub owner_free_confirmed_by_next_start: bool,
    pub feed_frames: usize,
    pub publication_count: usize,
    pub feed_timing_valid: Option<bool>,
    pub maximum_publication_deviation_nanos: Option<u128>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workload {
    pub behavior_passed: bool,
    pub setup_timing_passed: bool,
    pub first_error: Option<String>,
    pub rows: Vec<Row>,
    pub decoder_calls_started: u64,
    pub decoder_calls_completed: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub after_id: String,
    pub status: &'static str,
    pub include_vmmap: bool,
    pub controller_idle_and_wrapper_idle: bool,
}

#[derive(Default)]
struct State {
    armed: AtomicBool,
    active: AtomicBool,
    started: AtomicU64,
    completed: AtomicU64,
    compared: AtomicU64,
    mismatches: AtomicU64,
    error: std::sync::Mutex<Option<(u64, String)>>,
}
pub struct Controlled<D> {
    inner: D,
    expected: Arc<[f32]>,
    state: Arc<State>,
    entered: Sender<u64>,
    release: Receiver<()>,
    timeout: Duration,
}
pub struct DecoderControl {
    state: Arc<State>,
    entered: Receiver<u64>,
    release: Sender<()>,
}
/// Initialize backend-owned resources inside the measured serial worker.
/// Only the factory and the ordinary control channels cross threads.
pub fn controlled_service<D: Decoder>(
    initialize: impl FnOnce() -> Result<D, InferenceError> + Send + 'static,
    expected: Arc<[f32]>,
    timeout: Duration,
) -> Result<
    (
        everlast_dictation::inference_service::InferenceService,
        DecoderControl,
    ),
    everlast_dictation::inference_service::ServiceError,
> {
    let state = Arc::new(State::default());
    let worker_state = Arc::clone(&state);
    let (entered, receive) = bounded(1);
    let (release, proceed) = bounded(1);
    let service =
        everlast_dictation::inference_service::InferenceService::with_initializer(move || {
            Ok(Controlled {
                inner: initialize()?,
                expected,
                state: worker_state,
                entered,
                release: proceed,
                timeout,
            })
        })?;
    Ok((
        service,
        DecoderControl {
            state,
            entered: receive,
            release,
        },
    ))
}

#[cfg(test)]
pub fn controlled<D: Decoder>(
    inner: D,
    expected: Arc<[f32]>,
    timeout: Duration,
) -> (Controlled<D>, DecoderControl) {
    let state = Arc::new(State::default());
    let (entered, receive) = bounded(1);
    let (release, proceed) = bounded(1);
    (
        Controlled {
            inner,
            expected,
            state: Arc::clone(&state),
            entered,
            release: proceed,
            timeout,
        },
        DecoderControl {
            state,
            entered: receive,
            release,
        },
    )
}
struct Active<'a>(&'a State);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.completed.fetch_add(1, Ordering::AcqRel);
        self.0.active.store(false, Ordering::Release);
    }
}
impl<D: Decoder> Decoder for Controlled<D> {
    fn finish(self) -> Result<(), InferenceError> {
        self.inner.finish()
    }

    fn decode(&mut self, audio: &[f32], language: Language) -> Result<String, InferenceError> {
        self.state.active.store(true, Ordering::Release);
        let _active = Active(&self.state);
        let call = self.state.started.fetch_add(1, Ordering::AcqRel) + 1;
        let result = (|| {
            if self.state.armed.swap(false, Ordering::AcqRel) {
                self.entered
                    .try_send(call)
                    .map_err(|_| InferenceError::Decode)?;
                self.release
                    .recv_timeout(self.timeout)
                    .map_err(|_| InferenceError::Decode)?;
            }
            self.state.compared.fetch_add(1, Ordering::AcqRel);
            if audio != self.expected.as_ref() {
                self.state.mismatches.fetch_add(1, Ordering::AcqRel);
                Err(InferenceError::InvalidAudio)
            } else {
                self.inner.decode(audio, language)
            }
        })();
        if let Err(error) = &result {
            *self
                .state
                .error
                .lock()
                .map_err(|_| InferenceError::Decode)? = Some((call, error.to_string()));
        }
        result
    }
}
struct Release<'a>(&'a DecoderControl);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        if self.0.release.try_send(()).is_err() { /* bounded release already supplied or owner ended */
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecoderDiagnostics {
    pub started: u64,
    pub completed: u64,
    pub compared: u64,
    pub mismatches: u64,
    pub active: bool,
    pub armed: bool,
    pub last_error: Option<(u64, String)>,
}
impl DecoderControl {
    pub fn diagnostics(&self) -> Result<DecoderDiagnostics, String> {
        let last_error = self
            .state
            .error
            .lock()
            .map_err(|_| "decoder diagnostic lock poisoned")?
            .clone();
        Ok(DecoderDiagnostics {
            started: self.state.started.load(Ordering::Acquire),
            completed: self.state.completed.load(Ordering::Acquire),
            compared: self.state.compared.load(Ordering::Acquire),
            mismatches: self.state.mismatches.load(Ordering::Acquire),
            active: self.state.active.load(Ordering::Acquire),
            armed: self.state.armed.load(Ordering::Acquire),
            last_error,
        })
    }
    fn arm(&self) -> Result<Release<'_>, String> {
        if self.state.active.load(Ordering::Acquire)
            || self.state.armed.swap(true, Ordering::AcqRel)
        {
            return Err("decoder gate is already occupied".into());
        }
        if !self.entered.is_empty() || !self.release.is_empty() {
            self.state.armed.store(false, Ordering::Release);
            return Err("decoder gate retained stale messages".into());
        }
        Ok(Release(self))
    }
}
fn row(id: String, warmup: bool, operation: Operation) -> Row {
    Row {
        id,
        warmup,
        operation,
        session: None,
        status: "failed",
        error: None,
        elapsed_micros: 0,
        start_busy_observations: Vec::new(),
        terminal: None,
        reference_match: None,
        text_absent: false,
        decoder_started: 0,
        decoder_completed: 0,
        decoder_compared: 0,
        decoder_mismatches: 0,
        decoder_error: None,
        controller_idle_and_wrapper_idle: false,
        owner_free_confirmed_by_next_start: false,
        feed_frames: 0,
        publication_count: 0,
        feed_timing_valid: None,
        maximum_publication_deviation_nanos: None,
    }
}
fn start(runtime: &MeasuredRuntime, row: &mut Row, timeout: Duration) -> Result<u64, String> {
    let started = Instant::now();
    loop {
        let mut reply = runtime
            .start(SessionSettings {
                refinement: false,
                device: None,
                language: Language::German,
                generation: 1,
            })
            .map_err(|e| e.to_string())?;
        let result = loop {
            match reply.try_recv() {
                Ok(value) => break value,
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
                    if started.elapsed() < timeout =>
                {
                    thread::sleep(Duration::from_millis(1))
                }
                _ => return Err("start acknowledgement missing before bounded deadline".into()),
            }
        };
        match result {
            Ok(id) => return Ok(id),
            Err(error)
                if error
                    == "Controller service failed: Previous measurement or inference owner is still busy"
                    || error
                        == "Controller service failed: Previous audio owner has not joined" =>
            {
                let elapsed = started.elapsed().as_micros();
                if let Some(old) = row
                    .start_busy_observations
                    .iter_mut()
                    .find(|v| v.error == error)
                {
                    old.count += 1;
                    old.last_micros = elapsed;
                } else {
                    row.start_busy_observations.push(BusyObservation {
                        error,
                        count: 1,
                        first_micros: elapsed,
                        last_micros: elapsed,
                    });
                }
                if started.elapsed() >= timeout {
                    return Err(
                        "owner quiescence start handshake timed out (busy observations retained)"
                            .into(),
                    );
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}
fn record_feed(row: &mut Row, evidence: FeedEvidence, frames: usize) {
    row.feed_frames = frames;
    row.publication_count = evidence.publications.len();
    row.feed_timing_valid = Some(evidence.timing_valid);
    row.maximum_publication_deviation_nanos = evidence
        .publications
        .iter()
        .map(|p| p.deviation_nanos)
        .max();
}
fn decoder_counts(control: &DecoderControl) -> [u64; 4] {
    [
        &control.state.started,
        &control.state.completed,
        &control.state.compared,
        &control.state.mismatches,
    ]
    .map(|v| v.load(Ordering::Acquire))
}
fn update_counts(row: &mut Row, control: &DecoderControl, before: [u64; 4]) -> Result<(), String> {
    let now = decoder_counts(control);
    row.decoder_started = now[0] - before[0];
    row.decoder_completed = now[1] - before[1];
    row.decoder_compared = now[2] - before[2];
    row.decoder_mismatches = now[3] - before[3];
    if let Some((call, error)) = control
        .state
        .error
        .lock()
        .map_err(|_| "decoder diagnostic lock poisoned")?
        .as_ref()
        && *call > before[0]
    {
        row.decoder_error = Some(error.clone());
    }
    Ok(())
}
fn one(
    runtime: &MeasuredRuntime,
    control: &DecoderControl,
    samples: &[f32],
    reference: &str,
    row: &mut Row,
    timeout: Duration,
) -> Result<(), String> {
    let before = decoder_counts(control);
    let session = start(runtime, row, timeout)?;
    row.session = Some(session);
    let mut source = runtime.take_input(timeout).map_err(|e| e.to_string())?;
    let gate = if row.operation == Operation::CancelDecoderEntry {
        Some(control.arm()?)
    } else {
        None
    };
    let input = if row.operation == Operation::CancelCapture {
        &samples[..samples.len().min(320)]
    } else {
        samples
    };
    record_feed(
        row,
        feed(&mut source, input).map_err(|e| e.to_string())?,
        input.len(),
    );
    match row.operation {
        Operation::StopComplete => {
            runtime.stop(session).map_err(|e| e.to_string())?;
        }
        Operation::CancelCapture => runtime.cancel(session).map_err(|e| e.to_string())?,
        Operation::CancelDecoderEntry => {
            runtime.stop(session).map_err(|e| e.to_string())?;
            control
                .entered
                .recv_timeout(timeout)
                .map_err(|e| format!("decoder entry observation failed: {e}"))?;
            runtime.cancel(session).map_err(|e| e.to_string())?;
        }
    }
    drop(gate);
    drop(source);
    let deadline = Instant::now() + timeout;
    loop {
        let state = runtime.snapshot().map_err(|e| e.to_string())?;
        let timing = runtime.timing().map_err(|e| e.to_string())?;
        if timing.session != Some(session)
            || (state.session.phase != Phase::Idle && state.session.session != Some(session))
        {
            return Err("session identity changed during workload".into());
        }
        row.terminal = timing.terminal.map(|t| format!("{t:?}"));
        row.text_absent = state.session.text.is_none();
        if state.session.phase == Phase::Idle
            && timing.terminal.is_some()
            && !control.state.active.load(Ordering::Acquire)
        {
            row.controller_idle_and_wrapper_idle = true;
            update_counts(row, control, before)?;
            let expected = if row.operation == Operation::CancelCapture {
                0
            } else {
                1
            };
            if row.decoder_mismatches != 0 {
                return Err(
                    "actual captured decoder input differs from the complete fixture".into(),
                );
            }
            if let Some(error) = &row.decoder_error {
                return Err(format!("decoder failure: {error}"));
            }
            if row.decoder_started != expected
                || row.decoder_completed != expected
                || row.decoder_compared != expected
            {
                return Err("decoder call/completion/input-comparison count differs from the operation contract".into());
            }
            if let Some(error) = state.error {
                return Err(error);
            }
            if let Some(dictation_core::session::Outcome::Failed(message)) = &state.session.outcome
            {
                return Err(format!("controller failure: {message}"));
            }
            match row.operation {
                Operation::StopComplete => {
                    row.reference_match =
                        state.session.text.as_deref().map(|text| text == reference);
                    if timing.terminal != Some(MeasuredTerminal::Local)
                        || row.reference_match != Some(true)
                        || timing.t_text.is_none()
                    {
                        return Err(
                            "stop result failed exact reference/Local/text-time contract".into(),
                        );
                    }
                }
                _ => {
                    if timing.terminal != Some(MeasuredTerminal::Cancelled)
                        || !row.text_absent
                        || timing.t_text.is_some()
                    {
                        return Err(
                            "cancel retained text or accepted a cancelled inference result".into(),
                        );
                    }
                }
            }
            return Ok(());
        }
        if !state.running {
            return Err("runtime ended during workload".into());
        }
        if Instant::now() >= deadline {
            return Err("controller/decoder idle observation timed out".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
}
pub fn workload(
    runtime: &MeasuredRuntime,
    control: &DecoderControl,
    samples: &[f32],
    reference: &str,
    config: Config,
    mut checkpoint: impl FnMut(Checkpoint),
) -> Workload {
    let mut rows: Vec<Row> = Vec::new();
    let mut first_error = None;
    for index in 0..config.warmups + config.cycles {
        let warmup = index < config.warmups;
        let measured = index.saturating_sub(config.warmups);
        let operation = if warmup || measured % 3 == 0 {
            Operation::StopComplete
        } else if measured % 3 == 1 {
            Operation::CancelCapture
        } else {
            Operation::CancelDecoderEntry
        };
        let id = if warmup {
            format!("warmup-{:03}", index + 1)
        } else {
            format!("measured-{:03}", measured + 1)
        };
        let mut row = row(id, warmup, operation);
        let started = Instant::now();
        if let Some(error) = &first_error {
            row.status = "unavailable";
            row.error = Some(format!("preceding failure: {error}"));
        } else {
            let before = decoder_counts(control);
            let mut result = one(
                runtime,
                control,
                samples,
                reference,
                &mut row,
                config.timeout,
            );
            if let Err(error) = update_counts(&mut row, control, before) {
                result = Err(format!("{}; {error}", result.err().unwrap_or_default()));
            }
            if row.session.is_some()
                && let Some(previous) = rows.last_mut()
            {
                previous.owner_free_confirmed_by_next_start = true;
            }
            match result {
                Ok(()) => row.status = "ok",
                Err(error) => {
                    let mut error = error;
                    if let Some(session) = row.session
                        && let Err(cancel) = runtime.cancel(session)
                    {
                        error.push_str(&format!("; failure cleanup cancel: {cancel}"));
                    }
                    first_error = Some(error.clone());
                    row.error = Some(error);
                }
            }
        }
        row.elapsed_micros = started.elapsed().as_micros();
        if !warmup || index + 1 == config.warmups {
            checkpoint(Checkpoint {
                after_id: row.id.clone(),
                status: row.status,
                include_vmmap: index + 1 == config.warmups
                    || index + 1 == config.warmups + config.cycles,
                controller_idle_and_wrapper_idle: row.controller_idle_and_wrapper_idle,
            });
        }
        rows.push(row);
    }
    Workload {
        setup_timing_passed: rows.iter().all(|row| row.feed_timing_valid == Some(true)),
        behavior_passed: first_error.is_none(),
        first_error,
        rows,
        decoder_calls_started: control.state.started.load(Ordering::Acquire),
        decoder_calls_completed: control.state.completed.load(Ordering::Acquire),
    }
}

pub fn unavailable(error: String, config: Config) -> Workload {
    let mut rows = Vec::new();
    for index in 0..config.warmups + config.cycles {
        let warmup = index < config.warmups;
        let measured = index.saturating_sub(config.warmups);
        let operation = if warmup || measured % 3 == 0 {
            Operation::StopComplete
        } else if measured % 3 == 1 {
            Operation::CancelCapture
        } else {
            Operation::CancelDecoderEntry
        };
        let id = if warmup {
            format!("warmup-{:03}", index + 1)
        } else {
            format!("measured-{:03}", measured + 1)
        };
        let mut row = row(id, warmup, operation);
        row.status = "unavailable";
        row.error = Some(error.clone());
        rows.push(row);
    }
    Workload {
        behavior_passed: false,
        setup_timing_passed: false,
        first_error: Some(error),
        rows,
        decoder_calls_started: 0,
        decoder_calls_completed: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dictation_core::audio::AudioFormat;
    use everlast_dictation::{inference_service::InferenceService, transcription::InferenceError};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Fixed(Arc<AtomicUsize>);
    #[test]
    fn controlled_decoder_finishes_its_thread_owned_inner() -> Result<(), InferenceError> {
        use std::{cell::Cell, rc::Rc};
        struct Owned(Rc<Cell<bool>>);
        impl Decoder for Owned {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok(String::new())
            }
            fn finish(self) -> Result<(), InferenceError> {
                self.0.set(true);
                Ok(())
            }
        }
        let finished = Rc::new(Cell::new(false));
        let (decoder, _) = controlled(Owned(Rc::clone(&finished)), Arc::from([]), WAIT);
        decoder.finish()?;
        assert!(
            finished.get(),
            "resource wrapper skipped backend-owned cleanup"
        );
        Ok(())
    }

    #[test]
    fn resource_service_creates_and_finishes_non_send_decoder_on_one_worker()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::{rc::Rc, thread::ThreadId};
        struct Owned {
            owner: ThreadId,
            _local: Rc<()>,
            finished: Sender<ThreadId>,
        }
        impl Decoder for Owned {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok(String::new())
            }
            fn finish(self) -> Result<(), InferenceError> {
                if self.owner != thread::current().id() {
                    return Err(InferenceError::Decode);
                }
                self.finished
                    .try_send(self.owner)
                    .map_err(|_| InferenceError::Decode)
            }
        }
        let (entered, ready) = bounded(1);
        let (finished, observed) = bounded(1);
        let caller = thread::current().id();
        let (service, control) = controlled_service(
            move || {
                let owner = thread::current().id();
                entered
                    .try_send(owner)
                    .map_err(|_| InferenceError::Decode)?;
                Ok(Owned {
                    owner,
                    _local: Rc::new(()),
                    finished,
                })
            },
            Arc::from([]),
            WAIT,
        )?;
        let owner = ready.recv_timeout(Duration::from_secs(5))?;
        assert_ne!(owner, caller);
        service.shutdown()?;
        assert_eq!(observed.recv_timeout(Duration::from_secs(5))?, owner);
        assert_eq!(control.diagnostics()?.started, 0);
        Ok(())
    }

    impl Decoder for Fixed {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            assert_eq!(audio, &[0.125; 640]);
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("controlled reference".into())
        }
    }
    #[test]
    fn actual_runtime_workload_completes_and_cancels_without_retained_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(AtomicUsize::new(0));
        let (decoder, control) = controlled(
            Fixed(Arc::clone(&calls)),
            Arc::from([0.125; 640]),
            Duration::from_secs(2),
        );
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(decoder)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        crate::shared::wait_ready(&runtime)?;
        let mut checkpoints = Vec::new();
        let report = workload(
            &runtime,
            &control,
            &[0.125; 640],
            "controlled reference",
            Config {
                warmups: 1,
                cycles: 4,
                timeout: Duration::from_secs(2),
            },
            |point| checkpoints.push(point),
        );
        let cleanup = crate::shared::cleanup(runtime);
        assert!(cleanup.joined);
        assert!(report.behavior_passed, "{:?}", report.first_error);
        assert_eq!(calls.load(Ordering::SeqCst), 4); // warmup + stop + cancelled entry + stop
        assert_eq!(report.rows.len(), 5);
        assert_eq!(checkpoints.len(), 5);
        assert!(checkpoints[0].include_vmmap);
        assert!(checkpoints[4].include_vmmap);
        for row in &report.rows {
            assert_eq!(row.status, "ok");
            if row.operation != Operation::StopComplete {
                assert_eq!(row.terminal.as_deref(), Some("Cancelled"));
                assert_eq!(row.reference_match, None);
                assert!(row.text_absent);
            }
        }
        Ok(())
    }
    #[test]
    fn decoder_failure_is_retained_remaining_rows_unavailable_and_owner_is_joined()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Broken(Arc<AtomicUsize>);
        impl Drop for Broken {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        impl Decoder for Broken {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Err(InferenceError::Decode)
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let (decoder, control) = controlled(
            Broken(Arc::clone(&dropped)),
            Arc::from([0.125; 640]),
            Duration::from_secs(2),
        );
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(decoder)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        crate::shared::wait_ready(&runtime)?;
        let report = workload(
            &runtime,
            &control,
            &[0.125; 640],
            "never log this reference",
            Config {
                warmups: 1,
                cycles: 3,
                timeout: Duration::from_secs(2),
            },
            |_| {},
        );
        let cleanup = crate::shared::cleanup(runtime);
        assert!(cleanup.joined);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert!(!report.behavior_passed);
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.rows[0].status, "failed");
        assert!(
            report.rows[0]
                .error
                .as_deref()
                .is_some_and(|e| e.contains("Local transcription failed"))
        );
        assert!(
            report
                .rows
                .iter()
                .skip(1)
                .all(|row| row.status == "unavailable" && row.session.is_none())
        );
        assert_eq!(report.decoder_calls_started, 1);
        assert!(!serde_json::to_string(&report)?.contains("never log this reference"));
        Ok(())
    }
    #[test]
    fn a_busy_old_decoder_has_a_bounded_rejected_start_with_observations_retained()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(AtomicUsize::new(0));
        let (decoder, control) = controlled(
            Fixed(Arc::clone(&calls)),
            Arc::from([0.125; 640]),
            Duration::from_secs(2),
        );
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(decoder)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        crate::shared::wait_ready(&runtime)?;
        let mut first = row("old".into(), false, Operation::CancelDecoderEntry);
        let session = start(&runtime, &mut first, Duration::from_secs(2))?;
        let release = control.arm()?;
        let mut source = runtime.take_input(Duration::from_secs(2))?;
        feed(&mut source, &[0.125; 640])?;
        runtime.stop(session)?;
        control.entered.recv_timeout(Duration::from_secs(2))?;
        runtime.cancel(session)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while runtime.snapshot()?.session.phase != Phase::Idle {
            if Instant::now() > deadline {
                return Err("cancel core did not become idle".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        let mut next = row("next".into(), false, Operation::StopComplete);
        let attempt = start(&runtime, &mut next, Duration::from_millis(20));
        drop(release);
        drop(source);
        let cleanup = crate::shared::cleanup(runtime);
        assert!(cleanup.joined);
        assert!(attempt.is_err());
        assert!(!next.start_busy_observations.is_empty());
        assert!(
            next.start_busy_observations
                .iter()
                .all(|v| v.count > 0 && v.error.contains("still busy"))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn full_capture_input_mismatch_cannot_be_hidden_by_a_constant_transcript()
    -> Result<(), Box<dyn std::error::Error>> {
        struct AnyInput;
        impl Decoder for AnyInput {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok("same text".into())
            }
        }
        let (decoder, control) =
            controlled(AnyInput, Arc::from([0.25; 640]), Duration::from_secs(2));
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(decoder)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        crate::shared::wait_ready(&runtime)?;
        let report = workload(
            &runtime,
            &control,
            &[0.125; 640],
            "same text",
            Config {
                warmups: 1,
                cycles: 1,
                timeout: Duration::from_secs(2),
            },
            |_| {},
        );
        let cleanup = crate::shared::cleanup(runtime);
        assert!(cleanup.joined);
        assert!(!report.behavior_passed);
        assert_eq!(report.rows[0].decoder_started, 1);
        assert_eq!(report.rows[0].decoder_completed, 1);
        assert_eq!(report.rows[0].decoder_compared, 1);
        assert_eq!(report.rows[0].decoder_mismatches, 1);
        assert!(
            report.rows[0]
                .error
                .as_deref()
                .is_some_and(|e| e.contains("differs from the complete fixture"))
        );
        assert_eq!(report.rows[1].status, "unavailable");
        Ok(())
    }
    #[test]
    fn cancelled_inner_decode_error_remains_a_failed_row_and_survives_cleanup_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        struct ThirdFails(u32);
        impl Decoder for ThirdFails {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                self.0 += 1;
                if self.0 == 3 {
                    Err(InferenceError::Decode)
                } else {
                    Ok("reference".into())
                }
            }
        }
        let (decoder, control) = controlled(
            ThirdFails(0),
            Arc::from([0.125; 640]),
            Duration::from_secs(2),
        );
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(decoder)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        crate::shared::wait_ready(&runtime)?;
        let report = workload(
            &runtime,
            &control,
            &[0.125; 640],
            "reference",
            Config {
                warmups: 1,
                cycles: 4,
                timeout: Duration::from_secs(2),
            },
            |_| {},
        );
        let cleanup = crate::shared::cleanup(runtime);
        let diagnostic = control.diagnostics()?;
        assert!(cleanup.joined);
        assert!(!report.behavior_passed);
        let cancelled = &report.rows[3];
        assert_eq!(cancelled.operation, Operation::CancelDecoderEntry);
        assert_eq!(cancelled.status, "failed");
        assert_eq!(cancelled.terminal.as_deref(), Some("Cancelled"));
        assert!(cancelled.text_absent);
        assert_eq!(cancelled.decoder_started, 1);
        assert_eq!(cancelled.decoder_completed, 1);
        assert_eq!(cancelled.decoder_compared, 1);
        assert_eq!(cancelled.decoder_mismatches, 0);
        assert_eq!(
            cancelled.decoder_error.as_deref(),
            Some("Local transcription failed")
        );
        assert_eq!(report.rows[4].status, "unavailable");
        assert_eq!(diagnostic.started, 3);
        assert_eq!(diagnostic.completed, 3);
        assert!(!diagnostic.active);
        assert_eq!(
            diagnostic.last_error,
            Some((3, "Local transcription failed".into()))
        );
        Ok(())
    }
}
