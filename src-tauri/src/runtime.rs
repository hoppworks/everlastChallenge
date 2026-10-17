//! Serial host actor connecting capture, inference and the main-thread bridge.
use crate::{
    audio::{CaptureEvent, CaptureHandle},
    control::{self, Control, ControlTx, MailboxError},
    controller::{Action, Controller, Event, SessionSettings, SessionSnapshot},
    inference_service::{InferenceService, WorkToken},
};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use dictation_core::session::{DeliveryResult, Phase};
use macos_integration::{delivery::DeliveryToken, target::ProcessIdentity};
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

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
}

struct StartRequest {
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
    pub error: Option<String>,
}

struct Shared {
    closing: bool,
    snapshot: RuntimeSnapshot,
    cancellation: Option<(u64, DeliveryToken, WorkToken)>,
}

struct RuntimeExit(Arc<Mutex<Shared>>);
impl Drop for RuntimeExit {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.0.lock() {
            if let Some((_, delivery, work)) = shared.cancellation.take() {
                delivery.cancel();
                work.cancel();
            }
            shared.closing = true;
            shared.snapshot.running = false;
            shared.snapshot.model_ready = false;
            shared.snapshot.revision = shared.snapshot.revision.saturating_add(1);
        }
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
}

#[derive(Clone)]
pub struct Client {
    control: ControlTx<Request>,
    shared: Arc<Mutex<Shared>>,
}

impl Client {
    pub fn start(
        &self,
        settings: SessionSettings,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<oneshot::Receiver<Result<(), String>>, MailboxError> {
        let (reply, receiver) = oneshot::channel();
        self.control.request(Request::Start(StartRequest {
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
        self.control.stop(session)
    }
    pub fn cancel(&self, session: u64) -> Result<(), RuntimeError> {
        // Revoke before scheduling: native send cannot outrun a queued cancel.
        let shared = self
            .shared
            .lock()
            .map_err(|_| RuntimeError::Synchronization)?;
        if let Some((active, delivery, work)) = &shared.cancellation
            && *active == session
        {
            delivery.cancel();
            work.cancel();
        }
        drop(shared);
        self.control.cancel(session).map_err(RuntimeError::from)
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
    fn shutdown(&mut self) -> Result<(), String>;
}

#[derive(Default)]
struct CpalAudio {
    active: Option<(u64, CaptureHandle)>,
}
impl AudioDriver for CpalAudio {
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
        let finished = events
            .iter()
            .any(|event| matches!(event, CaptureEvent::Finished { .. }));
        if finished || handle.is_finished() {
            // Close the drain/is_finished race: Finished may have been sent
            // after the first drain but before is_finished became true.
            while let Some(event) = handle.next_event() {
                events.push(event);
            }
            let finished = events
                .iter()
                .any(|event| matches!(event, CaptureEvent::Finished { .. }));
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
                    if let CaptureEvent::Finished { result, .. } = event {
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
        let shared = Arc::new(Mutex::new(Shared {
            closing: false,
            snapshot: RuntimeSnapshot {
                revision: 1,
                running: true,
                model_ready: false,
                session: controller.snapshot(),
                maximum_warning: false,
                error: None,
            },
            cancellation: None,
        }));
        let (control, requests) = control::mailbox(16);
        let client = Client {
            control,
            shared: Arc::clone(&shared),
        };
        let worker = thread::Builder::new()
            .name("dictation-controller".into())
            .spawn(move || {
                let _exit = RuntimeExit(Arc::clone(&shared));
                let mut actor = Actor {
                    controller,
                    inference: Some(inference),
                    generation,
                    audio,
                    native,
                    shared,
                    maximum_warning: false,
                    error: None,
                    capture_pending: None,
                    deferred_audio: None,
                };
                let result = (|| { loop {
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
                            actor.audio.shutdown().map_err(RuntimeError::Service)?;
                            if let Some(inference) = actor.inference.take() {
                                inference.shutdown().map_err(service_error)?;
                            }
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
                        Some(Control::Request(Request::Start(request))) => {
                            let actions = actor.start_request(
                                request.settings, request.process, request.delivery, request.toggle,
                            );
                            let result = match actions {
                                Ok(actions) => {
                                    actor.error = None;
                                    actor.maximum_warning = false;
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
                } })();
                // All paths release native audio and join inference here,
                // outside the GUI, including controller/service errors.
                let audio_cleanup = actor.audio.shutdown().map_err(RuntimeError::Service);
                let inference_cleanup = match actor.inference.take() {
                    Some(service) => service.shutdown().map_err(service_error),
                    None => Ok(()),
                };
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
    error: Option<String>,
    capture_pending: Option<(u64, Instant)>,
    deferred_audio: Option<(u64, Option<String>)>,
}

impl<A: AudioDriver> Actor<A> {
    fn start_request(
        &mut self,
        settings: SessionSettings,
        process: Option<ProcessIdentity>,
        delivery: bool,
        toggle: bool,
    ) -> Result<Vec<Action>, crate::controller::ControllerError> {
        let shared = Arc::clone(&self.shared);
        let mut admission = shared
            .lock()
            .map_err(|_| crate::controller::ControllerError::Resource)?;
        if admission.closing {
            return Err(dictation_core::session::SessionError::NotReady.into());
        }
        let snapshot = self.controller.snapshot();
        let actions = if toggle && matches!(snapshot.phase, Phase::Starting | Phase::Recording) {
            self.controller.stop(
                snapshot
                    .session
                    .ok_or(crate::controller::ControllerError::Resource)?,
            )
        } else {
            let ready = self.inference.as_ref().is_some_and(InferenceService::ready)
                && settings.generation == self.generation;
            self.controller.start(settings, process, ready, delivery)
        }?;
        // Register before dispatch: cancellation and shutdown can see a session
        // even while its first native command is being sent.
        admission.cancellation = self.controller.cancellation_handles();
        Ok(actions)
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
        // Result adoption and immediate revocation have a single ordering point.
        // The lock covers only state mutation, never inference or device I/O.
        let shared = Arc::clone(&self.shared);
        let _admission = shared
            .lock()
            .map_err(|_| crate::controller::ControllerError::Resource)?;
        self.controller.accept(event)
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
                    if admission.closing
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
                CaptureEvent::Started { session, .. } => Event::AudioStarted(session),
                CaptureEvent::Finished {
                    session, result, ..
                } => Event::AudioFinished {
                    session,
                    result: result.map_err(|error| error.to_string()),
                },
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
        if let Some(service) = &self.inference
            && let Some(done) = service.take_result().map_err(service_error)?
        {
            let actions = self
                .accept(Event::InferenceFinished {
                    session: done.session,
                    generation: done.generation,
                    job_id: done.job_id,
                    sample_range: done.sample_range,
                    result: done.result.map_err(|error| error.to_string()),
                })
                .map_err(service_error)?;
            self.execute(actions)?;
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
        let mut next = RuntimeSnapshot {
            revision: shared.snapshot.revision,
            running: true,
            model_ready: self.inference.as_ref().is_some_and(InferenceService::ready),
            session: self.controller.snapshot(),
            maximum_warning: self.maximum_warning,
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
        mut reply: oneshot::Receiver<Result<(), String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match reply.try_recv() {
                Ok(result) => return result.map_err(Into::into),
                Err(oneshot::error::TryRecvError::Empty) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(1))
                }
                _ => return Err("request acknowledgement missing".into()),
            }
        }
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
}
