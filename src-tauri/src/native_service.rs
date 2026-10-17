//! Serial native orchestration. AX has its own owner; AppKit stays on the main thread.
use crate::runtime::{NativeCommand, NativeEndpoint, NativeResult};
use crossbeam_channel::{Sender, bounded};
use dictation_core::session::DeliveryResult;
use macos_integration::{
    clipboard::{Snapshot, capture_bounded},
    delivery::DeliveryToken,
    main_delivery::{self, Command, Environment, Reply},
    target::{
        ProcessIdentity, TargetApproval,
        worker::{TargetEvent, TargetWorker},
    },
};
use std::{
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const STEP_LIMIT: Duration = Duration::from_secs(2);
const MODIFIER_LIMIT: Duration = Duration::from_secs(1);
const RESTORE_DELAY: Duration = Duration::from_millis(500);
const TICK: Duration = Duration::from_millis(10);

type NativeError = String;

/// Schedule this request once with AppHandle::run_on_main_thread, calling execute there.
/// At most one scheduled request remains outstanding, even after its caller times out.
struct MainSlot {
    occupied: Arc<AtomicBool>,
    released: bool,
}
impl MainSlot {
    fn release(&mut self) {
        if !self.released {
            self.occupied.store(false, Ordering::Release);
            self.released = true;
        }
    }
}
impl Drop for MainSlot {
    fn drop(&mut self) {
        self.release();
    }
}

pub struct MainRequest {
    command: Command,
    reply: Sender<Result<Reply, NativeError>>,
    permit: Arc<AtomicBool>,
    completion: Arc<Mutex<()>>,
    slot: MainSlot,
    cleanup: Option<NonZeroU64>,
}
impl MainRequest {
    pub fn execute(self) {
        let Self {
            command,
            reply,
            permit,
            completion,
            mut slot,
            cleanup,
        } = self;
        let result = if permit.load(Ordering::Acquire) {
            main_delivery::dispatch(command).map_err(|error| error.to_string())
        } else {
            Err("Main-thread request expired".into())
        };
        // Completion and abandonment share a linearization point. Once a reply
        // is visible the slot is free; otherwise this closure owns cleanup.
        let completed = completion.lock();
        if (completed.is_err()
            || !permit.load(Ordering::Acquire)
            || reply.try_send(result).is_err())
            && let Some(session) = cleanup
            && let Err(error) = main_delivery::dispatch(Command::Cleanup { session })
        {
            eprintln!("Native cleanup after abandoned request failed: {error}");
        }
        slot.release();
    }
}

type Schedule = Box<dyn Fn(MainRequest) -> Result<(), String> + Send>;

struct MainBridge {
    schedule: Schedule,
    occupied: Arc<AtomicBool>,
}
impl MainBridge {
    fn call(
        &self,
        command: Command,
        token: Option<&DeliveryToken>,
        deadline: Instant,
    ) -> Result<Reply, NativeError> {
        let cleanup = match &command {
            Command::Environment => None,
            Command::Prepare { session, .. }
            | Command::Install { session, .. }
            | Command::Send { session, .. }
            | Command::Cleanup { session } => Some(*session),
        };
        if Instant::now() >= deadline {
            return Err("Main-thread deadline expired".into());
        }
        if self.occupied.swap(true, Ordering::AcqRel) {
            return Err("Main-thread request still outstanding".into());
        }
        let (reply, receive) = bounded(1);
        let permit = Arc::new(AtomicBool::new(true));
        let completion = Arc::new(Mutex::new(()));
        let request = MainRequest {
            command,
            reply,
            permit: Arc::clone(&permit),
            completion: Arc::clone(&completion),
            slot: MainSlot {
                occupied: Arc::clone(&self.occupied),
                released: false,
            },
            cleanup,
        };
        if let Err(error) = (self.schedule)(request) {
            if let Some(token) = token {
                token.cancel();
            }
            permit.store(false, Ordering::Release);
            return Err(error);
        }
        loop {
            {
                let _completed = completion
                    .lock()
                    .map_err(|_| "Main completion lock poisoned")?;
                match receive.try_recv() {
                    Ok(result) => return result,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        if let Some(token) = token {
                            token.cancel();
                        }
                        permit.store(false, Ordering::Release);
                        return Err("Main-thread request lost".into());
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => {}
                }
                if token.is_some_and(|token| !token.is_pending() && !token.is_committed())
                    || Instant::now() >= deadline
                {
                    // Competes atomically with the final send, closing the gap
                    // between the closure's permit check and dispatch.
                    if let Some(token) = token {
                        token.cancel();
                    }
                    permit.store(false, Ordering::Release);
                    return Err("Main-thread request cancelled or timed out".into());
                }
            }
            thread::sleep(TICK.min(deadline.saturating_duration_since(Instant::now())));
        }
    }
}

trait Backend: Send + 'static {
    type Approval;
    type Backup;
    fn capture(
        &mut self,
        session: NonZeroU64,
        process: ProcessIdentity,
        token: &DeliveryToken,
    ) -> Result<(), NativeError>;
    fn environment(
        &mut self,
        token: &DeliveryToken,
        deadline: Instant,
    ) -> Result<Environment, NativeError>;
    fn approve(
        &mut self,
        session: NonZeroU64,
        process: ProcessIdentity,
        token: &DeliveryToken,
    ) -> Result<Self::Approval, NativeError>;
    fn backup(&mut self, name: &str, token: &DeliveryToken) -> Result<Self::Backup, NativeError>;
    fn prepare(
        &mut self,
        session: NonZeroU64,
        name: String,
        backup: Self::Backup,
        text: String,
        token: &DeliveryToken,
    ) -> Result<(), NativeError>;
    fn install(
        &mut self,
        session: NonZeroU64,
        approval: Self::Approval,
        token: &DeliveryToken,
    ) -> Result<(), NativeError>;
    fn send(
        &mut self,
        session: NonZeroU64,
        approval: Self::Approval,
        token: &DeliveryToken,
    ) -> Result<(), NativeError>;
    fn cleanup(&mut self, session: NonZeroU64) -> Result<(), NativeError>;
    fn shutdown(self) -> Result<(), NativeError>;
}

