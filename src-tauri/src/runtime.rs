//! Serial host actor connecting capture, inference and the main-thread bridge.
use crate::{
    audio::{CaptureEvent, CaptureHandle, CaptureProgress},
    control::{self, Control, ControlTx, MailboxError},
    controller::{Action, Controller, Event, SessionSettings, SessionSnapshot},
    inference_service::{InferenceService, WorkToken},
};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use dictation_core::session::{DeliveryResult, Phase};
use macos_integration::{
    delivery::{CancelOutcome, DeliveryToken},
    target::ProcessIdentity,
};
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

#[cfg(feature = "measurement")]
pub mod measurement;

pub enum NativeCommand {
    Capture {
        session: NonZeroU64,
        process: Option<ProcessIdentity>,
        token: DeliveryToken,
    },
    Deliver {
        session: u64,
        text: String,
        token: DeliveryToken,
    },
    Cancel(u64),
}
pub enum NativeResult {
    Captured {
        session: u64,
        result: Result<(), String>,
    },
    Delivered {
        session: u64,
        result: DeliveryResult,
    },
}
pub struct NativePort {
    outgoing: Sender<NativeCommand>,
    incoming: Receiver<NativeResult>,
}
pub struct NativeEndpoint {
    pub requests: Receiver<NativeCommand>,
    pub responses: Sender<NativeResult>,
}

pub fn native_mailbox() -> (NativePort, NativeEndpoint) {
    let (outgoing, requests) = bounded(8);
    let (responses, incoming) = bounded(8);
    (
        NativePort { outgoing, incoming },
        NativeEndpoint {
            requests,
            responses,
        },
    )
}

enum Request {
    Start(StartRequest),
    Clear(oneshot::Sender<Result<(), String>>),
    #[cfg(feature = "measurement")]
    MeasuredStart(measurement::StartRequest),
}

struct StartRequest {
    admission_epoch: u64,
    settings: SessionSettings,
    process: Option<ProcessIdentity>,
    delivery: bool,
    toggle: bool,
    reply: oneshot::Sender<Result<(), String>>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub running: bool,
    pub model_ready: bool,
    pub session: SessionSnapshot,
    pub maximum_warning: bool,
    pub maximum_reached: bool,
    pub elapsed_millis: u64,
    pub level_permille: u16,
    pub error: Option<String>,
}

struct Shared {
    closing: bool,
    suspended: bool,
    admission_epoch: u64,
    snapshot: RuntimeSnapshot,
    cancellation: Option<(u64, DeliveryToken, WorkToken)>,
    #[cfg(feature = "measurement")]
    timing: Option<Arc<measurement::Trace>>,
}

fn close_admission(shared: &Mutex<Shared>) {
    let mut shared = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    shared.closing = true;
    if let Some((_, delivery, work)) = &shared.cancellation {
        delivery.cancel();
        work.cancel();
    }
}

struct RuntimeExit<'a> {
    shared: Arc<Mutex<Shared>>,
    requests: &'a crate::control::ControlRx<Request>,
}
impl Drop for RuntimeExit<'_> {
    fn drop(&mut self) {
        close_admission(&self.shared);
        {
            // Even an unwinding controller must revoke admission and settle
            // acknowledgements. Recovery here is teardown only.
            let mut shared = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some((_, delivery, work)) = shared.cancellation.take() {
                delivery.cancel();
                work.cancel();
            }
            shared.closing = true;
            shared.snapshot.running = false;
            shared.snapshot.model_ready = false;
            shared.snapshot.elapsed_millis = 0;
            shared.snapshot.level_permille = 0;
            shared.snapshot.revision = shared.snapshot.revision.saturating_add(1);
            #[cfg(feature = "measurement")]
            if let Some(trace) = &shared.timing {
                trace.closed();
            }
        }
        reject_pending(self.requests);
    }
}

