//! Developer-only Runtime measurement. Never enables native capture or delivery.
use super::*;
pub mod job_trace;
use crate::audio::replay::ReplayInput;
use dictation_core::{audio::AudioFormat, session::Outcome};
use job_trace::JobTraceSnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasuredTerminal {
    Local,
    Empty,
    Failed,
    Cancelled,
    Shutdown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimingSnapshot {
    pub session: Option<u64>,
    pub t0: Option<Instant>,
    pub t_stop: Option<Instant>,
    pub t_text: Option<Instant>,
    pub t_terminal: Option<Instant>,
    pub decode_timing: Option<crate::inference_service::DecodeTiming>,
    pub terminal: Option<MeasuredTerminal>,
}
pub(super) struct Trace {
    value: Mutex<TimingSnapshot>,
    jobs: Option<Mutex<job_trace::Ledger>>,
    #[cfg(test)]
    on_text: Mutex<Option<Sender<Instant>>>,
}
impl Trace {
    fn new() -> Self {
        Self {
            value: Mutex::new(TimingSnapshot::default()),
            jobs: None,
            #[cfg(test)]
            on_text: Mutex::new(None),
        }
    }
    fn snapshot(&self) -> Result<TimingSnapshot, RuntimeError> {
        Ok(*self
            .value
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?)
    }
    pub(super) fn reset(&self, session: u64) -> Result<(), RuntimeError> {
        if let Some(jobs) = &self.jobs {
            jobs.lock()
                .map_err(|_| RuntimeError::Synchronization)?
                .reset(session);
        }
        *self
            .value
            .lock()
            .map_err(|_| RuntimeError::Synchronization)? = TimingSnapshot {
            session: Some(session),
            ..TimingSnapshot::default()
        };
        Ok(())
    }
    fn started(&self, session: u64, at: Instant) {
        let mut trace = self.value.lock().unwrap_or_else(|p| p.into_inner());
        if trace.session == Some(session) && trace.t0.is_none() {
            trace.t0 = Some(at);
        }
    }
    pub(super) fn accepted(
        &self,
        previous: &SessionSnapshot,
        next: &SessionSnapshot,
        candidate: bool,
        at: Instant,
        decode_timing: Option<crate::inference_service::DecodeTiming>,
    ) -> Result<(), RuntimeError> {
        let kept = matches!(next.phase, Phase::Recording | Phase::Finalizing)
            || (next.phase == Phase::Idle
                && matches!(next.outcome, Some(Outcome::Local | Outcome::Empty)));
        if candidate
            && kept
            && let Some(timing) = decode_timing
        {
            let mut trace = self
                .value
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            if trace.session == previous.session {
                trace.decode_timing = Some(match trace.decode_timing {
                    None => timing,
                    Some(old) => crate::inference_service::DecodeTiming {
                        elapsed: old
                            .elapsed
                            .checked_add(timing.elapsed)
                            .ok_or(RuntimeError::Synchronization)?,
                        input_frames: old
                            .input_frames
                            .checked_add(timing.input_frames)
                            .ok_or(RuntimeError::Synchronization)?,
                    },
                });
            }
        }
        if candidate
            && matches!(previous.phase, Phase::Recording | Phase::Finalizing)
            && next.phase == Phase::Idle
            && matches!(next.outcome, Some(Outcome::Local | Outcome::Empty))
        {
            let mut trace = self
                .value
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            if trace.session == previous.session && trace.t_terminal.is_none() {
                trace.t_terminal = Some(at);
                if next.outcome == Some(Outcome::Local) {
                    trace.t_text = Some(at);
                }
                #[cfg(test)]
                if next.outcome == Some(Outcome::Local)
                    && let Some(sender) = self
                        .on_text
                        .lock()
                        .map_err(|_| RuntimeError::Synchronization)?
                        .as_ref()
                    && sender.try_send(at).is_err()
                { /* test observer no longer waiting */
                }
            }
        }
        Ok(())
    }
    pub(super) fn terminal(&self, state: &SessionSnapshot) -> Result<(), RuntimeError> {
        if state.phase != Phase::Idle {
            return Ok(());
        }
        let mut trace = self
            .value
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        if trace.session.is_none() || trace.terminal.is_some() {
            return Ok(());
        }
        trace.terminal = match &state.outcome {
            Some(Outcome::Local) => Some(MeasuredTerminal::Local),
            Some(Outcome::Empty) => Some(MeasuredTerminal::Empty),
            Some(Outcome::Failed(_)) | Some(Outcome::Held(_)) => Some(MeasuredTerminal::Failed),
            Some(Outcome::Cancelled) => Some(MeasuredTerminal::Cancelled),
            _ => None,
        };
        Ok(())
    }
    pub(super) fn closed(&self) {
        let mut trace = self.value.lock().unwrap_or_else(|p| p.into_inner());
        if trace.session.is_some() && trace.terminal.is_none() {
            trace.terminal = Some(MeasuredTerminal::Shutdown);
        }
    }
}

pub(super) struct StartRequest {
    pub admission_epoch: u64,
    pub settings: SessionSettings,
    pub reply: oneshot::Sender<Result<u64, String>>,
}

struct ReplayAudio {
    owner: CpalAudio,
    format: AudioFormat,
    ready: Sender<ReplayInput>,
    pending_input: Receiver<ReplayInput>,
    trace: Arc<Trace>,
    native: NativeEndpoint,
    pretranscription: bool,
    full_prefix_revision: bool,
}
impl AudioDriver for ReplayAudio {
    fn start(&mut self, session: u64, _: Option<String>) -> Result<(), String> {
        if self.owner.active.is_some() {
            return Err("Replay owner is still occupied".into());
        }
        let handle = if self.full_prefix_revision {
            CaptureHandle::start_full_prefix_replay(session, self.format, self.ready.clone())
        } else if self.pretranscription {
            CaptureHandle::start_pretranscription_replay(session, self.format, self.ready.clone())
        } else {
            CaptureHandle::start_replay(session, self.format, self.ready.clone())
        }
        .map_err(|e| e.to_string())?;
        self.owner.active = Some((session, handle));
        Ok(())
    }
    fn stop(&self, session: u64) {
        self.owner.stop(session);
    }
    fn poll(&mut self) -> Vec<CaptureEvent> {
        // Only cancelled-delivery cleanup is possible through this wrapper.
        // Keep its bounded controlled endpoint drained without native effects.
        for _ in 0..8 {
            if self.native.requests.try_recv().is_err() {
                break;
            }
        }
        let events = self.owner.poll();
        for event in &events {
            if let CaptureEvent::Started {
                session, started, ..
            } = event
            {
                self.trace.started(*session, *started);
            }
        }
        events
    }
    fn take_progress(&mut self) -> Option<CaptureProgress> {
        self.owner.take_progress()
    }
    fn shutdown(&mut self) -> Result<(), String> {
        self.owner.shutdown()
    }
    fn pretranscription(&self) -> bool {
        self.pretranscription
    }
    fn full_prefix_revision(&self) -> bool {
        self.full_prefix_revision
    }
    fn allow_seal(&self, epoch: u64) {
        if let Some((_, handle)) = &self.owner.active {
            handle.allow_seal(epoch);
        }
    }
    fn measurement_trace(&self) -> Option<Arc<Trace>> {
        Some(Arc::clone(&self.trace))
    }
    fn prepare_measurement(&mut self) -> Result<(), String> {
        if self.owner.active.is_some() {
            return Err("Previous audio owner has not joined".into());
        }
        // The old owner has joined, so it cannot publish another stale port.
        // This channel has capacity exactly one. Drop that owned port now.
        match self.pending_input.try_recv() {
            Ok(old) => drop(old),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return Err("Replay readiness disconnected".into()),
        }
        if !self.pending_input.is_empty() {
            return Err("Replay readiness reset failed".into());
        }
        Ok(())
    }
}

/// Serial measurement facade. No Client/native endpoint is exposed: every start
/// is a local-only actor handshake, including readiness/reset of prior owners.
pub struct MeasuredRuntime {
    runtime: Runtime,
    input: Receiver<ReplayInput>,
    trace: Arc<Trace>,
    #[cfg(test)]
    stop_entry: Option<Sender<Instant>>,
}
/// Session-bound Stop endpoint without ownership of Runtime or its worker.
#[derive(Clone)]
pub struct MeasuredStopHandle {
    client: Client,
    trace: Arc<Trace>,
    session: u64,
    #[cfg(test)]
    stop_entry: Option<Sender<Instant>>,
}
impl MeasuredStopHandle {
    pub fn stop(&self) -> Result<Instant, RuntimeError> {
        let arrived = Instant::now(); // FIRST operation, before shared locks.
        stop_at(
            &self.client,
            &self.trace,
            self.session,
            arrived,
            #[cfg(test)]
            &self.stop_entry,
        )
    }
}

/// Evidence reader only: owns neither Runtime nor audio/model storage.
#[derive(Clone)]
pub struct MeasuredJobTrace {
    trace: Arc<Trace>,
}
impl MeasuredJobTrace {
    pub fn snapshot(&self) -> Result<JobTraceSnapshot, RuntimeError> {
        Ok(match &self.trace.jobs {
            None => JobTraceSnapshot::default(),
            Some(jobs) => jobs
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?
                .snapshot(),
        })
    }
}
impl MeasuredRuntime {
    /// Experimental whole-prefix revisions; never selected by app starts.
    /// Job ranges intentionally overlap. Decoder input-frame totals describe
    /// repeated work, not unique recording coverage; suffix validators and RTF
    /// denominators must not be reused for this mode.
    pub fn spawn_full_prefix_revision(
        inference: InferenceService,
        generation: u64,
        format: AudioFormat,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_inner(inference, generation, format, false, true, true)
    }
    /// Experimental confirmed-pause pretranscription; never selected by app starts.
    pub fn spawn_pretranscription(
        inference: InferenceService,
        generation: u64,
        format: AudioFormat,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_inner(inference, generation, format, true, false, true)
    }
    /// Full-recording control with exactly the same bounded job observations.
    pub fn spawn_pretranscription_reference(
        inference: InferenceService,
        generation: u64,
        format: AudioFormat,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_inner(inference, generation, format, false, false, true)
    }
    pub fn spawn(
        inference: InferenceService,
        generation: u64,
        format: AudioFormat,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_inner(inference, generation, format, false, false, false)
    }
    fn spawn_inner(
        inference: InferenceService,
        generation: u64,
        format: AudioFormat,
        pretranscription: bool,
        full_prefix_revision: bool,
        jobs: bool,
    ) -> Result<Self, RuntimeError> {
        format.validate().map_err(service_error)?;
        let (ready, input) = bounded(1);
        let mut trace = Trace::new();
        if jobs {
            trace.jobs = Some(Mutex::new(job_trace::Ledger::new()?));
        }
        let trace = Arc::new(trace);
        if jobs {
            inference
                .observe_jobs(trace.clone())
                .map_err(service_error)?;
        }
        let (native, endpoint) = native_mailbox();
        let audio = ReplayAudio {
            owner: CpalAudio::default(),
            format,
            ready,
            pending_input: input.clone(),
            trace: Arc::clone(&trace),
            native: endpoint,
            pretranscription,
            full_prefix_revision,
        };
        let runtime = Runtime::with_audio(inference, generation, native, audio)?;
        Ok(Self {
            runtime,
            input,
            trace,
            #[cfg(test)]
            stop_entry: None,
        })
    }
    pub fn start(
        &self,
        settings: SessionSettings,
    ) -> Result<oneshot::Receiver<Result<u64, String>>, MailboxError> {
        let client = &self.runtime.client;
        let shared = client
            .shared
            .lock()
            .map_err(|_| MailboxError::Synchronization)?;
        if shared.closing {
            return Err(MailboxError::Closed);
        }
        if shared.suspended {
            return Err(MailboxError::Suspended);
        }
        let (reply, receive) = oneshot::channel();
        client
            .control
            .request(Request::MeasuredStart(StartRequest {
                admission_epoch: shared.admission_epoch,
                settings,
                reply,
            }))?;
        Ok(receive)
    }
    pub fn take_input(&self, timeout: Duration) -> Result<ReplayInput, RuntimeError> {
        let id = self
            .trace
            .snapshot()?
            .session
            .ok_or_else(|| RuntimeError::Service("No accepted measured session".into()))?;
        let input = self.input.recv_timeout(timeout).map_err(|_| {
            RuntimeError::Service("Replay input unavailable before deadline".into())
        })?;
        let state = self.snapshot()?;
        if self.trace.snapshot()?.session != Some(id)
            || state.session.session != Some(id)
            || !matches!(state.session.phase, Phase::Starting | Phase::Recording)
        {
            return Err(RuntimeError::Service(
                "Replay input belongs to an ended measurement".into(),
            ));
        }
        Ok(input)
    }
    pub fn job_trace_handle(&self) -> MeasuredJobTrace {
        MeasuredJobTrace {
            trace: Arc::clone(&self.trace),
        }
    }
    pub fn job_trace(&self) -> Result<JobTraceSnapshot, RuntimeError> {
        self.job_trace_handle().snapshot()
    }
    pub fn stop_handle(&self, session: u64) -> MeasuredStopHandle {
        MeasuredStopHandle {
            client: self.runtime.client.clone(),
            trace: Arc::clone(&self.trace),
            session,
            #[cfg(test)]
            stop_entry: self.stop_entry.clone(),
        }
    }
    pub fn stop(&self, session: u64) -> Result<Instant, RuntimeError> {
        let arrived = Instant::now(); // FIRST operation: before any lock or queue.
        stop_at(
            &self.runtime.client,
            &self.trace,
            session,
            arrived,
            #[cfg(test)]
            &self.stop_entry,
        )
    }
    pub fn cancel(&self, session: u64) -> Result<(), RuntimeError> {
        self.runtime.client.cancel(session)
    }
    pub fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        self.runtime.client.snapshot()
    }
    pub fn timing(&self) -> Result<TimingSnapshot, RuntimeError> {
        self.trace.snapshot()
    }
    pub fn shutdown(&self) -> Result<(), RuntimeError> {
        self.runtime.client.shutdown()
    }
    pub fn join(self) -> Result<(), RuntimeError> {
        self.runtime.join()
    }
}