struct NativeBackend {
    target: TargetWorker,
    main: MainBridge,
    helper: PathBuf,
    next_request: u64,
}
impl NativeBackend {
    fn event(&self, token: &DeliveryToken, deadline: Instant) -> Result<TargetEvent, NativeError> {
        loop {
            if !token.is_pending() {
                return Err("Target request cancelled".into());
            }
            if let Some(event) = self
                .target
                .next_event()
                .map_err(|error| error.to_string())?
            {
                return Ok(event);
            }
            if Instant::now() >= deadline {
                return Err("Target request timed out".into());
            }
            thread::sleep(TICK);
        }
    }
    fn unit(&self, command: Command, token: Option<&DeliveryToken>) -> Result<(), NativeError> {
        match self
            .main
            .call(command, token, Instant::now() + STEP_LIMIT)?
        {
            Reply::Prepared | Reply::Installed | Reply::Sent | Reply::Cleaned(_) => Ok(()),
            Reply::Environment(_) => Err("Unexpected main-thread response".into()),
        }
    }
}
impl Backend for NativeBackend {
    type Approval = TargetApproval;
    type Backup = Snapshot;
    fn capture(
        &mut self,
        session: NonZeroU64,
        process: ProcessIdentity,
        token: &DeliveryToken,
    ) -> Result<(), NativeError> {
        self.target
            .capture(session, process, token.clone())
            .map_err(|error| error.to_string())?;
        // Up to eight old replies may remain after a cancelled wait.
        let deadline = Instant::now() + STEP_LIMIT;
        for _ in 0..9 {
            if let TargetEvent::Captured {
                session: id,
                result,
            } = self.event(token, deadline)?
                && id == session
            {
                return result.map_err(|error| error.to_string());
            }
        }
        Err("Target capture reply did not match session".into())
    }
    fn environment(
        &mut self,
        token: &DeliveryToken,
        deadline: Instant,
    ) -> Result<Environment, NativeError> {
        match self
            .main
            .call(Command::Environment, Some(token), deadline)?
        {
            Reply::Environment(environment) => Ok(environment),
            _ => Err("Missing native environment".into()),
        }
    }
    fn approve(
        &mut self,
        session: NonZeroU64,
        process: ProcessIdentity,
        token: &DeliveryToken,
    ) -> Result<Self::Approval, NativeError> {
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or("Native request identifiers exhausted")?;
        let request =
            NonZeroU64::new(self.next_request).ok_or("Invalid native request identifier")?;
        self.target
            .approve(session, request, process)
            .map_err(|error| error.to_string())?;
        let deadline = Instant::now() + STEP_LIMIT;
        for _ in 0..9 {
            if let TargetEvent::Approved {
                session: id,
                request: tag,
                result,
            } = self.event(token, deadline)?
                && id == session
                && tag == request
            {
                return result.map_err(|error| error.to_string());
            }
        }
        Err("Target approval did not match request".into())
    }
    fn backup(&mut self, name: &str, token: &DeliveryToken) -> Result<Snapshot, NativeError> {
        capture_bounded(&self.helper, name, token).map_err(|error| error.to_string())
    }
    fn prepare(
        &mut self,
        session: NonZeroU64,
        name: String,
        backup: Snapshot,
        text: String,
        token: &DeliveryToken,
    ) -> Result<(), NativeError> {
        self.unit(
            Command::Prepare {
                session,
                name,
                backup,
                text,
                token: token.clone(),
            },
            Some(token),
        )
    }
    fn install(
        &mut self,
        session: NonZeroU64,
        approval: TargetApproval,
        token: &DeliveryToken,
    ) -> Result<(), NativeError> {
        self.unit(Command::Install { session, approval }, Some(token))
    }
    fn send(
        &mut self,
        session: NonZeroU64,
        approval: TargetApproval,
        token: &DeliveryToken,
    ) -> Result<(), NativeError> {
        self.unit(Command::Send { session, approval }, Some(token))
    }
    fn cleanup(&mut self, session: NonZeroU64) -> Result<(), NativeError> {
        self.unit(Command::Cleanup { session }, None)
    }
    fn shutdown(self) -> Result<(), NativeError> {
        self.target.shutdown().map_err(|error| error.to_string())
    }
}