fn reject_pending(requests: &crate::control::ControlRx<Request>) {
    for request in requests.close_requests() {
        let disconnected = match request {
            Request::Start(request) => request
                .reply
                .send(Err("Controller is closed".into()))
                .is_err(),
            Request::Clear(reply) => reply.send(Err("Controller is closed".into())).is_err(),
            #[cfg(feature = "measurement")]
            Request::MeasuredStart(request) => request
                .reply
                .send(Err("Controller is closed".into()))
                .is_err(),
        };
        if disconnected { /* caller already closed */ }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Mailbox(#[from] MailboxError),
    #[error("Controller synchronization failed")]
    Synchronization,
    #[error("Controller worker could not start or join")]
    Worker,
    #[error("Controller service failed: {0}")]
    Service(String),
    #[error("Recording is suspended")]
    Suspended,
    #[error("Recording request predates system sleep")]
    StaleAdmission,
    #[error("Recording admission identifiers exhausted; restart the application")]
    AdmissionExhausted,
}

fn control_targets_current(shared: &Shared, session: u64) -> Result<bool, MailboxError> {
    if shared.closing {
        return Err(MailboxError::Closed);
    }
    if session == 0 || session > (u64::MAX >> 2) {
        return Err(MailboxError::InvalidSession);
    }
    Ok(shared
        .cancellation
        .as_ref()
        .is_some_and(|(active, _, _)| *active == session))
}

#[derive(Clone)]
pub struct Client {
    control: ControlTx<Request>,
    shared: Arc<Mutex<Shared>>,
}

impl Client {
    /// Runs a bounded feedback action only while capture admission is idle.
    /// The callback must not block or re-enter Runtime. Its lock also gates
    /// accepted starts and native audio dispatch, so feedback cannot race them.
    pub fn with_idle_feedback<R>(
        &self,
        held_session: u64,
        action: impl FnOnce() -> R,
    ) -> Result<Option<R>, RuntimeError> {
        let shared = self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        if shared.closing
            || shared.suspended
            || shared.cancellation.is_some()
            || shared.snapshot.session.phase != Phase::Idle
            || shared.snapshot.session.last_held_session != Some(held_session)
        {
            return Ok(None);
        }
        // Retain admission through the effect; a new start can only proceed
        // after it returns. Snapshot alone could still describe an older idle.
        let result = action();
        drop(shared);
        Ok(Some(result))
    }
    pub fn start(
        &self,
        settings: SessionSettings,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<oneshot::Receiver<Result<(), String>>, MailboxError> {
        let admission = self
            .shared
            .lock()
            .map_err(|_| MailboxError::Synchronization)?;
        if admission.closing {
            return Err(MailboxError::Closed);
        }
        if admission.suspended {
            return Err(MailboxError::Suspended);
        }
        let (reply, receiver) = oneshot::channel();
        self.control.request(Request::Start(StartRequest {
            admission_epoch: admission.admission_epoch,
            settings,
            process,
            delivery,
            toggle,
            reply,
        }))?;
        Ok(receiver)
    }
    pub fn clear_text(&self) -> Result<oneshot::Receiver<Result<(), String>>, MailboxError> {
        let (reply, receive) = oneshot::channel();
        self.control.request(Request::Clear(reply))?;
        Ok(receive)
    }
    pub fn stop(&self, session: u64) -> Result<(), MailboxError> {
        let admission = self
            .shared
            .lock()
            .map_err(|_| MailboxError::Synchronization)?;
        if !control_targets_current(&admission, session)? {
            return Ok(());
        }
        // Keep identity validation and latch publication ordered against the
        // actor publishing a new session. Foreign future IDs cannot poison it.
        let result = self.control.stop(session);
        drop(admission);
        result
    }
    pub fn cancel(&self, session: u64) -> Result<(), RuntimeError> {
        // Revoke before scheduling: native send cannot outrun a queued cancel.
        let shared = self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        if !control_targets_current(&shared, session)? {
            return Ok(());
        }
        if let Some((_, delivery, work)) = &shared.cancellation {
            delivery.cancel();
            work.cancel();
        }
        let result = self.control.cancel(session).map_err(RuntimeError::from);
        drop(shared);
        result
    }
    /// Invalidate queued starts and active work immediately. Audio ownership is
    /// released by the controller; this notification callback never joins a worker.
    pub fn suspend(&self) -> Result<(), RuntimeError> {
        let (session, exhausted) = {
            let mut shared = self
                .shared
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            if shared.closing {
                return Err(MailboxError::Closed.into());
            }
            let mut exhausted = false;
            if !shared.suspended {
                match shared.admission_epoch.checked_add(1) {
                    Some(epoch) => shared.admission_epoch = epoch,
                    None => {
                        shared.closing = true;
                        exhausted = true;
                    }
                }
                shared.suspended = true;
            }
            let session = shared
                .cancellation
                .as_ref()
                .and_then(|(session, delivery, work)| {
                    let committed = delivery.cancel() == CancelOutcome::AlreadyCommitted;
                    work.cancel();
                    // Lifecycle cleanup cannot retract a committed send. Wake
                    // the actor to settle it, without reporting a user-cancel error.
                    (!committed).then_some(*session)
                });
            (session, exhausted)
        };
        if exhausted {
            self.control.shutdown()?;
            return Err(RuntimeError::AdmissionExhausted);
        }
        if let Some(session) = session {
            self.control.cancel(session)?;
        } else {
            self.control.notify()?;
        }
        Ok(())
    }
    /// Re-enable fresh explicit starts. Old admissions and revoked work remain invalid.
    pub fn resume(&self) -> Result<(), RuntimeError> {
        {
            let mut shared = self
                .shared
                .lock()
                .map_err(|_| RuntimeError::Synchronization)?;
            if shared.closing {
                return Err(MailboxError::Closed.into());
            }
            shared.suspended = false;
        }
        self.control.notify()?;
        Ok(())
    }
    pub fn is_suspended(&self) -> Result<bool, RuntimeError> {
        Ok(self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?
            .suspended)
    }
    pub fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        Ok(self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?
            .snapshot
            .clone())
    }
    pub fn shutdown(&self) -> Result<(), RuntimeError> {
        let revoked = self
            .shared
            .lock()
            .map(|mut shared| {
                shared.closing = true;
                if let Some((_, delivery, work)) = &shared.cancellation {
                    delivery.cancel();
                    work.cancel();
                }
            })
            .map_err(|_| RuntimeError::Synchronization);
        let requested = self.control.shutdown().map_err(RuntimeError::from);
        revoked.and(requested)
    }
}

trait AudioDriver: Send + 'static {
    fn start(&mut self, session: u64, device: Option<String>) -> Result<(), String>;
    fn stop(&self, session: u64);
    fn poll(&mut self) -> Vec<CaptureEvent>;
    fn take_progress(&mut self) -> Option<CaptureProgress> {
        None
    }
    fn shutdown(&mut self) -> Result<(), String>;
    #[cfg(feature = "measurement")]
    fn pretranscription(&self) -> bool {
        false
    }
    #[cfg(feature = "measurement")]
    fn allow_seal(&self, _epoch: u64) {}
    #[cfg(feature = "measurement")]
    fn measurement_trace(&self) -> Option<Arc<measurement::Trace>> {
        None
    }
    #[cfg(feature = "measurement")]
    fn prepare_measurement(&mut self) -> Result<(), String> {
        Err("Audio source does not support measurement reset".into())
    }
}

#[derive(Default)]
struct CpalAudio {
    active: Option<(u64, CaptureHandle)>,
}
impl AudioDriver for CpalAudio {
    fn take_progress(&mut self) -> Option<CaptureProgress> {
        self.active
            .as_ref()
            .and_then(|(_, handle)| handle.take_progress())
    }
    fn start(&mut self, session: u64, device: Option<String>) -> Result<(), String> {
        if self.active.is_some() {
            return Err("Microphone is still occupied".into());
        }
        self.active = Some((
            session,
            CaptureHandle::start(session, device).map_err(|error| error.to_string())?,
        ));
        Ok(())
    }
    fn stop(&self, session: u64) {
        if let Some((active, handle)) = &self.active
            && *active == session
        {
            handle.request_stop();
        }
    }
    fn poll(&mut self) -> Vec<CaptureEvent> {
        let mut events = Vec::new();
        let Some((session, handle)) = &self.active else {
            return events;
        };
        let session = *session;
        while let Some(event) = handle.next_event() {
            events.push(event);
        }
        let finished = events.iter().any(|event| event.is_finished());
        if finished || handle.is_finished() {
            // Close the drain/is_finished race: Finished may have been sent
            // after the first drain but before is_finished became true.
            while let Some(event) = handle.next_event() {
                events.push(event);
            }
            let finished = events.iter().any(|event| event.is_finished());
            let Some((_, handle)) = self.active.take() else {
                return events;
            };
            let joined = handle.join();
            if !finished {
                events.push(CaptureEvent::Finished {
                    session,
                    result: Err(joined.err().unwrap_or(crate::audio::CaptureError::Worker)),
                    maximum: false,
                });
            } else if let Err(error) = joined {
                for event in &mut events {
                    if let Some(result) = event.result_mut() {
                        *result = Err(error);
                        break;
                    }
                }
            }
        }
        events
    }
    fn shutdown(&mut self) -> Result<(), String> {
        if let Some((_, handle)) = self.active.take() {
            handle.request_stop();
            handle.join().map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

pub struct Runtime {
    client: Client,
    worker: Option<JoinHandle<Result<(), RuntimeError>>>,
}

impl Runtime {
    pub fn spawn(
        inference: InferenceService,
        generation: u64,
        native: NativePort,
    ) -> Result<Self, RuntimeError> {
        Self::with_audio(inference, generation, native, CpalAudio::default())
    }
    fn with_audio(
        inference: InferenceService,
        generation: u64,
        native: NativePort,
        audio: impl AudioDriver,
    ) -> Result<Self, RuntimeError> {
        let controller = Controller::default();
        #[cfg(feature = "measurement")]
        let controller = {
            let mut controller = controller;
            if audio.pretranscription() {
                controller.enable_pretranscription();
            }
            controller
        };
        #[cfg(feature = "measurement")]
        let timing = audio.measurement_trace();
        let shared = Arc::new(Mutex::new(Shared {
            closing: false,
            suspended: false,
            admission_epoch: 0,
            snapshot: RuntimeSnapshot {
                revision: 1,
                running: true,
                model_ready: false,
                session: controller.snapshot(),
                maximum_warning: false,
                maximum_reached: false,
                elapsed_millis: 0,
                level_permille: 0,
                error: None,
            },
            cancellation: None,
            #[cfg(feature = "measurement")]
            timing,
        }));
        let (control, requests) = control::mailbox(16);
        let client = Client {
            control,
            shared: Arc::clone(&shared),
        };
        let worker = thread::Builder::new()
            .name("dictation-controller".into())
            .spawn(move || {
                let _exit = RuntimeExit { shared: Arc::clone(&shared), requests: &requests };
                let mut actor = Actor {
                    controller,
                    inference: Some(inference),
                    generation,
                    audio,
                    native,
                    shared,
                    maximum_warning: false,
                maximum_reached: false,
                    progress: None,
                    error: None,
                    capture_pending: None,
                    deferred_audio: None,
                };
                // Contain Rust controller panics on this owner thread while
                // retaining the actor for explicit owner shutdown below. No
                // partially mutated controller state is used to resume work.
                let result = catch_unwind(AssertUnwindSafe(|| { loop {
                    actor.reconcile_revoked()?;
                    actor.poll()?;
                    actor.publish()?;
                    match requests.wait(Duration::from_millis(10)) {
                        Some(Control::Shutdown) => {
                            if let Some(session) = actor.controller.snapshot().session {
                                match actor.controller.cancel(session) {
                                    Ok(actions) => actor.execute(actions)?,
                                    Err(crate::controller::ControllerError::Session(dictation_core::session::SessionError::DeliveryCommitted)) => {},
                                    Err(error) => return Err(service_error(error)),
                                }
                            }
                            // All joins happen after close_admission/reject_pending below.
                            return Ok(());
                        }
                        Some(Control::Stop(session)) => {
                            let actions = actor.controller.stop(session).map_err(service_error)?;
                            actor.execute(actions)?;
                        }
                        Some(Control::Cancel(session)) => match actor.controller.cancel(session) {
                            Ok(actions) => actor.execute(actions)?,
                            Err(error) => actor.error = Some(error.to_string()),
                        },
                        Some(Control::Request(Request::Clear(reply))) => {
                            actor.controller.clear_text();
                            actor.publish()?;
                            if reply.send(Ok(())).is_err() { /* requester no longer waiting */ }
                        }
                        #[cfg(feature = "measurement")]
                        Some(Control::Request(Request::MeasuredStart(request))) => {
                            let result = actor.start_measured(request.admission_epoch, request.settings)
                                .map_err(|error| error.to_string());
                            actor.publish()?;
                            if request.reply.send(result).is_err() { /* caller no longer waiting */ }
                        }
                        Some(Control::Request(Request::Start(request))) => {
                            let actions = actor.start_request(
                                request.admission_epoch, request.settings, request.process, request.delivery, request.toggle,
                            );
                            let result = match actions {
                                Ok(actions) => {
                                    actor.error = None;
                                    actor.execute(actions).map_err(|error| error.to_string())
                                }
                                Err(error) => Err(error.to_string()),
                            };
                            actor.publish()?;
                            // A closed UI listener does not revoke an accepted command.
                            if request.reply.send(result).is_err() { /* caller no longer waiting */
                            }
                        }
                        None => {}
                    }
                } })).unwrap_or(Err(RuntimeError::Worker));
                // Revoke outputs and settle accepted requests before potentially slow owner joins.
                close_admission(&actor.shared);
                reject_pending(&requests);
                // All paths release native audio and join inference here,
                // outside the GUI, including controller/service errors.
                // Each cleanup is independently contained: a failing/panicking
                // audio owner must not bypass joining the inference owner.
                let audio_cleanup = catch_unwind(AssertUnwindSafe(|| {
                    actor.audio.shutdown().map_err(RuntimeError::Service)
                })).unwrap_or(Err(RuntimeError::Worker));
                let inference_cleanup = catch_unwind(AssertUnwindSafe(|| {
                    match actor.inference.take() {
                        Some(service) => service.shutdown().map_err(service_error),
                        None => Ok(()),
                    }
                })).unwrap_or(Err(RuntimeError::Worker));
                let result = result.and(audio_cleanup).and(inference_cleanup);
                if let Err(error) = &result
                    && let Ok(mut shared) = actor.shared.lock() {
                    shared.snapshot.error = Some(error.to_string());
                }
                result
            })
            .map_err(|_| RuntimeError::Worker)?;
        Ok(Self {
            client,
            worker: Some(worker),
        })
    }
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    pub fn join(mut self) -> Result<(), RuntimeError> {
        let requested = self.client.shutdown();
        let joined = self
            .worker
            .take()
            .ok_or(RuntimeError::Worker)?
            .join()
            .map_err(|_| RuntimeError::Worker)?;
        joined?;
        match requested {
            Ok(()) | Err(RuntimeError::Mailbox(MailboxError::Closed)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if self.client.shutdown().is_err() { /* worker already stopped */ }
    }
}

fn service_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Service(error.to_string())
}

struct Actor<A: AudioDriver> {
    controller: Controller,
    inference: Option<InferenceService>,
    generation: u64,
    audio: A,
    native: NativePort,
    shared: Arc<Mutex<Shared>>,
    maximum_warning: bool,
    maximum_reached: bool,
    progress: Option<CaptureProgress>,
    error: Option<String>,
    capture_pending: Option<(u64, Instant)>,
    deferred_audio: Option<(u64, Option<String>)>,
}

impl<A: AudioDriver> Actor<A> {
    fn start_request(
        &mut self,
        admission_epoch: u64,
        settings: SessionSettings,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<Vec<Action>, RuntimeError> {
        let shared = Arc::clone(&self.shared);
        let mut admission = shared.lock().map_err(|_| RuntimeError::Synchronization)?;
        if admission.closing {
            return Err(MailboxError::Closed.into());
        }
        if admission.suspended {
            return Err(RuntimeError::Suspended);
        }
        if admission_epoch != admission.admission_epoch {
            return Err(RuntimeError::StaleAdmission);
        }
        let snapshot = self.controller.snapshot();
        let actions = if toggle && matches!(snapshot.phase, Phase::Starting | Phase::Recording) {
            self.controller
                .stop(snapshot.session.ok_or(RuntimeError::Synchronization)?)
        } else {
            let ready = self.inference.as_ref().is_some_and(InferenceService::ready)
                && settings.generation == self.generation;
            self.controller.start(settings, process, ready, delivery)
        }
        .map_err(service_error)?;
        if self.controller.snapshot().session != snapshot.session {
            self.maximum_warning = false;
            self.maximum_reached = false;
        }
        // Register before dispatch: cancellation and shutdown can see a session
        // even while its first native command is being sent.
        admission.cancellation = self.controller.cancellation_handles();
        Ok(actions)
    }

    fn reconcile_revoked(&mut self) -> Result<(), RuntimeError> {
        let actions = {
            let shared = Arc::clone(&self.shared);
            let _admission = shared.lock().map_err(|_| RuntimeError::Synchronization)?;
            self.controller.reconcile_cancel().map_err(service_error)?
        };
        self.execute(actions)
    }

    fn fail(
        &mut self,
        session: u64,
        message: String,
    ) -> Result<Vec<Action>, crate::controller::ControllerError> {
        let shared = Arc::clone(&self.shared);
        let _admission = shared
            .lock()
            .map_err(|_| crate::controller::ControllerError::Resource)?;
        self.controller.fail(session, message)
    }

    fn accept(&mut self, event: Event) -> Result<Vec<Action>, crate::controller::ControllerError> {
        self.accept_with_timing(
            event,
            #[cfg(feature = "measurement")]
            None,
        )
    }

    fn accept_with_timing(
        &mut self,
        event: Event,
        #[cfg(feature = "measurement")] decode_timing: Option<
            crate::inference_service::DecodeTiming,
        >,
    ) -> Result<Vec<Action>, crate::controller::ControllerError> {
        // Result adoption and immediate revocation have a single ordering point.
        // The lock covers only state mutation, never inference or device I/O.
        let shared = Arc::clone(&self.shared);
        let _admission = shared
            .lock()
            .map_err(|_| crate::controller::ControllerError::Resource)?;
        #[cfg(feature = "measurement")]
        let previous = self.controller.snapshot();
        #[cfg(feature = "measurement")]
        let candidate = self.controller.inference_matches(&event)
            && matches!(&event, Event::InferenceFinished { result: Ok(_), .. });
        #[cfg(feature = "measurement")]
        let audio_final = matches!(&event, Event::SegmentedFinished { session, result: Ok(_), .. }
            if previous.session == Some(*session));
        #[cfg(feature = "measurement")]
        let decode_timing = decode_timing.filter(|timing| matches!(&event,
            Event::InferenceFinished { sample_range, .. } if timing.input_frames == sample_range.len()));
        let actions = self.controller.accept(event)?;
        #[cfg(feature = "measurement")]
        {
            // Capture at the actual adoption boundary, before snapshot cloning,
            // action dispatch, publication, or any observer's polling interval.
            let adopted = Instant::now();
            if let Some(trace) = &_admission.timing {
                trace
                    .accepted(
                        &previous,
                        &self.controller.snapshot(),
                        candidate || audio_final,
                        adopted,
                        decode_timing,
                    )
                    .map_err(|_| crate::controller::ControllerError::Resource)?;
            }
        }
        Ok(actions)
    }

    fn execute(&mut self, actions: Vec<Action>) -> Result<(), RuntimeError> {
        let mut pending: VecDeque<_> = actions.into();
        while let Some(action) = pending.pop_front() {
            let follow = match action {
                Action::CaptureTarget {
                    session,
                    process,
                    token,
                } => {
                    if self
                        .native
                        .outgoing
                        .try_send(NativeCommand::Capture {
                            session,
                            process,
                            token: token.clone(),
                        })
                        .is_err()
                    {
                        token.cancel();
                        self.error = Some("Native target capture unavailable".into());
                    } else {
                        self.capture_pending = Some((session.get(), Instant::now()));
                    }
                    vec![]
                }
                Action::StartAudio { session, device } => {
                    // Share admission with Client::cancel: after cancel returns no
                    // new microphone start can pass this boundary.
                    let shared = Arc::clone(&self.shared);
                    let admission = shared.lock().map_err(|_| RuntimeError::Synchronization)?;
                    if (admission.closing || admission.suspended)
                        && let Some((_, _, work)) = self.controller.cancellation_handles()
                    {
                        work.cancel();
                    }
                    let mut follow = self.controller.reconcile_cancel().map_err(service_error)?;
                    let valid = self
                        .controller
                        .cancellation_handles()
                        .is_some_and(|(id, _, work)| id == session && work.is_valid());
                    if !valid {
                        follow.extend(
                            self.controller
                                .accept(Event::AudioFinished {
                                    session,
                                    result: Err("Capture cancelled before microphone start".into()),
                                })
                                .map_err(service_error)?,
                        );
                    } else if self.capture_pending.is_some_and(|(id, _)| id == session) {
                        self.deferred_audio = Some((session, device));
                    } else if let Err(message) = self.audio.start(session, device) {
                        follow.extend(
                            self.controller
                                .accept(Event::AudioFinished {
                                    session,
                                    result: Err(message),
                                })
                                .map_err(service_error)?,
                        );
                    }
                    follow
                }
                Action::StopAudio(session) => {
                    self.audio.stop(session);
                    if self
                        .deferred_audio
                        .as_ref()
                        .is_some_and(|(pending, _)| *pending == session)
                    {
                        self.deferred_audio = None;
                        self.capture_pending = None;
                        self.controller
                            .accept(Event::AudioFinished {
                                session,
                                result: Err("Capture cancelled before microphone start".into()),
                            })
                            .map_err(service_error)?
                    } else {
                        vec![]
                    }
                }
                Action::Transcribe(job) => {
                    let session = job.session;
                    let result = self
                        .inference
                        .as_ref()
                        .ok_or_else(|| "Model is unavailable".to_owned())
                        .and_then(|service| service.submit(job).map_err(|error| error.to_string()));
                    match result {
                        Ok(()) => vec![],
                        Err(message) => self.fail(session, message).map_err(service_error)?,
                    }
                }
                Action::CancelInference(session) => {
                    if let Some(service) = &self.inference {
                        service.cancel(session).map_err(service_error)?;
                    }
                    vec![]
                }
                Action::Deliver {
                    session,
                    text,
                    token,
                } => {
                    if self
                        .native
                        .outgoing
                        .try_send(NativeCommand::Deliver {
                            session,
                            text,
                            token,
                        })
                        .is_err()
                    {
                        self.controller
                            .accept(Event::DeliveryFinished {
                                session,
                                result: DeliveryResult::Held("Native delivery unavailable".into()),
                            })
                            .map_err(service_error)?
                    } else {
                        vec![]
                    }
                }
                Action::AbortDelivery(session) => {
                    if self
                        .native
                        .outgoing
                        .try_send(NativeCommand::Cancel(session))
                        .is_err()
                    {
                        self.error = Some("Native cleanup notification unavailable".into());
                    }
                    vec![]
                }
            };
            pending.extend(follow);
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<(), RuntimeError> {
        for event in self.audio.poll() {
            let event = match event {
                #[cfg(feature = "measurement")]
                CaptureEvent::Segment { session, prefix } => Event::Segment {
                    session,
                    timeline_range: prefix.timeline_range,
                    samples: prefix.samples,
                },
                #[cfg(feature = "measurement")]
                CaptureEvent::SegmentedFinished {
                    session,
                    base,
                    result,
                    maximum,
                } => {
                    if self.controller.audio_is_unreleased(session) && maximum {
                        self.maximum_reached = true;
                    }
                    Event::SegmentedFinished {
                        session,
                        base,
                        result: result.map_err(|e| e.to_string()),
                    }
                }
                CaptureEvent::Started { session, .. } => Event::AudioStarted(session),
                CaptureEvent::Finished {
                    session,
                    result,
                    maximum,
                } => {
                    if self.controller.audio_is_unreleased(session) && maximum {
                        self.maximum_reached = true;
                    }
                    Event::AudioFinished {
                        session,
                        result: result.map_err(|error| error.to_string()),
                    }
                }
                CaptureEvent::MaximumWarning { session } => {
                    if self.controller.snapshot().session == Some(session) {
                        self.maximum_warning = true;
                    }
                    continue;
                }
            };
            let actions = self.accept(event).map_err(service_error)?;
            self.execute(actions)?;
        }
        if let Some(mut progress) = self.audio.take_progress() {
            let session = self.controller.snapshot();
            if session.phase == Phase::Recording
                && session.session == Some(progress.session)
                && self.progress.is_none_or(|old| {
                    old.session != progress.session || old.elapsed_millis <= progress.elapsed_millis
                })
            {
                progress.elapsed_millis = progress.elapsed_millis.min(300_000);
                progress.level_permille = progress.level_permille.min(1_000);
                self.progress = Some(progress);
            }
        }
        if let Some(service) = &self.inference
            && let Some(done) = service.take_result().map_err(service_error)?
        {
            let actions = self
                .accept_with_timing(
                    Event::InferenceFinished {
                        session: done.session,
                        generation: done.generation,
                        job_id: done.job_id,
                        sample_range: done.sample_range,
                        timeline_range: done.timeline_range,
                        result: done.result.map_err(|error| error.to_string()),
                    },
                    #[cfg(feature = "measurement")]
                    done.decode_timing,
                )
                .map_err(service_error)?;
            self.execute(actions)?;
        }
        #[cfg(feature = "measurement")]
        if self.audio.pretranscription()
            && let Some(epoch) = self.controller.seal_available()
            && self
                .inference
                .as_ref()
                .is_some_and(|s| s.busy().is_ok_and(|busy| !busy))
        {
            self.audio.allow_seal(epoch);
        }
        if self
            .inference
            .as_ref()
            .is_some_and(InferenceService::is_finished)
        {
            let service = self.inference.take().ok_or(RuntimeError::Synchronization)?;
            let message = service.shutdown().err().map_or_else(
                || "Inference worker stopped".into(),
                |error| error.to_string(),
            );
            self.error = Some(message.clone());
            if let Some(session) = self.controller.snapshot().session {
                let actions = self.fail(session, message).map_err(service_error)?;
                self.execute(actions)?;
            }
        }
        for _ in 0..8 {
            match self.native.incoming.try_recv() {
                Ok(NativeResult::Captured { session, result }) => {
                    if self
                        .capture_pending
                        .is_some_and(|(pending, _)| pending == session)
                    {
                        self.capture_pending = None;
                        if let Err(message) = result {
                            if let Some((_, token, _)) = self.controller.cancellation_handles() {
                                token.cancel();
                            }
                            self.error = Some(message);
                        }
                        if let Some((session, device)) = self.deferred_audio.take() {
                            self.execute(vec![Action::StartAudio { session, device }])?;
                        }
                    }
                }
                Ok(NativeResult::Delivered { session, result }) => {
                    let actions = self
                        .accept(Event::DeliveryFinished { session, result })
                        .map_err(service_error)?;
                    self.execute(actions)?;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let snapshot = self.controller.snapshot();
                    if snapshot.phase == Phase::Delivering
                        && let Some(session) = snapshot.session
                    {
                        let actions = self
                            .accept(Event::DeliveryFinished {
                                session,
                                result: DeliveryResult::Held("Native delivery disconnected".into()),
                            })
                            .map_err(service_error)?;
                        self.execute(actions)?;
                    }
                    break;
                }
            }
        }
        if self
            .capture_pending
            .is_some_and(|(_, started)| started.elapsed() >= Duration::from_secs(2))
        {
            self.capture_pending = None;
            self.error = Some("Start target capture timed out; automatic delivery disabled".into());
            if let Some((_, token, _)) = self.controller.cancellation_handles() {
                token.cancel();
            }
            if let Some((session, device)) = self.deferred_audio.take() {
                self.execute(vec![Action::StartAudio { session, device }])?;
            }
        }
        Ok(())
    }

    fn publish(&self) -> Result<(), RuntimeError> {
        let mut shared = self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        let session = self.controller.snapshot();
        #[cfg(feature = "measurement")]
        if let Some(trace) = &shared.timing {
            trace.terminal(&session)?;
        }
        let progress = self.progress.filter(|value| {
            !shared.closing
                && !shared.suspended
                && session.phase == Phase::Recording
                && session.session == Some(value.session)
                && self
                    .controller
                    .cancellation_handles()
                    .is_some_and(|(id, _, work)| id == value.session && work.is_valid())
        });
        let mut next = RuntimeSnapshot {
            revision: shared.snapshot.revision,
            running: true,
            model_ready: self.inference.as_ref().is_some_and(InferenceService::ready),
            session,
            maximum_warning: self.maximum_warning,
            maximum_reached: self.maximum_reached,
            elapsed_millis: progress.map_or(0, |value| value.elapsed_millis),
            level_permille: progress.map_or(0, |value| value.level_permille),
            error: self.error.clone(),
        };
        if next != shared.snapshot {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(RuntimeError::Synchronization)?;
            shared.snapshot = next;
        }
        shared.cancellation = self.controller.cancellation_handles();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        inference_service::Decoder,
        transcription::{InferenceError, Language},
    };
    use dictation_core::{audio::AudioFormat, session::Outcome};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FixtureDecoder;
    impl Decoder for FixtureDecoder {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            if audio.len() != 800 {
                return Err(InferenceError::InvalidAudio);
            }
            Ok("fixture words".into())
        }
    }
    struct ControlledAudio {
        events: Receiver<CaptureEvent>,
        started: Sender<u64>,
        stopped: Sender<u64>,
        closed: Arc<AtomicBool>,
        gate: Option<(Arc<AtomicBool>, Sender<()>, Receiver<()>)>,
        progress: Option<Arc<crate::audio::ProgressSlot>>,
    }
    impl AudioDriver for ControlledAudio {
        fn start(&mut self, session: u64, _: Option<String>) -> Result<(), String> {
            self.started
                .try_send(session)
                .map_err(|error| error.to_string())
        }
        fn stop(&self, session: u64) {
            assert!(self.stopped.try_send(session).is_ok());
        }
        fn poll(&mut self) -> Vec<CaptureEvent> {
            if let Some((armed, entered, resume)) = &self.gate
                && armed.swap(false, Ordering::AcqRel)
            {
                assert!(entered.send(()).is_ok());
                assert!(resume.recv_timeout(Duration::from_secs(2)).is_ok());
            }
            self.events.try_iter().take(3).collect()
        }
        fn shutdown(&mut self) -> Result<(), String> {
            self.closed.store(true, Ordering::Release);
            Ok(())
        }
        fn take_progress(&mut self) -> Option<crate::audio::CaptureProgress> {
            self.progress.as_ref().and_then(|slot| slot.take())
        }
    }
    fn settings() -> SessionSettings {
        SessionSettings {
            device: None,
            language: Language::German,
            generation: 1,
        }
    }
    fn wait_for(
        client: &Client,
        predicate: impl Fn(&RuntimeSnapshot) -> bool,
    ) -> Result<RuntimeSnapshot, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let state = client.snapshot()?;
            if predicate(&state) {
                return Ok(state);
            }
            if !state.running || Instant::now() >= deadline {
                return Err("runtime state did not arrive".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn accepted(
        reply: oneshot::Receiver<Result<(), String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        acknowledged(reply)?.map_err(Into::into)
    }
    fn acknowledged(
        mut reply: oneshot::Receiver<Result<(), String>>,
    ) -> Result<Result<(), String>, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match reply.try_recv() {
                Ok(result) => return Ok(result),
                Err(oneshot::error::TryRecvError::Empty) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(1))
                }
                _ => return Err("request acknowledgement missing".into()),
            }
        }
    }

    struct LifecycleFixture {
        runtime: Runtime,
        audio: Sender<CaptureEvent>,
        starts: Receiver<u64>,
        stops: Receiver<u64>,
        native: NativeEndpoint,
        armed: Arc<AtomicBool>,
        at_poll: Receiver<()>,
        continue_poll: Sender<()>,
        closed: Arc<AtomicBool>,
        progress: Arc<crate::audio::ProgressSlot>,
    }
    impl LifecycleFixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Self::with_decoder(FixtureDecoder)
        }
        fn with_decoder(decoder: impl Decoder) -> Result<Self, Box<dyn std::error::Error>> {
            let (audio, events) = bounded(3);
            let (started, starts) = bounded(4);
            let (stopped, stops) = bounded(4);
            let (entered, at_poll) = bounded(1);
            let (continue_poll, proceed) = bounded(1);
            let armed = Arc::new(AtomicBool::new(false));
            let closed = Arc::new(AtomicBool::new(false));
            let progress = Arc::new(crate::audio::ProgressSlot::default());
            let (port, native) = native_mailbox();
            let runtime = Runtime::with_audio(
                InferenceService::with_decoder(decoder)?,
                1,
                port,
                ControlledAudio {
                    events,
                    started,
                    stopped,
                    closed: Arc::clone(&closed),
                    gate: Some((Arc::clone(&armed), entered, proceed)),
                    progress: Some(Arc::clone(&progress)),
                },
            )?;
            wait_for(&runtime.client(), |state| state.model_ready)?;
            Ok(Self {
                runtime,
                audio,
                starts,
                stops,
                native,
                armed,
                at_poll,
                continue_poll,
                closed,
                progress,
            })
        }
        fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.armed.store(true, Ordering::Release);
            self.at_poll.recv_timeout(Duration::from_secs(1))?;
            Ok(())
        }
        fn proceed(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.continue_poll.send(())?;
            Ok(())
        }
        fn record(&self) -> Result<DeliveryToken, Box<dyn std::error::Error>> {
            accepted(self.runtime.client().start(settings(), None, true, false)?)?;
            let token = match self.native.requests.recv_timeout(Duration::from_secs(1))? {
                NativeCommand::Capture { token, .. } => token,
                _ => return Err("capture missing".into()),
            };
            self.native.responses.send(NativeResult::Captured {
                session: 1,
                result: Ok(()),
            })?;
            assert_eq!(self.starts.recv_timeout(Duration::from_secs(1))?, 1);
            self.audio.send(CaptureEvent::Started {
                session: 1,
                format: AudioFormat {
                    sample_rate: 16_000,
                    channels: 1,
                },
                started: Instant::now(),
            })?;
            wait_for(&self.runtime.client(), |state| {
                state.session.phase == Phase::Recording
            })?;
            Ok(token)
        }
        fn deliver(&self) -> Result<DeliveryToken, Box<dyn std::error::Error>> {
            self.record()?;
            self.runtime.client().stop(1)?;
            assert_eq!(self.stops.recv_timeout(Duration::from_secs(1))?, 1);
            self.audio.send(CaptureEvent::Finished {
                session: 1,
                result: Ok(vec![0.1; 800]),
                maximum: false,
            })?;
            match self.native.requests.recv_timeout(Duration::from_secs(1))? {
                NativeCommand::Deliver { token, .. } => Ok(token),
                _ => Err("delivery missing".into()),
            }
        }
    }

    #[test]
    fn duplicate_finish_cannot_turn_a_manual_stop_into_a_maximum_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        fixture.record()?;
        let client = fixture.runtime.client();
        client.stop(1)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        fixture.pause()?;
        for maximum in [false, true] {
            fixture.audio.send(CaptureEvent::Finished {
                session: 1,
                result: Ok(vec![0.1; 800]),
                maximum,
            })?;
        }
        fixture.proceed()?;
        let state = wait_for(&client, |state| state.session.phase == Phase::Delivering)?;
        assert!(!state.maximum_reached);
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn held_feedback_is_authorized_only_for_the_latest_held_idle_session()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::cell::Cell;
        let fixture = LifecycleFixture::new()?;
        fixture.deliver()?;
        let client = fixture.runtime.client();
        let calls = Cell::new(0);
        assert_eq!(client.with_idle_feedback(1, || calls.set(1))?, None);
        fixture.native.responses.send(NativeResult::Delivered {
            session: 1,
            result: DeliveryResult::Held("target changed".into()),
        })?;
        wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert_eq!(client.with_idle_feedback(2, || calls.set(99))?, None);
        assert_eq!(
            client.with_idle_feedback(1, || {
                calls.set(calls.get() + 1);
                42
            })?,
            Some(42)
        );
        assert_eq!(calls.get(), 1);
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(client.with_idle_feedback(1, || calls.set(99))?, None);
        client.shutdown()?;
        assert_eq!(client.with_idle_feedback(1, || calls.set(99))?, None);
        assert_eq!(calls.get(), 1);
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn maximum_feedback_survives_toggle_stop_and_resets_only_for_a_new_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        fixture.record()?;
        let client = fixture.runtime.client();
        fixture
            .audio
            .send(CaptureEvent::MaximumWarning { session: 1 })?;
        wait_for(&client, |state| state.maximum_warning)?;
        accepted(client.start(settings(), None, true, true)?)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        assert!(client.snapshot()?.maximum_warning);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: true,
        })?;
        let state = wait_for(&client, |state| state.session.phase == Phase::Delivering)?;
        assert!(state.maximum_reached);
        fixture.native.responses.send(NativeResult::Delivered {
            session: 1,
            result: DeliveryResult::Held("target changed".into()),
        })?;
        let state = wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert!(state.maximum_reached);
        assert_eq!(state.session.last_held_session, Some(1));
        accepted(client.start(settings(), None, false, false)?)?;
        assert!(!client.snapshot()?.maximum_warning);
        assert!(!client.snapshot()?.maximum_reached);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![]),
            maximum: true,
        })?;
        // An acknowledgement forces publication after the stale event poll.
        accepted(client.clear_text()?)?;
        assert!(!client.snapshot()?.maximum_reached);
        client.shutdown()?;
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn progress_is_monotone_session_bound_and_cannot_displace_cancel_or_finish()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::audio::CaptureProgress;
        let fixture = LifecycleFixture::new()?;
        fixture.record()?;
        let client = fixture.runtime.client();
        let (mut port, mut reader) = crate::audio::capture_buffer(AudioFormat {
            sample_rate: 16_000,
            channels: 2,
        })?;
        port.publish(&[0.3_f32, 0.4]);
        reader.read()?;
        fixture
            .progress
            .publish(reader.progress(1, Duration::from_millis(120)));
        let measured = wait_for(&client, |state| state.elapsed_millis == 120)?;
        assert_eq!(measured.level_permille, 354);
        fixture.pause()?;
        fixture.progress.publish(CaptureProgress {
            session: 1,
            elapsed_millis: 110,
            level_permille: 10,
        });
        fixture.proceed()?;
        accepted(client.clear_text()?)?;
        assert_eq!(client.snapshot()?.elapsed_millis, 120);
        assert_eq!(client.snapshot()?.level_permille, 354);
        fixture.pause()?;
        for elapsed_millis in 1_000..11_000 {
            fixture.progress.publish(CaptureProgress {
                session: 1,
                elapsed_millis,
                level_permille: 500,
            });
        }
        client.stop(1)?;
        client.cancel(1)?;
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![]),
            maximum: false,
        })?;
        fixture.proceed()?;
        wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert_eq!(
            (
                client.snapshot()?.elapsed_millis,
                client.snapshot()?.level_permille
            ),
            (0, 0)
        );
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(fixture.starts.recv_timeout(Duration::from_secs(1))?, 2);
        fixture.audio.send(CaptureEvent::Started {
            session: 2,
            format: AudioFormat {
                sample_rate: 16_000,
                channels: 1,
            },
            started: Instant::now(),
        })?;
        wait_for(&client, |state| state.session.phase == Phase::Recording)?;
        fixture.pause()?;
        fixture.progress.publish(CaptureProgress {
            session: 1,
            elapsed_millis: 90_000,
            level_permille: 999,
        });
        fixture.proceed()?;
        accepted(client.clear_text()?)?;
        assert_eq!(
            (
                client.snapshot()?.elapsed_millis,
                client.snapshot()?.level_permille
            ),
            (0, 0)
        );
        fixture.progress.publish(CaptureProgress {
            session: 2,
            elapsed_millis: 50,
            level_permille: 250,
        });
        wait_for(&client, |state| state.elapsed_millis == 50)?;
        client.shutdown()?;
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn progress_flood_preserves_stop_and_finished_through_the_runtime()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        fixture.record()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        for elapsed_millis in 0..10_000 {
            fixture.progress.publish(CaptureProgress {
                session: 1,
                elapsed_millis,
                level_permille: 500,
            });
        }
        client.stop(1)?;
        fixture.proceed()?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        wait_for(&client, |state| state.session.phase == Phase::Finalizing)?;
        assert_eq!(client.snapshot()?.elapsed_millis, 0);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        assert!(matches!(
            fixture
                .native
                .requests
                .recv_timeout(Duration::from_secs(1))?,
            NativeCommand::Deliver { session: 1, .. }
        ));
        assert_eq!(client.snapshot()?.level_permille, 0);
        client.shutdown()?;
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn full_normal_queue_preserves_active_stop_against_foreign_session_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        for foreign_first in [false, true] {
            let fixture = LifecycleFixture::new()?;
            fixture.record()?;
            let client = fixture.runtime.client();
            fixture.pause()?;
            let mut replies = Vec::new();
            for _ in 0..16 {
                replies.push(client.clear_text()?);
            }
            assert!(matches!(
                client.start(settings(), None, false, false),
                Err(MailboxError::Full)
            ));
            if foreign_first {
                client.stop(2)?;
                client.cancel(3)?;
            }
            client.stop(1)?;
            if !foreign_first {
                client.stop(2)?;
                client.cancel(3)?;
            }
            fixture.proceed()?;
            // Each ordinary acknowledgement fences the independently pending control;
            // the actor cannot consume a normal request while its priority is occupied.
            for reply in replies {
                accepted(reply)?;
            }
            let stopped = fixture.stops.try_recv();
            let phase = client.snapshot()?.session.phase;
            fixture.runtime.join()?;
            assert!(
                matches!(stopped, Ok(1)),
                "foreign session control displaced the current stop: {stopped:?}"
            );
            assert_eq!(phase, Phase::Finalizing);
        }
        Ok(())
    }

    #[test]
    fn full_normal_queue_cannot_displace_cancel_or_lose_the_last_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let token = fixture.record()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        let mut replies = Vec::new();
        for _ in 0..15 {
            replies.push(client.clear_text()?);
        }
        let last = client.start(settings(), None, false, false)?;
        assert!(matches!(client.clear_text(), Err(MailboxError::Full)));
        client.cancel(99)?;
        assert!(
            token.is_pending(),
            "foreign cancel revoked the current delivery"
        );
        client.cancel(1)?;
        client.stop(1)?;
        assert!(
            !token.is_pending(),
            "current cancel must revoke before actor progress"
        );
        fixture.proceed()?;
        for reply in replies {
            accepted(reply)?;
        }
        assert!(
            acknowledged(last)?.is_err(),
            "last accepted start must return busy during cancellation"
        );
        assert_eq!(fixture.stops.try_recv()?, 1);
        assert_eq!(client.snapshot()?.session.phase, Phase::Cancelling);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        wait_for(&client, |snapshot| {
            snapshot.session.phase == Phase::Idle
                && snapshot.session.outcome == Some(Outcome::Cancelled)
        })?;
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(fixture.starts.recv_timeout(Duration::from_secs(1))?, 2);
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn stale_controls_racing_a_session_change_cannot_cancel_the_new_recording()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        fixture.record()?;
        let client = fixture.runtime.client();
        client.cancel(1)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        wait_for(&client, |snapshot| {
            snapshot.session.phase == Phase::Cancelling
        })?;
        fixture.pause()?;
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        let next = client.start(settings(), None, false, false)?;
        let (race, racing) = bounded(1);
        let old_client = client.clone();
        let old_controls = thread::spawn(move || -> Result<(), RuntimeError> {
            racing
                .recv_timeout(Duration::from_secs(1))
                .map_err(service_error)?;
            for _ in 0..8 {
                old_client.stop(1)?;
                old_client.cancel(1)?;
            }
            Ok(())
        });
        race.send(())?;
        fixture.proceed()?;
        accepted(next)?;
        old_controls
            .join()
            .map_err(|_| "old control sender panicked")??;
        assert_eq!(fixture.starts.recv_timeout(Duration::from_secs(1))?, 2);
        fixture.audio.send(CaptureEvent::Started {
            session: 2,
            format: AudioFormat {
                sample_rate: 16_000,
                channels: 1,
            },
            started: Instant::now(),
        })?;
        wait_for(&client, |snapshot| {
            snapshot.session.session == Some(2) && snapshot.session.phase == Phase::Recording
        })?;
        fixture.pause()?;
        let mut replies = Vec::new();
        for _ in 0..16 {
            replies.push(client.clear_text()?);
        }
        client.stop(2)?;
        client.cancel(1)?;
        client.stop(1)?;
        fixture.proceed()?;
        for reply in replies {
            accepted(reply)?;
        }
        assert_eq!(fixture.stops.try_recv()?, 2);
        assert_eq!(client.snapshot()?.session.phase, Phase::Finalizing);
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn sleep_revokes_recording_before_wake_and_waits_for_audio_release()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let token = fixture.record()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        client.suspend()?;
        client.suspend()?;
        assert!(client.is_suspended()?);
        assert!(!token.is_pending());
        assert!(matches!(
            client.start(settings(), None, false, false),
            Err(MailboxError::Suspended)
        ));
        client.resume()?;
        client.resume()?;
        assert!(!client.is_suspended()?);
        fixture.proceed()?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        wait_for(&client, |state| state.session.phase == Phase::Cancelling)?;
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        wait_for(&client, |state| {
            state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Cancelled)
        })?;
        assert!(matches!(
            fixture.starts.try_recv(),
            Err(TryRecvError::Empty)
        ));
        while let Ok(command) = fixture.native.requests.try_recv() {
            assert!(!matches!(command, NativeCommand::Deliver { .. }));
        }
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(fixture.starts.recv_timeout(Duration::from_secs(1))?, 2);
        fixture.runtime.join()?;
        assert!(fixture.closed.load(Ordering::Acquire));
        Ok(())
    }

    #[test]
    fn target_reply_after_sleep_cannot_start_deferred_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let client = fixture.runtime.client();
        accepted(client.start(settings(), None, true, false)?)?;
        let token = match fixture
            .native
            .requests
            .recv_timeout(Duration::from_secs(1))?
        {
            NativeCommand::Capture { token, .. } => token,
            _ => return Err("capture missing".into()),
        };
        fixture.pause()?;
        client.suspend()?;
        client.resume()?;
        fixture.native.responses.send(NativeResult::Captured {
            session: 1,
            result: Ok(()),
        })?;
        fixture.proceed()?;
        let snapshot = wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert_eq!(snapshot.session.outcome, Some(Outcome::Cancelled));
        assert!(!token.is_pending());
        assert!(matches!(
            fixture.starts.try_recv(),
            Err(TryRecvError::Empty)
        ));
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn sleep_before_delivery_commit_revokes_late_result_after_wake()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let token = fixture.deliver()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        client.suspend()?;
        assert!(!token.is_pending());
        assert!(token.try_commit().is_err());
        client.resume()?;
        fixture.native.responses.send(NativeResult::Delivered {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        fixture.proceed()?;
        let snapshot = wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert_eq!(snapshot.session.outcome, Some(Outcome::Cancelled));
        assert_eq!(snapshot.session.text.as_deref(), Some("fixture words"));
        assert!(matches!(
            fixture.starts.try_recv(),
            Err(TryRecvError::Empty)
        ));
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn delivery_committed_before_sleep_stays_sent_and_duplicate_power_events_are_harmless()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let token = fixture.deliver()?;
        token.try_commit()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        client.suspend()?;
        client.suspend()?;
        assert!(token.is_committed());
        client.resume()?;
        client.resume()?;
        let queued = client.start(settings(), None, false, false)?;
        fixture.proceed()?;
        // A normal request acknowledgement fences the higher-priority lifecycle
        // control. Deliberately withhold Delivered until that control was handled.
        assert!(accepted(queued).is_err());
        let error_before_delivery = client.snapshot()?.error;
        fixture.native.responses.send(NativeResult::Delivered {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        let snapshot = wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert_eq!(snapshot.session.outcome, Some(Outcome::Sent));
        assert_eq!(snapshot.session.text.as_deref(), Some("fixture words"));
        assert_eq!(snapshot.error, None);
        assert_eq!(error_before_delivery, None);
        assert!(matches!(
            fixture.starts.try_recv(),
            Err(TryRecvError::Empty)
        ));
        fixture.runtime.join()?;
        Ok(())
    }

    #[test]
    fn shutdown_rejects_queued_requests_before_a_blocked_decoder_finishes()
    -> Result<(), Box<dyn std::error::Error>> {
        struct BlockedDecoder {
            entered: Sender<()>,
            release: Receiver<()>,
        }
        impl Decoder for BlockedDecoder {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                self.entered
                    .send(())
                    .map_err(|_| InferenceError::InvalidAudio)?;
                self.release
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|_| InferenceError::InvalidAudio)?;
                Ok("revoked decoder result".into())
            }
        }
        let (entered, decoding) = bounded(1);
        let (release_decoder, released) = bounded(1);
        let fixture = LifecycleFixture::with_decoder(BlockedDecoder {
            entered,
            release: released,
        })?;
        fixture.record()?;
        let client = fixture.runtime.client();
        client.stop(1)?;
        assert_eq!(fixture.stops.recv_timeout(Duration::from_secs(1))?, 1);
        fixture.audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        decoding.recv_timeout(Duration::from_secs(1))?;
        fixture.pause()?;
        let mut start_reply = client.start(settings(), None, false, false)?;
        let mut clear_reply = client.clear_text()?;
        client.shutdown()?;
        fixture.proceed()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut start_rejected, mut clear_rejected) = (false, false);
        while Instant::now() < deadline && !(start_rejected && clear_rejected) {
            if matches!(start_reply.try_recv(), Ok(Err(_))) {
                start_rejected = true;
            }
            if matches!(clear_reply.try_recv(), Ok(Err(_))) {
                clear_rejected = true;
            }
            thread::yield_now();
        }
        // Always release and join, including the RED case, before reporting a failure.
        release_decoder.send(())?;
        fixture.runtime.join()?;
        assert!(
            start_rejected,
            "accepted start acknowledgement waited for decoder shutdown"
        );
        assert!(
            clear_rejected,
            "accepted clear acknowledgement waited for decoder shutdown"
        );
        Ok(())
    }

    #[test]
    fn shutdown_dominates_wake_and_never_starts_queued_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = LifecycleFixture::new()?;
        let client = fixture.runtime.client();
        fixture.pause()?;
        let mut reply = client.start(settings(), None, false, false)?;
        let mut clear_reply = client.clear_text()?;
        client.suspend()?;
        client.shutdown()?;
        assert!(matches!(
            client.resume(),
            Err(RuntimeError::Mailbox(MailboxError::Closed))
        ));
        assert!(matches!(
            client.suspend(),
            Err(RuntimeError::Mailbox(MailboxError::Closed))
        ));
        assert!(matches!(
            client.start(settings(), None, false, false),
            Err(MailboxError::Closed)
        ));
        fixture.proceed()?;
        fixture.runtime.join()?;
        assert!(
            matches!(reply.try_recv(), Ok(Err(_))),
            "shutdown must settle an accepted queued request"
        );
        assert!(matches!(clear_reply.try_recv(), Ok(Err(_))));
        assert!(fixture.closed.load(Ordering::Acquire));
        assert!(!client.snapshot()?.running);
        assert!(matches!(
            fixture.starts.try_recv(),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected)
        ));
        Ok(())
    }

    #[test]
    fn panic_guard_settles_queued_request_and_closes_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        struct PanicAudio {
            entered: Sender<()>,
            proceed: Receiver<()>,
        }
        impl AudioDriver for PanicAudio {
            fn start(&mut self, _: u64, _: Option<String>) -> Result<(), String> {
                Ok(())
            }
            fn stop(&self, _: u64) {}
            fn poll(&mut self) -> Vec<CaptureEvent> {
                assert!(self.entered.send(()).is_ok());
                // Sending the rendezvous value injects a worker panic, without
                // panicking the test caller or touching any native device.
                assert!(
                    self.proceed.recv_timeout(Duration::from_secs(2)).is_err(),
                    "injected worker failure"
                );
                Vec::new()
            }
            fn shutdown(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
        let (entered, at_poll) = bounded(1);
        let (proceed, continue_poll) = bounded(1);
        let (native, _endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(FixtureDecoder)?,
            1,
            native,
            PanicAudio {
                entered,
                proceed: continue_poll,
            },
        )?;
        at_poll.recv_timeout(Duration::from_secs(1))?;
        let client = runtime.client();
        let mut reply = client.start(settings(), None, false, false)?;
        proceed.send(())?;
        assert!(runtime.join().is_err());
        assert!(matches!(reply.try_recv(), Ok(Err(_))));
        assert!(!client.snapshot()?.running);
        assert!(matches!(
            client.start(settings(), None, false, false),
            Err(MailboxError::Closed)
        ));
        Ok(())
    }

    #[test]
    fn sleep_then_wake_rejects_a_queued_start_without_creating_a_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_send_audio, receive_audio) = bounded(3);
        let (started, did_start) = bounded(2);
        let (stopped, _did_stop) = bounded(2);
        let (entered, at_poll) = bounded(1);
        let (continue_tx, continue_poll) = bounded(1);
        let armed = Arc::new(AtomicBool::new(false));
        let (native, _endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(FixtureDecoder)?,
            1,
            native,
            ControlledAudio {
                events: receive_audio,
                started,
                stopped,
                closed: Arc::new(AtomicBool::new(false)),
                gate: Some((Arc::clone(&armed), entered, continue_poll)),
                progress: None,
            },
        )?;
        let client = runtime.client();
        wait_for(&client, |state| state.model_ready)?;
        armed.store(true, Ordering::Release);
        at_poll.recv_timeout(Duration::from_secs(1))?;
        let reply = client.start(settings(), None, false, false)?;
        client.suspend()?;
        client.resume()?;
        continue_tx.send(())?;
        assert!(
            accepted(reply).is_err(),
            "pre-sleep start must not be replayed after wake"
        );
        assert_eq!(client.snapshot()?.session.phase, Phase::Idle);
        assert_eq!(client.snapshot()?.session.session, None);
        assert!(matches!(did_start.try_recv(), Err(TryRecvError::Empty)));
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(did_start.recv_timeout(Duration::from_secs(1))?, 1);
        runtime.join()?;
        Ok(())
    }

    #[test]
    fn actor_runs_capture_inference_and_tagged_delivery_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let (send_audio, receive_audio) = bounded(3);
        let (started, did_start) = bounded(2);
        let (stopped, did_stop) = bounded(2);
        let closed = Arc::new(AtomicBool::new(false));
        let (native, endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(FixtureDecoder)?,
            1,
            native,
            ControlledAudio {
                events: receive_audio,
                started,
                stopped,
                closed: Arc::clone(&closed),
                gate: None,
                progress: None,
            },
        )?;
        let client = runtime.client();
        wait_for(&client, |state| state.model_ready)?;
        accepted(client.start(settings(), None, true, false)?)?;
        match endpoint.requests.recv_timeout(Duration::from_secs(1))? {
            NativeCommand::Capture { session, .. } => assert_eq!(session.get(), 1),
            _ => return Err("target capture was not first".into()),
        }
        assert!(matches!(did_start.try_recv(), Err(TryRecvError::Empty)));
        endpoint.responses.send(NativeResult::Captured {
            session: 1,
            result: Ok(()),
        })?;
        assert_eq!(did_start.recv_timeout(Duration::from_secs(1))?, 1);
        send_audio.send(CaptureEvent::Started {
            session: 1,
            format: AudioFormat {
                sample_rate: 16_000,
                channels: 1,
            },
            started: Instant::now(),
        })?;
        wait_for(&client, |state| state.session.phase == Phase::Recording)?;
        client.stop(1)?;
        assert_eq!(did_stop.recv_timeout(Duration::from_secs(1))?, 1);
        send_audio.send(CaptureEvent::Finished {
            session: 1,
            result: Ok(vec![0.1; 800]),
            maximum: false,
        })?;
        match endpoint.requests.recv_timeout(Duration::from_secs(1))? {
            NativeCommand::Deliver {
                session,
                text,
                token,
            } => {
                assert_eq!(session, 1);
                assert_eq!(text, "fixture words");
                token.try_commit()?;
            }
            _ => return Err("delivery missing".into()),
        }
        endpoint.responses.send(NativeResult::Delivered {
            session: 1,
            result: DeliveryResult::Sent,
        })?;
        let finished = wait_for(&client, |state| {
            state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Sent)
        })?;
        assert_eq!(finished.session.text.as_deref(), Some("fixture words"));
        accepted(client.clear_text()?)?;
        let cleared = client.snapshot()?;
        assert!(cleared.revision > finished.revision);
        assert_eq!(cleared.session.text, None);
        assert_eq!(cleared.session.phase, Phase::Idle);
        assert_eq!(cleared.session.outcome, Some(Outcome::Sent));
        runtime.join()?;
        assert!(closed.load(Ordering::Acquire));
        assert!(!client.snapshot()?.running);
        Ok(())
    }

    #[test]
    fn cancel_before_target_reply_revokes_immediately_and_never_starts_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_send_audio, receive_audio) = bounded(3);
        let (started, did_start) = bounded(2);
        let (stopped, _did_stop) = bounded(2);
        let (native, endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(FixtureDecoder)?,
            1,
            native,
            ControlledAudio {
                events: receive_audio,
                started,
                stopped,
                closed: Arc::new(AtomicBool::new(false)),
                gate: None,
                progress: None,
            },
        )?;
        let client = runtime.client();
        wait_for(&client, |state| state.model_ready)?;
        accepted(client.start(settings(), None, true, false)?)?;
        let token = match endpoint.requests.recv_timeout(Duration::from_secs(1))? {
            NativeCommand::Capture { token, .. } => token,
            _ => return Err("capture missing".into()),
        };
        client.cancel(1)?;
        assert!(!token.is_pending());
        wait_for(&client, |state| {
            state.session.phase == Phase::Idle && state.session.outcome == Some(Outcome::Cancelled)
        })?;
        endpoint.responses.send(NativeResult::Captured {
            session: 1,
            result: Ok(()),
        })?;
        accepted(client.start(settings(), None, false, false)?)?;
        assert_eq!(did_start.recv_timeout(Duration::from_secs(1))?, 2);
        assert!(matches!(did_start.try_recv(), Err(TryRecvError::Empty)));
        runtime.join()?;
        Ok(())
    }
    #[test]
    fn cancel_during_poll_prevents_ready_target_reply_from_starting_audio()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_send_audio, receive_audio) = bounded(3);
        let (started, did_start) = bounded(2);
        let (stopped, _did_stop) = bounded(4);
        let (entered, at_poll) = bounded(1);
        let (resume, continue_poll) = bounded(1);
        let armed = Arc::new(AtomicBool::new(false));
        let (native, endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            InferenceService::with_decoder(FixtureDecoder)?,
            1,
            native,
            ControlledAudio {
                events: receive_audio,
                started,
                stopped,
                closed: Arc::new(AtomicBool::new(false)),
                gate: Some((Arc::clone(&armed), entered, continue_poll)),
                progress: None,
            },
        )?;
        let client = runtime.client();
        wait_for(&client, |state| state.model_ready)?;
        accepted(client.start(settings(), None, true, false)?)?;
        assert!(matches!(
            endpoint.requests.recv_timeout(Duration::from_secs(1))?,
            NativeCommand::Capture { .. }
        ));
        armed.store(true, Ordering::Release);
        at_poll.recv_timeout(Duration::from_secs(1))?;
        endpoint.responses.send(NativeResult::Captured {
            session: 1,
            result: Ok(()),
        })?;
        client.cancel(1)?;
        resume.send(())?;
        wait_for(&client, |state| state.session.phase == Phase::Idle)?;
        assert!(matches!(did_start.try_recv(), Err(TryRecvError::Empty)));
        runtime.join()?;
        Ok(())
    }
    struct ReleaseOwners {
        gates: Vec<Sender<()>>,
        join: Option<JoinHandle<()>>,
    }
    impl Drop for ReleaseOwners {
        fn drop(&mut self) {
            for gate in &self.gates {
                if gate.try_send(()).is_err() { /* already released or owner gone */ }
            }
            if let Some(join) = self.join.take()
                && join.join().is_err()
            { /* result is observed through the test channel */
            }
        }
    }
    struct HeldDecoder {
        entered: Sender<()>,
        release: Receiver<()>,
        dropped: Arc<AtomicBool>,
    }
    impl Decoder for HeldDecoder {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            self.entered
                .send(())
                .map_err(|_| InferenceError::InvalidAudio)?;
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| InferenceError::InvalidAudio)?;
            Ok("cancelled owner result".into())
        }
    }
    impl Drop for HeldDecoder {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }
    struct PanicOwnedAudio {
        armed: Arc<AtomicBool>,
        entered: Sender<()>,
        trigger: Receiver<()>,
        shutdown_entered: Sender<()>,
        owner: Option<JoinHandle<()>>,
        dropped: Arc<AtomicBool>,
        cleanup_failure: Option<bool>,
        fault_reached: Arc<AtomicBool>,
    }
    impl AudioDriver for PanicOwnedAudio {
        fn start(&mut self, _: u64, _: Option<String>) -> Result<(), String> {
            Ok(())
        }
        fn stop(&self, _: u64) {}
        fn poll(&mut self) -> Vec<CaptureEvent> {
            if self.armed.swap(false, Ordering::AcqRel) {
                assert!(self.entered.send(()).is_ok());
                assert!(
                    self.trigger.recv_timeout(Duration::from_secs(2)).is_err(),
                    "injected controller panic"
                );
            }
            Vec::new()
        }
        fn shutdown(&mut self) -> Result<(), String> {
            self.shutdown_entered
                .send(())
                .map_err(|_| "shutdown observer closed")?;
            self.owner
                .take()
                .ok_or("owner missing")?
                .join()
                .map_err(|_| "audio owner failed".to_owned())?;
            if let Some(panic) = self.cleanup_failure {
                self.fault_reached.store(true, Ordering::Release);
                assert!(!panic, "injected audio shutdown panic");
                return Err("injected audio shutdown error".into());
            }
            Ok(())
        }
    }
    impl Drop for PanicOwnedAudio {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }
    fn panic_cleanup_contract(
        cleanup_failure: Option<bool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (release_audio, audio_release) = bounded(1);
        let (release_decoder, decoder_release) = bounded(1);
        let (trigger, panic_trigger) = bounded(1);
        // Every early error releases all gates; all fixture workers also have
        // bounded waits. Join is retained by this cleanup guard once spawned.
        let mut cleanup = ReleaseOwners {
            gates: vec![
                release_audio.clone(),
                release_decoder.clone(),
                trigger.clone(),
            ],
            join: None,
        };
        let (decoder_entered, decoding) = bounded(1);
        let decoder_dropped = Arc::new(AtomicBool::new(false));
        let inference = InferenceService::with_decoder(HeldDecoder {
            entered: decoder_entered,
            release: decoder_release,
            dropped: Arc::clone(&decoder_dropped),
        })?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference.ready() {
            if Instant::now() >= deadline {
                return Err("decoder not ready".into());
            }
            thread::yield_now();
        }
        inference.submit(crate::inference_service::Job {
            session: 99,
            generation: 1,
            job_id: 99,
            sample_range: 0..800,
            timeline_range: 0..800,
            audio: Arc::new(vec![0.1; 800]),
            language: Language::German,
            token: WorkToken::default(),
        })?;
        decoding.recv_timeout(Duration::from_secs(1))?;
        let audio_stopped = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&audio_stopped);
        let owner = thread::spawn(move || {
            if audio_release.recv_timeout(Duration::from_secs(5)).is_err() { /* bounded fallback on test failure */
            }
            observed.store(true, Ordering::Release);
        });
        let audio_dropped = Arc::new(AtomicBool::new(false));
        let fault_reached = Arc::new(AtomicBool::new(false));
        let armed = Arc::new(AtomicBool::new(false));
        let (entered, at_poll) = bounded(1);
        let (shutdown_entered, shutting_down) = bounded(1);
        let (native, _endpoint) = native_mailbox();
        let runtime = Runtime::with_audio(
            inference,
            1,
            native,
            PanicOwnedAudio {
                armed: Arc::clone(&armed),
                entered,
                trigger: panic_trigger,
                shutdown_entered,
                owner: Some(owner),
                dropped: Arc::clone(&audio_dropped),
                cleanup_failure,
                fault_reached: Arc::clone(&fault_reached),
            },
        )?;
        let client = runtime.client();
        wait_for(&client, |s| s.model_ready)?;
        accepted(client.start(settings(), None, false, false)?)?;
        armed.store(true, Ordering::Release);
        at_poll.recv_timeout(Duration::from_secs(1))?;
        let queued_start = client.start(settings(), None, false, false)?;
        let queued_clear = client.clear_text()?;
        trigger.send(())?;
        let start_rejected = acknowledged(queued_start)?.is_err();
        let clear_rejected = acknowledged(queued_clear)?.is_err();
        let admission_closed = matches!(
            client.start(settings(), None, false, false),
            Err(MailboxError::Closed)
        );
        let (finished, joined) = bounded(1);
        cleanup.join = Some(thread::spawn(move || {
            if finished.send(runtime.join()).is_err() { /* test caller stopped waiting */ }
        }));
        let early = joined.recv_timeout(Duration::from_millis(100)).ok();
        let shutdown_called = shutting_down
            .recv_timeout(Duration::from_millis(100))
            .is_ok();
        release_audio.try_send(())?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !audio_stopped.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }
        let after_audio = joined.recv_timeout(Duration::from_millis(100)).ok();
        release_decoder.try_send(())?;
        let result = match early.as_ref().or(after_audio.as_ref()) {
            Some(_) => None,
            None => Some(joined.recv_timeout(Duration::from_secs(2))?),
        };
        drop(cleanup);
        assert!(
            start_rejected && clear_rejected && admission_closed,
            "admission/acknowledgements waited for blocked owners"
        );
        assert!(
            early.is_none(),
            "Runtime::join returned while both owners were blocked"
        );
        assert!(shutdown_called, "panic bypassed audio shutdown");
        assert!(
            after_audio.is_none(),
            "Runtime::join returned before held decoder shutdown/drop"
        );
        assert!(matches!(result, Some(Err(RuntimeError::Worker))));
        assert!(audio_stopped.load(Ordering::Acquire));
        assert!(audio_dropped.load(Ordering::Acquire));
        assert!(decoder_dropped.load(Ordering::Acquire));
        assert_eq!(
            fault_reached.load(Ordering::Acquire),
            cleanup_failure.is_some()
        );
        Ok(())
    }
    #[test]
    fn controller_panic_closes_admission_before_joining_both_blocked_owners()
    -> Result<(), Box<dyn std::error::Error>> {
        panic_cleanup_contract(None)
    }
    #[test]
    fn controller_panic_keeps_original_error_and_joins_inference_after_audio_shutdown_error()
    -> Result<(), Box<dyn std::error::Error>> {
        panic_cleanup_contract(Some(false))
    }
    #[test]
    fn controller_panic_keeps_original_error_and_joins_inference_after_audio_shutdown_panic()
    -> Result<(), Box<dyn std::error::Error>> {
        panic_cleanup_contract(Some(true))
    }
    mod maximum_contract;
    mod restart_contract;
}