fn stop_at(
    client: &Client,
    trace: &Trace,
    session: u64,
    arrived: Instant,
    #[cfg(test)] stop_entry: &Option<Sender<Instant>>,
) -> Result<Instant, RuntimeError> {
    #[cfg(test)]
    if let Some(sender) = stop_entry
        && sender.try_send(arrived).is_err()
    { /* test observer no longer waiting */ }
    let shared = client
        .shared
        .lock()
        .map_err(|_| RuntimeError::Synchronization)?;
    if !control_targets_current(&shared, session)? {
        return Err(RuntimeError::Service(
            "Stop does not target the current measurement".into(),
        ));
    }
    {
        let mut trace = trace
            .value
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        if trace.session != Some(session) || trace.terminal.is_some() {
            return Err(RuntimeError::Service("Measurement already ended".into()));
        }
        if trace.t_stop.is_none() {
            trace.t_stop = Some(arrived);
        }
    }
    client.control.stop(session)?;
    Ok(arrived)
}

impl<A: AudioDriver> Actor<A> {
    pub(super) fn start_measured(
        &mut self,
        epoch: u64,
        settings: SessionSettings,
    ) -> Result<u64, RuntimeError> {
        let shared = Arc::clone(&self.shared);
        let mut admission = shared.lock().map_err(|_| RuntimeError::Synchronization)?;
        if admission.closing {
            return Err(MailboxError::Closed.into());
        }
        if admission.suspended {
            return Err(RuntimeError::Suspended);
        }
        if epoch != admission.admission_epoch {
            return Err(RuntimeError::StaleAdmission);
        }
        let service = self
            .inference
            .as_ref()
            .ok_or_else(|| RuntimeError::Service("Model unavailable".into()))?;
        if !service.ready() || settings.generation != self.generation {
            return Err(RuntimeError::Service(
                "Warm model generation unavailable".into(),
            ));
        }
        if self.controller.snapshot().phase != Phase::Idle
            || service.busy().map_err(service_error)?
        {
            return Err(RuntimeError::Service(
                "Previous measurement or inference owner is still busy".into(),
            ));
        }
        self.audio
            .prepare_measurement()
            .map_err(RuntimeError::Service)?;
        drop(service.take_result().map_err(service_error)?);
        let trace = admission
            .timing
            .as_ref()
            .ok_or(RuntimeError::Synchronization)?;
        self.controller.clear_text();
        let actions = self
            .controller
            .start(settings, None, true, false)
            .map_err(service_error)?;
        let id = self
            .controller
            .snapshot()
            .session
            .ok_or(RuntimeError::Synchronization)?;
        trace.reset(id)?;
        admission.cancellation = self.controller.cancellation_handles();
        drop(admission);
        self.error = None;
        self.maximum_warning = false;
        self.maximum_reached = false;
        self.progress = None;
        self.execute(actions)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    #[path = "full_prefix_tests.rs"]
    mod full_prefix_tests;
    #[path = "pretranscription_tests.rs"]
    mod pretranscription_tests;
    #[path = "measurement_stop_tests.rs"]
    mod stop_handle_tests;

    use super::*;
    use crate::{
        inference_service::Decoder,
        transcription::{InferenceError, Language},
    };
    use std::thread;
    struct Words;
    impl Decoder for Words {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            Ok("accepted final text".into())
        }
    }
    fn await_value<T>(mut reply: oneshot::Receiver<T>) -> Result<T, Box<dyn std::error::Error>> {
        let limit = Instant::now() + Duration::from_secs(2);
        loop {
            match reply.try_recv() {
                Ok(value) => return Ok(value),
                Err(oneshot::error::TryRecvError::Closed) => {
                    return Err("acknowledgement closed".into());
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
            if Instant::now() > limit {
                return Err("acknowledgement timed out".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn settings() -> SessionSettings {
        SessionSettings {
            device: None,
            language: Language::German,
            generation: 1,
        }
    }
    fn wait_state(
        runtime: &MeasuredRuntime,
        predicate: impl Fn(&RuntimeSnapshot) -> bool,
    ) -> Result<RuntimeSnapshot, Box<dyn std::error::Error>> {
        let limit = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = runtime.snapshot()?;
            if predicate(&snapshot) {
                return Ok(snapshot);
            }
            if Instant::now() > limit {
                return Err("runtime state timed out".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn finish(runtime: &MeasuredRuntime) -> Result<TimingSnapshot, Box<dyn std::error::Error>> {
        let id = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        assert_eq!(runtime.timing()?.session, Some(id));
        assert_eq!(runtime.timing()?.t_stop, None);
        assert_eq!(runtime.timing()?.t_text, None);
        assert_eq!(runtime.timing()?.t_terminal, None);
        assert_eq!(runtime.timing()?.decode_timing, None);
        assert_eq!(runtime.snapshot()?.session.text, None);
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        input.port.publish(&[0.1f32; 3200]);
        runtime.stop(id)?;
        wait_state(runtime, |state| state.session.phase == Phase::Idle)?;
        Ok(runtime.timing()?)
    }
    #[test]
    fn real_replay_owner_finishes_local_text_with_internal_times_before_observation()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Words)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        let (text_time, at_adoption) = bounded(1);
        *runtime
            .trace
            .on_text
            .lock()
            .map_err(|_| "test observer poisoned")? = Some(text_time);
        let limit = Instant::now() + Duration::from_secs(2);
        while !runtime.snapshot()?.model_ready {
            if Instant::now() > limit {
                return Err("model not ready".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        let id = await_value(runtime.start(SessionSettings {
            device: None,
            language: Language::German,
            generation: 1,
        })?)?
        .map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        input.port.publish(&[0.1f32; 3200]);
        let stop = runtime.stop(id)?;
        // No snapshot/timing polling between Stop and actual adoption. The
        // test-only notifier carries the Instant captured at that boundary.
        let adopted = at_adoption.recv_timeout(Duration::from_secs(2))?;
        thread::sleep(Duration::from_millis(20));
        let times = runtime.timing()?;
        assert_eq!(times.session, Some(id));
        assert_eq!(times.t0, Some(input.started));
        assert_eq!(times.t_stop, Some(stop));
        assert_eq!(times.t_text, Some(adopted));
        assert_eq!(times.t_terminal, Some(adopted));
        assert_eq!(
            times
                .decode_timing
                .ok_or("missing accepted timing")?
                .input_frames,
            3200
        );
        assert_eq!(times.terminal, Some(MeasuredTerminal::Local));
        assert!(
            times
                .t_text
                .is_some_and(|time| time >= stop && time <= Instant::now())
        );
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn warm_reuse_clears_retained_text_and_empty_or_error_runs_have_no_text_time()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Sequence(VecDeque<Result<String, InferenceError>>);
        impl Decoder for Sequence {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                self.0.pop_front().ok_or(InferenceError::Decode)?
            }
        }
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Sequence(VecDeque::from([
                Ok("first run text".into()),
                Ok(String::new()),
                Err(InferenceError::Decode),
            ])))?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_state(&runtime, |s| s.model_ready)?;
        let first = finish(&runtime)?;
        let empty = finish(&runtime)?;
        let failed = finish(&runtime)?;
        assert_eq!(first.terminal, Some(MeasuredTerminal::Local));
        assert!(first.t_text.is_some());
        assert_eq!(
            first
                .decode_timing
                .ok_or("accepted Engine timing missing")?
                .input_frames,
            3200
        );
        assert_eq!(
            empty
                .decode_timing
                .ok_or("accepted empty Engine timing missing")?
                .input_frames,
            3200
        );
        assert_eq!(failed.decode_timing, None);
        assert_eq!(empty.terminal, Some(MeasuredTerminal::Empty));
        assert_eq!(empty.t_text, None);
        assert!(empty.t_terminal.is_some());
        assert_eq!(failed.t_terminal, None);
        assert_eq!(failed.terminal, Some(MeasuredTerminal::Failed));
        assert_eq!(failed.t_text, None);
        assert!(first.session < empty.session && empty.session < failed.session);
        assert_eq!(runtime.snapshot()?.session.text, None);
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn cancelled_unconsumed_ready_port_is_removed_before_a_new_run()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Words)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_state(&runtime, |s| s.model_ready)?;
        let first = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        wait_state(&runtime, |s| s.session.phase == Phase::Recording)?;
        let old_start = runtime.timing()?.t0;
        runtime.cancel(first)?;
        wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(
            runtime.timing()?.terminal,
            Some(MeasuredTerminal::Cancelled)
        );
        assert_eq!(runtime.timing()?.t_text, None);
        let second = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        assert!(old_start.is_some_and(|old| old < input.started));
        input.port.publish(&[0.1f32; 3200]);
        runtime.stop(second)?;
        wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(runtime.timing()?.session, Some(second));
        assert_eq!(runtime.timing()?.terminal, Some(MeasuredTerminal::Local));
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn stop_captures_host_ingress_before_a_contended_admission_lock()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Words)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_state(&runtime, |s| s.model_ready)?;
        let id = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        input.port.publish(&[0.1f32; 3200]);
        let (entered, at_ingress) = bounded(1);
        runtime.stop_entry = Some(entered);
        let runtime = Arc::new(runtime);
        let lock = runtime
            .runtime
            .client
            .shared
            .lock()
            .map_err(|_| "admission poisoned")?;
        let owner = Arc::clone(&runtime);
        let (sent, done) = bounded(1);
        let caller = thread::spawn(move || {
            if sent.send(owner.stop(id)).is_err() { /* test caller gone */ }
        });
        let ingress = at_ingress.recv_timeout(Duration::from_secs(2))?;
        let early = done.recv_timeout(Duration::from_millis(20));
        let before_unlock = Instant::now();
        drop(lock);
        assert!(matches!(
            early,
            Err(crossbeam_channel::RecvTimeoutError::Timeout)
        ));
        let returned = done.recv_timeout(Duration::from_secs(2))??;
        caller.join().map_err(|_| "stop caller panicked")?;
        assert_eq!(ingress, returned);
        assert!(returned < before_unlock);
        wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(runtime.timing()?.t_stop, Some(ingress));
        let runtime = Arc::try_unwrap(runtime).map_err(|_| "outstanding measured owner")?;
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn cancelled_idle_core_is_not_ready_while_old_decoder_is_busy_and_old_result_cannot_stamp_new_run()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Held {
            entered: Sender<()>,
            release: Receiver<()>,
            calls: usize,
        }
        impl Decoder for Held {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                self.calls += 1;
                if self.calls == 1 {
                    self.entered
                        .try_send(())
                        .map_err(|_| InferenceError::Decode)?;
                    self.release
                        .recv_timeout(Duration::from_secs(5))
                        .map_err(|_| InferenceError::Decode)?;
                    Ok("obsolete cancelled text".into())
                } else {
                    Ok("new final text".into())
                }
            }
        }
        struct Release(Sender<()>);
        impl Drop for Release {
            fn drop(&mut self) {
                if self.0.try_send(()).is_err() { /* already released */ }
            }
        }
        let (entered, decoding) = bounded(1);
        let (release, resume) = bounded(1);
        let _release = Release(release.clone());
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Held {
                entered,
                release: resume,
                calls: 0,
            })?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_state(&runtime, |s| s.model_ready)?;
        let first = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        input.port.publish(&[0.1f32; 3200]);
        runtime.stop(first)?;
        decoding.recv_timeout(Duration::from_secs(2))?;
        runtime.cancel(first)?;
        wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
        let cancelled = runtime.timing()?;
        assert_eq!(cancelled.terminal, Some(MeasuredTerminal::Cancelled));
        assert_eq!(cancelled.t_text, None);
        assert_eq!(cancelled.t_terminal, None);
        assert_eq!(cancelled.decode_timing, None);
        let rejected = await_value(runtime.start(settings())?)?;
        assert!(rejected.is_err_and(|message| message.contains("busy")));
        assert_eq!(
            runtime.timing()?,
            cancelled,
            "Busy must not reset the preceding trace"
        );
        release.try_send(())?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let second = loop {
            match await_value(runtime.start(settings())?)? {
                Ok(id) => break id,
                Err(error) if error.contains("busy") && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(error) => return Err(std::io::Error::other(error).into()),
            }
        };
        assert!(second > first);
        assert_eq!(runtime.timing()?.t_stop, None);
        assert_eq!(runtime.timing()?.t_text, None);
        assert_eq!(runtime.snapshot()?.session.text, None);
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        input.port.publish(&[0.1f32; 3200]);
        assert!(runtime.stop(first).is_err());
        assert_eq!(runtime.timing()?.t_stop, None);
        runtime.stop(second)?;
        wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(runtime.timing()?.terminal, Some(MeasuredTerminal::Local));
        assert!(runtime.timing()?.t_text.is_some());
        assert_eq!(
            runtime.snapshot()?.session.text.as_deref(),
            Some("new final text")
        );
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn wrong_generation_and_foreign_stop_never_create_measurement_times()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Words)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_state(&runtime, |s| s.model_ready)?;
        let mut wrong = settings();
        wrong.generation = 2;
        assert!(await_value(runtime.start(wrong)?)?.is_err());
        assert_eq!(runtime.timing()?, TimingSnapshot::default());
        assert!(runtime.stop(999).is_err());
        assert_eq!(runtime.timing()?, TimingSnapshot::default());
        runtime.join()?;
        Ok(())
    }

    // InferenceService supplies trusted tags; inject malformed completions only
    // at the actual Actor->Controller adoption boundary, without a second model
    // of its validation rules or an exposed production injection API.
    #[test]
    fn rejected_completion_tags_do_not_stamp_text_before_real_controller_adoption()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = Controller::default();
        controller.start(settings(), None, true, false)?;
        controller.accept(Event::AudioStarted(1))?;
        controller.stop(1)?;
        let actions = controller.accept(Event::AudioFinished {
            session: 1,
            result: Ok(vec![0.1; 3200]),
        })?;
        let job = actions
            .into_iter()
            .find_map(|action| match action {
                Action::Transcribe(job) => Some(job),
                _ => None,
            })
            .ok_or("expected actual inference job")?;
        let trace = Arc::new(Trace::new());
        trace.reset(1)?;
        let cancellation = controller.cancellation_handles();
        let shared = Arc::new(Mutex::new(Shared {
            closing: false,
            suspended: false,
            admission_epoch: 0,
            snapshot: RuntimeSnapshot {
                revision: 1,
                running: true,
                model_ready: true,
                session: controller.snapshot(),
                maximum_warning: false,
                maximum_reached: false,
                elapsed_millis: 0,
                level_permille: 0,
                error: None,
            },
            cancellation,
            timing: Some(Arc::clone(&trace)),
        }));
        let (native, _endpoint) = native_mailbox();
        let mut actor = Actor {
            controller,
            inference: None,
            generation: 1,
            audio: CpalAudio::default(),
            native,
            shared,
            maximum_warning: false,
            maximum_reached: false,
            progress: None,
            error: None,
            capture_pending: None,
            deferred_audio: None,
        };
        for (session, generation, job_id, sample_range) in [
            (2, 1, 1, job.sample_range.clone()),
            (1, 2, 1, job.sample_range.clone()),
            (1, 1, 2, job.sample_range.clone()),
            (1, 1, 1, 1..3200),
        ] {
            actor.accept_with_timing(
                Event::InferenceFinished {
                    session,
                    generation,
                    job_id,
                    timeline_range: sample_range.clone(),
                    sample_range,
                    result: Ok("untrusted completion".into()),
                },
                Some(crate::inference_service::DecodeTiming {
                    elapsed: Duration::from_millis(50),
                    input_frames: 3200,
                }),
            )?;
            assert_eq!(actor.controller.snapshot().phase, Phase::Finalizing);
            assert_eq!(trace.snapshot()?.t_text, None);
            assert_eq!(trace.snapshot()?.t_terminal, None);
            assert_eq!(trace.snapshot()?.decode_timing, None);
        }
        actor.accept_with_timing(
            Event::InferenceFinished {
                session: 1,
                generation: 1,
                job_id: 1,
                sample_range: job.sample_range,
                timeline_range: job.timeline_range,
                result: Ok("accepted completion".into()),
            },
            Some(crate::inference_service::DecodeTiming {
                elapsed: Duration::from_millis(50),
                input_frames: 3200,
            }),
        )?;
        assert_eq!(actor.controller.snapshot().outcome, Some(Outcome::Local));
        let adopted = trace.snapshot()?.t_text.ok_or("missing adoption time")?;
        // Deliberate acceptance/publication gap: observers must not see a
        // terminal enum until the shared controller snapshot is published.
        assert_eq!(trace.snapshot()?.terminal, None);
        assert_eq!(
            actor
                .shared
                .lock()
                .map_err(|_| "poisoned")?
                .snapshot
                .session
                .phase,
            Phase::Finalizing
        );
        actor.publish()?;
        assert_eq!(trace.snapshot()?.terminal, Some(MeasuredTerminal::Local));
        assert_eq!(
            actor
                .shared
                .lock()
                .map_err(|_| "poisoned")?
                .snapshot
                .session
                .text
                .as_deref(),
            Some("accepted completion")
        );
        actor.accept_with_timing(
            Event::InferenceFinished {
                session: 1,
                generation: 1,
                job_id: 1,
                sample_range: 0..3200,
                timeline_range: 0..3200,
                result: Ok("duplicate completion".into()),
            },
            Some(crate::inference_service::DecodeTiming {
                elapsed: Duration::from_millis(999),
                input_frames: 3200,
            }),
        )?;
        assert_eq!(trace.snapshot()?.t_text, Some(adopted));
        assert_eq!(trace.snapshot()?.t_terminal, Some(adopted));
        assert_eq!(
            trace.snapshot()?.decode_timing,
            Some(crate::inference_service::DecodeTiming {
                elapsed: Duration::from_millis(50),
                input_frames: 3200
            })
        );
        Ok(())
    }
}