pub struct NativeService {
    stopping: Arc<AtomicBool>,
    token: Arc<Mutex<Option<DeliveryToken>>>,
    worker: Option<JoinHandle<Result<(), NativeError>>>,
}
impl NativeService {
    /// Uses the application event loop; call after Tauri setup has an AppHandle.
    pub fn for_app(endpoint: NativeEndpoint, app: tauri::AppHandle) -> Result<Self, String> {
        let helper = std::env::current_exe().map_err(|error| error.to_string())?;
        Self::spawn(endpoint, helper, move |request| {
            app.run_on_main_thread(move || request.execute())
                .map_err(|error| error.to_string())
        })
    }

    pub fn spawn(
        endpoint: NativeEndpoint,
        helper: PathBuf,
        schedule: impl Fn(MainRequest) -> Result<(), String> + Send + 'static,
    ) -> Result<Self, String> {
        let backend = NativeBackend {
            target: TargetWorker::start().map_err(|error| error.to_string())?,
            main: MainBridge {
                schedule: Box::new(schedule),
                occupied: Arc::new(AtomicBool::new(false)),
            },
            helper,
            next_request: 0,
        };
        Self::with_backend(endpoint, backend)
    }
    fn with_backend<B: Backend>(endpoint: NativeEndpoint, backend: B) -> Result<Self, String> {
        let stopping = Arc::new(AtomicBool::new(false));
        let token = Arc::new(Mutex::new(None));
        let mut owner = Owner {
            backend,
            endpoint,
            stopping: Arc::clone(&stopping),
            token: Arc::clone(&token),
            restore: None,
            captured: None,
        };
        let worker = thread::Builder::new()
            .name("native-delivery".into())
            .spawn(move || {
                let result = owner.run();
                let cleanup = owner.cleanup();
                let shutdown = owner.backend.shutdown();
                result.and(cleanup).and(shutdown)
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            stopping,
            token,
            worker: Some(worker),
        })
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Ok(token) = self.token.lock()
            && let Some(token) = token.as_ref()
        {
            token.cancel();
        }
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// Call off the GUI thread; its event loop must remain available for cleanup.
    pub fn join(mut self) -> Result<(), String> {
        self.stop();
        self.worker
            .take()
            .ok_or("Native worker missing")?
            .join()
            .map_err(|_| "Native worker panicked")?
    }
}
impl Drop for NativeService {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Captured {
    session: NonZeroU64,
    token: DeliveryToken,
    valid: bool,
}

struct Owner<B: Backend> {
    backend: B,
    endpoint: NativeEndpoint,
    stopping: Arc<AtomicBool>,
    token: Arc<Mutex<Option<DeliveryToken>>>,
    restore: Option<(NonZeroU64, Instant)>,
    captured: Option<Captured>,
}
impl<B: Backend> Owner<B> {
    fn check(&self, token: &DeliveryToken) -> Result<(), NativeError> {
        if self.stopping.load(Ordering::Acquire) || !token.is_pending() {
            Err("Native delivery cancelled".into())
        } else {
            Ok(())
        }
    }
    fn track(&self, token: &DeliveryToken) -> Result<(), NativeError> {
        let mut current = self
            .token
            .lock()
            .map_err(|_| "Native token lock poisoned")?;
        *current = Some(token.clone());
        self.check(token)
    }
    fn cleanup(&mut self) -> Result<(), NativeError> {
        if let Some((session, _)) = self.restore {
            self.backend.cleanup(session)?;
            self.restore = None;
        }
        Ok(())
    }
    fn environment(
        &mut self,
        token: &DeliveryToken,
        deadline: Instant,
    ) -> Result<Environment, NativeError> {
        self.check(token)?;
        let environment = self.backend.environment(token, deadline)?;
        self.check(token)?;
        if !environment.accessibility_trusted {
            return Err("Accessibility permission missing".into());
        }
        if environment.secure_input {
            return Err("Secure input active".into());
        }
        Ok(environment)
    }
    fn deliver(
        &mut self,
        session: NonZeroU64,
        text: String,
        token: &DeliveryToken,
    ) -> Result<(), NativeError> {
        if !self.captured.as_ref().is_some_and(|capture| {
            capture.valid && capture.session == session && capture.token.same_work(token)
        }) {
            return Err("No matching frozen start target".into());
        }
        self.cleanup()?;
        self.track(token)?;
        if token.session() != session || text.is_empty() || text.len() > 256 * 1024 {
            return Err("Invalid native delivery".into());
        }
        let deadline = Instant::now() + MODIFIER_LIMIT;
        let environment = loop {
            let environment = self.environment(token, deadline)?;
            if !environment.modifiers_held {
                break environment;
            }
            if Instant::now() >= deadline {
                return Err("Physical modifier keys remain held".into());
            }
            thread::sleep(TICK.min(deadline.saturating_duration_since(Instant::now())));
        };
        // Validate the frozen field before asking any clipboard provider for data.
        self.backend.approve(session, environment.process, token)?;
        let backup = self.backend.backup(&environment.pasteboard_name, token)?;
        self.check(token)?;
        // From here even an unacknowledged preparation must be cleaned up.
        self.restore = Some((session, Instant::now()));
        self.backend
            .prepare(session, environment.pasteboard_name, backup, text, token)?;
        let environment = self.environment(token, Instant::now() + STEP_LIMIT)?;
        let approval = self.backend.approve(session, environment.process, token)?;
        self.check(token)?;
        self.backend.install(session, approval, token)?;
        let environment = self.environment(token, Instant::now() + STEP_LIMIT)?;
        let approval = self.backend.approve(session, environment.process, token)?;
        self.check(token)?;
        self.backend.send(session, approval, token)?;
        self.restore = Some((session, Instant::now() + RESTORE_DELAY));
        Ok(())
    }
    fn run(&mut self) -> Result<(), NativeError> {
        while !self.stopping.load(Ordering::Acquire) {
            if self
                .restore
                .is_some_and(|(_, deadline)| Instant::now() >= deadline)
            {
                self.cleanup()?;
            }
            let command = match self.endpoint.requests.recv_timeout(TICK) {
                Ok(command) => command,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let response = match command {
                NativeCommand::Capture {
                    session,
                    process,
                    token,
                } => {
                    let result = if self
                        .captured
                        .as_ref()
                        .is_some_and(|capture| capture.session >= session)
                    {
                        Err("Start target already fixed for this session".into())
                    } else {
                        let result = self.track(&token).and_then(|()| {
                            if token.session() != session {
                                return Err("Target token does not match session".into());
                            }
                            let process = process.ok_or("Start target not captured")?;
                            self.backend.capture(session, process, &token)
                        });
                        self.captured = Some(Captured {
                            session,
                            token,
                            valid: result.is_ok(),
                        });
                        result
                    };
                    NativeResult::Captured {
                        session: session.get(),
                        result,
                    }
                }
                NativeCommand::Deliver {
                    session,
                    text,
                    token,
                } => {
                    // A cancelled newer request must not terminate the service or
                    // shorten an older sent lease's restore interval.
                    let waited = (|| {
                        while self
                            .restore
                            .is_some_and(|(_, deadline)| Instant::now() < deadline)
                        {
                            self.check(&token)?;
                            thread::sleep(TICK);
                        }
                        Ok::<_, String>(())
                    })();
                    let result = waited.and_then(|()| match NonZeroU64::new(session) {
                        Some(id) => self.deliver(id, text, &token),
                        None => Err("Invalid native session".into()),
                    });
                    let result = match result {
                        Ok(()) => DeliveryResult::Sent,
                        Err(error) => {
                            let cleanup = if self.restore.is_some_and(|(id, _)| id.get() == session)
                            {
                                self.cleanup()
                            } else {
                                Ok(())
                            };
                            match cleanup {
                                Ok(()) if token.is_committed() => DeliveryResult::Failed(format!(
                                    "Send committed but completion could not be confirmed: {error}"
                                )),
                                Ok(()) => DeliveryResult::Held(error),
                                Err(cleanup) => {
                                    DeliveryResult::Failed(format!("{error}; cleanup: {cleanup}"))
                                }
                            }
                        }
                    };
                    NativeResult::Delivered { session, result }
                }
                NativeCommand::Cancel(session) => {
                    if self.restore.is_some_and(|(id, _)| id.get() == session) {
                        self.cleanup()?;
                    }
                    continue;
                }
            };
            self.endpoint
                .responses
                .try_send(response)
                .map_err(|error| format!("Native response queue: {error}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct Trace {
        actions: Mutex<Vec<String>>,
        held: AtomicBool,
        cancel_backup: AtomicBool,
        approval: AtomicUsize,
    }
    impl Trace {
        fn record(&self, action: impl Into<String>) {
            if let Ok(mut actions) = self.actions.lock() {
                actions.push(action.into());
            }
        }
        fn actions(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
            Ok(self.actions.lock().map_err(|_| "trace poisoned")?.clone())
        }
    }
    struct Fixture(Arc<Trace>);
    fn process() -> Result<ProcessIdentity, String> {
        ProcessIdentity::read(std::process::id() as i32).map_err(|error| error.to_string())
    }
    impl Backend for Fixture {
        type Approval = usize;
        type Backup = ();
        fn capture(
            &mut self,
            _: NonZeroU64,
            _: ProcessIdentity,
            _: &DeliveryToken,
        ) -> Result<(), String> {
            self.0.record("capture");
            Ok(())
        }
        fn environment(&mut self, _: &DeliveryToken, _: Instant) -> Result<Environment, String> {
            Ok(Environment {
                process: process()?,
                modifiers_held: self.0.held.load(Ordering::Acquire),
                accessibility_trusted: true,
                secure_input: false,
                pasteboard_name: "fixture".into(),
            })
        }
        fn approve(
            &mut self,
            _: NonZeroU64,
            _: ProcessIdentity,
            _: &DeliveryToken,
        ) -> Result<usize, String> {
            let id = self.0.approval.fetch_add(1, Ordering::AcqRel) + 1;
            self.0.record(format!("approve:{id}"));
            Ok(id)
        }
        fn backup(&mut self, _: &str, token: &DeliveryToken) -> Result<(), String> {
            self.0.record("backup");
            if self.0.cancel_backup.load(Ordering::Acquire) {
                token.cancel();
            }
            Ok(())
        }
        fn prepare(
            &mut self,
            _: NonZeroU64,
            _: String,
            _: (),
            _: String,
            _: &DeliveryToken,
        ) -> Result<(), String> {
            self.0.record("prepare");
            Ok(())
        }
        fn install(
            &mut self,
            _: NonZeroU64,
            approval: usize,
            _: &DeliveryToken,
        ) -> Result<(), String> {
            self.0.record(format!("install:{approval}"));
            Ok(())
        }
        fn send(
            &mut self,
            _: NonZeroU64,
            approval: usize,
            token: &DeliveryToken,
        ) -> Result<(), String> {
            token.try_commit().map_err(|error| error.to_string())?;
            self.0.record(format!("send:{approval}"));
            Ok(())
        }
        fn cleanup(&mut self, _: NonZeroU64) -> Result<(), String> {
            self.0.record("cleanup");
            Ok(())
        }
        fn shutdown(self) -> Result<(), String> {
            self.0.record("shutdown");
            Ok(())
        }
    }

    #[test]
    fn abandoned_main_request_releases_slot_when_scheduler_drops_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let bridge = MainBridge {
            schedule: Box::new(|request| {
                drop(request);
                Ok(())
            }),
            occupied: Arc::new(AtomicBool::new(false)),
        };
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        for _ in 0..2 {
            let result = bridge.call(
                Command::Cleanup { session: id },
                None,
                Instant::now() + STEP_LIMIT,
            );
            assert!(result.is_err());
            assert!(!bridge.occupied.load(Ordering::Acquire));
        }
        Ok(())
    }
    fn service(
        trace: Arc<Trace>,
    ) -> Result<
        (
            NativeService,
            Sender<NativeCommand>,
            crossbeam_channel::Receiver<NativeResult>,
        ),
        String,
    > {
        let (commands, requests) = bounded(8);
        let (responses, results) = bounded(8);
        Ok((
            NativeService::with_backend(
                NativeEndpoint {
                    requests,
                    responses,
                },
                Fixture(trace),
            )?,
            commands,
            results,
        ))
    }
    fn deliver(
        commands: &Sender<NativeCommand>,
        session: u64,
        token: DeliveryToken,
    ) -> Result<(), Box<dyn std::error::Error>> {
        commands.send(NativeCommand::Deliver {
            session,
            text: "fixture words".into(),
            token,
        })?;
        Ok(())
    }
    fn capture(
        commands: &Sender<NativeCommand>,
        results: &crossbeam_channel::Receiver<NativeResult>,
        token: &DeliveryToken,
    ) -> Result<(), Box<dyn std::error::Error>> {
        commands.send(NativeCommand::Capture {
            session: token.session(),
            process: Some(process()?),
            token: token.clone(),
        })?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Captured { result: Ok(()), .. }
        ));
        Ok(())
    }
    #[test]
    fn delivery_uses_three_fresh_checks_and_cleans_once_on_shutdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Trace::default());
        let (service, commands, results) = service(Arc::clone(&trace))?;
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        commands.send(NativeCommand::Capture {
            session: id,
            process: Some(process()?),
            token: token.clone(),
        })?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Captured { result: Ok(()), .. }
        ));
        deliver(&commands, 1, token)?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Delivered {
                result: DeliveryResult::Sent,
                ..
            }
        ));
        service.join()?;
        assert_eq!(
            trace.actions()?,
            [
                "capture",
                "approve:1",
                "backup",
                "prepare",
                "approve:2",
                "install:2",
                "approve:3",
                "send:3",
                "cleanup",
                "shutdown"
            ]
        );
        Ok(())
    }
    #[test]
    fn cancel_during_backup_never_installs_and_service_accepts_next_capture()
    -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Trace::default());
        trace.cancel_backup.store(true, Ordering::Release);
        let (service, commands, results) = service(Arc::clone(&trace))?;
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        capture(&commands, &results, &token)?;
        deliver(&commands, 1, token)?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Delivered {
                result: DeliveryResult::Held(_),
                ..
            }
        ));
        let next = NonZeroU64::new(2).ok_or("invalid id")?;
        commands.send(NativeCommand::Capture {
            session: next,
            process: Some(process()?),
            token: DeliveryToken::new(next),
        })?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Captured {
                session: 2,
                result: Ok(()),
                ..
            }
        ));
        service.join()?;
        assert_eq!(
            trace.actions()?,
            ["capture", "approve:1", "backup", "capture", "shutdown"]
        );
        Ok(())
    }
    #[test]
    fn held_modifiers_time_out_without_reading_or_changing_clipboard()
    -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Trace::default());
        trace.held.store(true, Ordering::Release);
        let (service, commands, results) = service(Arc::clone(&trace))?;
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        capture(&commands, &results, &token)?;
        deliver(&commands, 1, token)?;
        assert!(matches!(
            results.recv_timeout(Duration::from_secs(3))?,
            NativeResult::Delivered {
                result: DeliveryResult::Held(_),
                ..
            }
        ));
        service.join()?;
        assert_eq!(trace.actions()?, ["capture", "shutdown"]);
        Ok(())
    }
    #[test]
    fn timed_out_main_request_remains_bounded_until_its_closure_is_released()
    -> Result<(), Box<dyn std::error::Error>> {
        let (queued, take) = bounded(1);
        let bridge = MainBridge {
            schedule: Box::new(move |request| {
                queued.try_send(request).map_err(|error| error.to_string())
            }),
            occupied: Arc::new(AtomicBool::new(false)),
        };
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        assert!(
            bridge
                .call(
                    Command::Cleanup { session: id },
                    None,
                    Instant::now() + TICK
                )
                .is_err()
        );
        assert!(bridge.occupied.load(Ordering::Acquire));
        assert!(
            bridge
                .call(
                    Command::Cleanup { session: id },
                    None,
                    Instant::now() + TICK
                )
                .is_err()
        );
        assert_eq!(take.len(), 1);
        drop(take.recv()?);
        assert!(!bridge.occupied.load(Ordering::Acquire));
        Ok(())
    }
    #[test]
    fn abandoned_request_revokes_the_real_send_token() -> Result<(), Box<dyn std::error::Error>> {
        let (queued, take) = bounded(1);
        let bridge = MainBridge {
            schedule: Box::new(move |request| {
                queued.try_send(request).map_err(|error| error.to_string())
            }),
            occupied: Arc::new(AtomicBool::new(false)),
        };
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        assert!(
            bridge
                .call(
                    Command::Cleanup { session: id },
                    Some(&token),
                    Instant::now() + TICK
                )
                .is_err()
        );
        assert!(matches!(
            token.try_commit(),
            Err(macos_integration::delivery::CommitError::Cancelled)
        ));
        drop(take.recv()?);
        Ok(())
    }
    #[test]
    fn successful_send_commit_is_not_mistaken_for_cancel_before_reply()
    -> Result<(), Box<dyn std::error::Error>> {
        let (queued, take) = bounded(1);
        let bridge = MainBridge {
            schedule: Box::new(move |request| {
                queued.try_send(request).map_err(|error| error.to_string())
            }),
            occupied: Arc::new(AtomicBool::new(false)),
        };
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        let caller_token = token.clone();
        let caller = thread::spawn(move || {
            bridge.call(
                Command::Cleanup { session: id },
                Some(&caller_token),
                Instant::now() + STEP_LIMIT,
            )
        });
        let request = take.recv_timeout(STEP_LIMIT)?;
        token.try_commit()?;
        thread::sleep(TICK * 3);
        let sent = request.reply.try_send(Ok(Reply::Sent));
        drop(request);
        let result = caller.join().map_err(|_| "caller panicked")?;
        assert!(matches!(result, Ok(Reply::Sent)));
        sent?;
        Ok(())
    }
    #[test]
    fn missing_start_target_cannot_be_replaced_by_a_retry_or_fresh_token()
    -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Trace::default());
        let (service, commands, results) = service(Arc::clone(&trace))?;
        let id = NonZeroU64::new(1).ok_or("invalid id")?;
        let token = DeliveryToken::new(id);
        commands.send(NativeCommand::Capture {
            session: id,
            process: None,
            token: token.clone(),
        })?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Captured { result: Err(_), .. }
        ));
        commands.send(NativeCommand::Capture {
            session: id,
            process: Some(process()?),
            token: DeliveryToken::new(id),
        })?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Captured { result: Err(_), .. }
        ));
        deliver(&commands, 1, token)?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Delivered {
                result: DeliveryResult::Held(_),
                ..
            }
        ));
        service.join()?;
        assert_eq!(trace.actions()?, ["shutdown"]);
        Ok(())
    }
    #[test]
    fn cancel_while_waiting_for_previous_restore_does_not_stop_service()
    -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Trace::default());
        let (service, commands, results) = service(Arc::clone(&trace))?;
        let token = DeliveryToken::new(NonZeroU64::new(1).ok_or("invalid id")?);
        capture(&commands, &results, &token)?;
        deliver(&commands, 1, token)?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Delivered {
                result: DeliveryResult::Sent,
                ..
            }
        ));
        let second = DeliveryToken::new(NonZeroU64::new(2).ok_or("invalid id")?);
        capture(&commands, &results, &second)?;
        second.cancel();
        deliver(&commands, 2, second)?;
        assert!(matches!(
            results.recv_timeout(STEP_LIMIT)?,
            NativeResult::Delivered {
                result: DeliveryResult::Held(_),
                ..
            }
        ));
        let third = DeliveryToken::new(NonZeroU64::new(3).ok_or("invalid id")?);
        capture(&commands, &results, &third)?;
        service.join()?;
        Ok(())
    }
}
