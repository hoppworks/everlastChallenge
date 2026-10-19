//! Opt-in local developer transport; commands execute only when polled on the main thread.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(15 * 60);
const RECORDING_LIMIT: Duration = Duration::from_secs(60);
const RESPONSE_LIMIT: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(100);
const MAX_REQUEST: usize = 1024;

pub trait Handler {
    fn inspect(&self, recipient_pid: i32) -> Result<Value, String>;
    fn status(&self) -> Result<Value, String>;
    fn start(&self, recipient_pid: i32) -> Result<(), String>;
    #[cfg(feature = "measurement")]
    fn start_host(&self, recipient_pid: i32) -> Result<(), String>;
    #[cfg(feature = "measurement")]
    fn host_observation(&self) -> Result<Value, String>;
    #[cfg(feature = "measurement")]
    fn handoff_observation(&self) -> Result<Value, String>;
    fn stop(&self, session: u64) -> Result<(), String>;
    fn cancel(&self, session: u64) -> Result<(), String>;
    fn shutdown(&self) -> Result<(), String>;
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(rename = "recipientPid")]
    recipient_pid: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    op: String,
    session: Option<u64>,
}
struct Pending {
    request: Request,
    reply: SyncSender<Value>,
    deadline: Instant,
}
struct Identity {
    path: PathBuf,
    dev: u64,
    ino: u64,
}
impl Identity {
    fn remove(&self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path)
            && meta.dev() == self.dev
            && meta.ino() == self.ino
        {
            let _result = fs::remove_file(&self.path);
        }
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        self.remove();
    }
}

/// The main thread owns admission and automatic cancellation. The background thread
/// performs bounded socket I/O and merely wakes the existing app event loop.
pub struct Control {
    requests: Receiver<Pending>,
    stopped: Arc<AtomicBool>,
    socket: Identity,
    recipient_pid: i32,
    expires: Instant,
    start_pending: bool,
    start_deadline: Option<Instant>,
    owned: Option<u64>,
    ending: bool,
}
impl Control {
    pub fn from_env(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Option<Self>, String> {
        match std::env::var_os("EVERLAST_TEST_CONTROL_DIR") {
            None => Ok(None),
            Some(path) => Self::bind(Path::new(&path), wake).map(Some),
        }
    }
    pub fn bind(directory: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self, String> {
        let config = validate_directory(directory)?;
        let path = directory.join("control.sock");
        let listener =
            UnixListener::bind(&path).map_err(|_| "test-control socket must be fresh")?;
        let meta = fs::symlink_metadata(&path).map_err(io_error)?;
        let socket = Identity {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        };
        // Binding creates a kernel-owned inode under this process's effective UID.
        // Comparing it proves ownership without a new unsafe/geteuid dependency.
        if meta.uid() != fs::symlink_metadata(directory).map_err(io_error)?.uid() {
            return Err("test-control directory is not owned by this process".into());
        }
        fs::set_permissions(&socket.path, fs::Permissions::from_mode(0o600)).map_err(io_error)?;
        listener.set_nonblocking(true).map_err(io_error)?;
        let (send, requests) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let expires = Instant::now() + TTL;
        thread::Builder::new()
            .name("test-control-transport".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) && Instant::now() < expires {
                    wake();
                    match listener.accept() {
                        Ok((stream, _)) => serve(stream, &send, &wake, &stop),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(TICK)
                        }
                        Err(_) => break,
                    }
                }
                // One final main-thread poll enforces the expiry cancellation.
                wake();
            })
            .map_err(io_error)?;
        Ok(Self {
            requests,
            stopped,
            socket,
            recipient_pid: config.recipient_pid,
            expires,
            start_pending: false,
            start_deadline: None,
            owned: None,
            ending: false,
        })
    }
    pub fn poll(&mut self, handler: &impl Handler) {
        self.poll_at(handler, Instant::now());
    }
    fn poll_at(&mut self, handler: &impl Handler, now: Instant) {
        self.observe_owner(handler);
        let expired = now >= self.expires;
        let over_recording = self.start_deadline.is_some_and(|deadline| now >= deadline);
        if expired || over_recording {
            self.ending = true;
            // Desktop revokes only its exact actor-bound test admission, including
            // a pending start whose SID has not yet reached this observer.
            if handler.shutdown().is_ok() {
                self.owned = None;
                self.start_pending = false;
            }
            if expired {
                self.stopped.store(true, Ordering::Release);
                self.socket.remove();
            }
        }
        while let Ok(pending) = self.requests.try_recv() {
            if now >= pending.deadline {
                continue;
            }
            let op = pending.request.op.clone();
            let result = if expired {
                Err("test-control endpoint expired".into())
            } else {
                self.execute(handler, pending.request, now)
            };
            let response = match result {
                Ok(snapshot) => {
                    json!({"schemaVersion":1,"ok":true,"op":op,"snapshot":snapshot,"error":null})
                }
                Err(error) => {
                    json!({"schemaVersion":1,"ok":false,"op":op,"snapshot":null,"error":error})
                }
            };
            let _result = pending.reply.try_send(response);
        }
    }
    fn observe_owner(&mut self, handler: &impl Handler) {
        if !self.start_pending && self.owned.is_none() {
            return;
        }
        if let Ok(status) = handler.status() {
            // This marker is bound by Desktop to the actual test admission token,
            // not inferred from an unrelated user session seen between polls.
            let controlled = status
                .get("controlledSessionId")
                .and_then(Value::as_u64)
                .filter(|id| *id > 0);
            if self.start_pending
                && let Some(session) = controlled
            {
                self.owned = Some(session);
                self.start_pending = false;
            }
            if self.start_pending
                && controlled.is_none()
                && status
                    .get("controlledStartPending")
                    .and_then(Value::as_bool)
                    == Some(false)
            {
                // The asynchronous admission finished without an owned session.
                // A new explicit request may retry; no automatic retry occurs.
                self.start_pending = false;
                self.start_deadline = None;
            }
            if let Some(session) = self.owned
                && status.get("sessionId").and_then(Value::as_u64) != Some(session)
            {
                self.owned = None;
                self.start_deadline = None;
                self.ending = false;
            }
        }
    }
    fn execute(
        &mut self,
        handler: &impl Handler,
        request: Request,
        now: Instant,
    ) -> Result<Value, String> {
        validate_request(&request)?;
        match request.op.as_str() {
            "inspect" => public_inspection(handler, self.recipient_pid),
            "status" => public_status(handler),
            #[cfg(feature = "measurement")]
            "host-observation" => handler.host_observation(),
            #[cfg(feature = "measurement")]
            "handoff-observation" => handler.handoff_observation(),
            "start" | "start-host" => {
                if self.start_pending || self.owned.is_some() || self.ending {
                    return Err("test-control start already admitted or expired".into());
                }
                if request.op == "start-host" {
                    #[cfg(feature = "measurement")]
                    handler.start_host(self.recipient_pid)?;
                    #[cfg(not(feature = "measurement"))]
                    return Err("Host measurement is not compiled".into());
                } else {
                    handler.start(self.recipient_pid)?;
                }
                self.start_pending = true;
                self.start_deadline = Some(now + RECORDING_LIMIT);
                public_status(handler)
            }
            "stop" | "cancel" => {
                let session = request.session.ok_or("session required")?;
                if self.owned != Some(session) {
                    return Err("session is not owned by test-control".into());
                }
                if request.op == "stop" {
                    handler.stop(session)?;
                } else {
                    handler.cancel(session)?;
                }
                public_status(handler)
            }
            _ => Err("unsupported operation".into()),
        }
    }
    pub fn close(&mut self, handler: &impl Handler) {
        self.observe_owner(handler);
        let _result = handler.shutdown();
        self.stopped.store(true, Ordering::Release);
        self.socket.remove();
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.socket.remove();
    }
}
// Explicit projection prevents a future internal status field from exposing text.
fn public_status(handler: &impl Handler) -> Result<Value, String> {
    let source = handler.status()?;
    let mut output = serde_json::Map::new();
    for key in [
        "setupReady",
        "microphoneGranted",
        "accessibilityGranted",
        "phase",
        "sessionId",
        "outcome",
        "publicSentenceMatches",
        "transcriptCharacters",
        "actionError",
        "controlledSessionId",
        "controlledStartPending",
    ] {
        output.insert(
            key.to_owned(),
            source
                .get(key)
                .ok_or("incomplete test-control status")?
                .clone(),
        );
    }
    for key in [
        "setupReady",
        "microphoneGranted",
        "accessibilityGranted",
        "publicSentenceMatches",
        "actionError",
        "controlledStartPending",
    ] {
        if !output[key].is_boolean() {
            return Err("invalid test-control status type".into());
        }
    }
    for key in ["sessionId", "controlledSessionId"] {
        if !output[key].is_null() && output[key].as_u64().is_none_or(|id| id == 0) {
            return Err("invalid test-control session".into());
        }
    }
    if output["transcriptCharacters"].as_u64().is_none()
        || !matches!(
            output["phase"].as_str(),
            Some("Idle" | "Starting" | "Recording" | "Finalizing" | "Cancelling" | "Delivering")
        )
        || !(output["outcome"].is_null()
            || matches!(
                output["outcome"].as_str(),
                Some("sent" | "held" | "cancelled" | "local" | "empty" | "failed")
            ))
    {
        return Err("invalid test-control status value".into());
    }
    Ok(Value::Object(output))
}

// A second strict projection prevents future diagnostics from exporting titles,
// text, arbitrary role/subrole strings, or observations of another process.
fn public_inspection(handler: &impl Handler, recipient_pid: i32) -> Result<Value, String> {
    let source = handler.inspect(recipient_pid)?;
    let mut output = serde_json::Map::new();
    for key in [
        "kind",
        "pid",
        "role",
        "roleRead",
        "roleErrorCode",
        "secure",
        "subroleRead",
        "subroleErrorCode",
        "enabled",
        "enabledRead",
        "enabledErrorCode",
        "valueSettable",
        "valueSettableErrorCode",
        "selectedTextSettable",
        "selectedTextSettableErrorCode",
        "captureEligible",
        "holdReason",
    ] {
        output.insert(
            key.into(),
            source
                .get(key)
                .ok_or("incomplete recipient metadata")?
                .clone(),
        );
    }
    if output["kind"] != "own-test-recipient-field-metadata"
        || output["pid"].as_i64() != Some(i64::from(recipient_pid))
        || !matches!(
            output["role"].as_str(),
            Some("textField" | "textArea" | "comboBox" | "other")
        )
        || !output["captureEligible"].is_boolean()
        || !(output["holdReason"].is_null()
            || matches!(
                output["holdReason"].as_str(),
                Some("unconfirmedField" | "notEditable" | "unknownEditability" | "secureField")
            ))
    {
        return Err("invalid recipient metadata".into());
    }
    for key in ["secure", "enabled", "valueSettable", "selectedTextSettable"] {
        if !(output[key].is_null() || output[key].is_boolean()) {
            return Err("invalid recipient metadata flag".into());
        }
    }
    for key in ["roleRead", "subroleRead", "enabledRead"] {
        if !matches!(
            output[key].as_str(),
            Some("value" | "unsupported" | "noValue" | "error" | "wrongType" | "missingValue")
        ) {
            return Err("invalid recipient metadata status".into());
        }
    }
    for key in [
        "roleErrorCode",
        "subroleErrorCode",
        "enabledErrorCode",
        "valueSettableErrorCode",
        "selectedTextSettableErrorCode",
    ] {
        if !output[key].is_null()
            && output[key]
                .as_i64()
                .is_none_or(|n| i32::try_from(n).is_err())
        {
            return Err("invalid recipient metadata code".into());
        }
    }
    Ok(Value::Object(output))
}
fn io_error(error: impl std::fmt::Display) -> String {
    format!("test-control I/O: {error}")
}
fn validate_directory(path: &Path) -> Result<Config, String> {
    if !path.is_absolute() || fs::canonicalize(path).map_err(io_error)? != path {
        return Err("test-control directory must be an existing canonical absolute path".into());
    }
    let dir = fs::symlink_metadata(path).map_err(io_error)?;
    if !dir.is_dir() || dir.permissions().mode() & 0o777 != 0o700 {
        return Err("test-control directory must have mode 0700".into());
    }
    let config_path = path.join("config.json");
    let meta = fs::symlink_metadata(&config_path).map_err(io_error)?;
    if !meta.is_file()
        || meta.uid() != dir.uid()
        || meta.permissions().mode() & 0o777 != 0o600
        || meta.len() > 256
    {
        return Err(
            "test-control config must be an owned regular mode0600 file of at most 256 bytes"
                .into(),
        );
    }
    // Read through a descriptor and compare identity; reject replacement/symlinks.
    let mut file = OpenOptions::new()
        .read(true)
        .open(&config_path)
        .map_err(io_error)?;
    let opened = file.metadata().map_err(io_error)?;
    if opened.dev() != meta.dev() || opened.ino() != meta.ino() {
        return Err("test-control config changed".into());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(257)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > 256 {
        return Err("test-control config too large".into());
    }
    let config: Config =
        serde_json::from_slice(&bytes).map_err(|_| "invalid test-control config")?;
    if config.recipient_pid <= 0 {
        return Err("recipientPid must be positive".into());
    }
    Ok(config)
}
fn validate_request(request: &Request) -> Result<(), String> {
    if request.schema_version != 1 {
        return Err("unsupported schemaVersion".into());
    }
    match request.op.as_str() {
        "status" | "start" | "inspect" if request.session.is_none() => Ok(()),
        #[cfg(feature = "measurement")]
        "start-host" | "host-observation" | "handoff-observation" if request.session.is_none() => {
            Ok(())
        }
        "stop" | "cancel" if request.session.is_some_and(|id| id > 0) => Ok(()),
        _ => Err("invalid operation or session".into()),
    }
}
fn read_request(stream: &mut UnixStream) -> Result<Request, String> {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(io_error)?;
    let mut bytes = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|time| !time.is_zero())
            .ok_or("request deadline exceeded")?;
        stream.set_read_timeout(Some(remaining)).map_err(io_error)?;
        let mut byte = [0];
        if stream
            .read(&mut byte)
            .map_err(|_| "request read timeout or failure")?
            == 0
        {
            return Err("request requires newline".into());
        }
        bytes.push(byte[0]);
        if bytes.len() > MAX_REQUEST {
            return Err("request exceeds 1024 bytes".into());
        }
        if byte[0] == b'\n' {
            break;
        }
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| "invalid request JSON")?;
    validate_request(&request)?;
    Ok(request)
}
fn serve(
    mut stream: UnixStream,
    send: &SyncSender<Pending>,
    wake: &Arc<dyn Fn() + Send + Sync>,
    stopped: &AtomicBool,
) {
    let result = (|| {
        let request = read_request(&mut stream)?;
        let op = request.op.clone();
        let (reply, receive) = mpsc::sync_channel(1);
        let deadline = Instant::now() + RESPONSE_LIMIT;
        send.try_send(Pending {
            request,
            reply,
            deadline,
        })
        .map_err(|_| "test-control busy")?;
        loop {
            wake();
            match receive.recv_timeout(TICK) {
                Ok(value) => return Ok(value),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("test-control closed".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if stopped.load(Ordering::Acquire) || Instant::now() >= deadline {
                        return Ok(
                            json!({"schemaVersion":1,"ok":false,"op":op,"snapshot":null,"error":"main-thread response timeout"}),
                        );
                    }
                }
            }
        }
    })();
    let response = result.unwrap_or_else(|error: String| json!({"schemaVersion":1,"ok":false,"op":"invalid","snapshot":null,"error":error}));
    if let Ok(mut bytes) = serde_json::to_vec(&response) {
        bytes.push(b'\n');
        let _result = stream.set_write_timeout(Some(Duration::from_millis(250)));
        let _result = stream.write_all(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::FileTypeExt;
    #[test]
    fn binds_private_socket_only_in_explicit_private_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = fs::canonicalize(temp.path())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        fs::write(path.join("config.json"), br#"{"recipientPid":123}"#)?;
        fs::set_permissions(path.join("config.json"), fs::Permissions::from_mode(0o600))?;
        let control = Control::bind(&path, Arc::new(|| {}))?;
        assert_eq!(
            fs::metadata(path.join("control.sock"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(
            fs::symlink_metadata(path.join("control.sock"))?
                .file_type()
                .is_socket()
        );
        assert!(Control::bind(&path, Arc::new(|| {})).is_err());
        drop(control);
        assert!(!path.join("control.sock").exists());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        assert!(Control::bind(&path, Arc::new(|| {})).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct FakeState {
        session: Option<u64>,
        controlled: Option<u64>,
        starts: usize,
        pending: bool,
        stopped: Vec<u64>,
        shutdowns: usize,
        inspections: Vec<i32>,
        inspection_override: Option<Value>,
    }
    #[derive(Default)]
    struct Fake {
        state: Mutex<FakeState>,
    }
    impl Handler for Fake {
        fn inspect(&self, recipient_pid: i32) -> Result<Value, String> {
            let mut state = self.state.lock().map_err(|_| "test lock")?;
            state.inspections.push(recipient_pid);
            if let Some(value) = &state.inspection_override {
                return Ok(value.clone());
            }
            Ok(
                json!({"kind":"own-test-recipient-field-metadata", "pid": recipient_pid,
                "role":"textArea", "roleRead":"value", "roleErrorCode":null,
                "secure":false, "subroleRead":"unsupported", "subroleErrorCode":-25205,
                "enabled":null, "enabledRead":"noValue", "enabledErrorCode":-25212,
                "valueSettable":true, "valueSettableErrorCode":null,
                "selectedTextSettable":null, "selectedTextSettableErrorCode":-25205,
                "captureEligible":false, "holdReason":"unknownEditability",
                "privateText":"must never leave the process", "windowTitle":"private sentinel"}),
            )
        }
        fn status(&self) -> Result<Value, String> {
            let state = self.state.lock().map_err(|_| "test lock")?;
            Ok(
                json!({"setupReady":true,"microphoneGranted":true,"accessibilityGranted":true,"phase":"Idle","sessionId":state.session,"controlledSessionId":state.controlled,"controlledStartPending":state.pending,"outcome":null,"publicSentenceMatches":false,"transcriptCharacters":0,"actionError":false,"transcript":"must never leave the process"}),
            )
        }
        fn start(&self, _: i32) -> Result<(), String> {
            let mut state = self.state.lock().map_err(|_| "test lock")?;
            state.starts += 1;
            state.pending = true;
            Ok(())
        }
        #[cfg(feature = "measurement")]
        fn start_host(&self, recipient_pid: i32) -> Result<(), String> {
            self.start(recipient_pid)
        }
        #[cfg(feature = "measurement")]
        fn host_observation(&self) -> Result<Value, String> {
            Ok(json!({"test":true}))
        }
        #[cfg(feature = "measurement")]
        fn handoff_observation(&self) -> Result<Value, String> {
            Ok(json!({"test":true}))
        }
        fn stop(&self, session: u64) -> Result<(), String> {
            self.state
                .lock()
                .map_err(|_| "test lock")?
                .stopped
                .push(session);
            Ok(())
        }
        fn cancel(&self, session: u64) -> Result<(), String> {
            self.stop(session)
        }
        fn shutdown(&self) -> Result<(), String> {
            self.state.lock().map_err(|_| "test lock")?.shutdowns += 1;
            Ok(())
        }
    }
    #[cfg(feature = "measurement")]
    #[test]
    fn host_start_uses_bound_owner_and_blocks_duplicate_until_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_directory, mut control) = fixture()?;
        let handler = Fake::default();
        control.execute(&handler, request("start-host", None), Instant::now())?;
        assert!(
            control
                .execute(&handler, request("start-host", None), Instant::now())
                .is_err()
        );
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.starts, 1);
        assert!(
            control
                .execute(&handler, request("cancel", Some(99)), Instant::now())
                .is_err()
        );
        {
            let mut state = handler.state.lock().map_err(|_| "test lock")?;
            state.controlled = Some(7);
            state.session = Some(7);
            state.pending = false;
        }
        control.observe_owner(&handler);
        control.execute(&handler, request("cancel", Some(7)), Instant::now())?;
        assert_eq!(
            handler.state.lock().map_err(|_| "test lock")?.stopped,
            vec![7]
        );
        assert!(validate_request(&request("host-observation", Some(7))).is_err());
        Ok(())
    }
    fn fixture() -> Result<(tempfile::TempDir, Control), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let dir = fs::canonicalize(temp.path())?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        fs::write(dir.join("config.json"), br#"{"recipientPid":123}"#)?;
        fs::set_permissions(dir.join("config.json"), fs::Permissions::from_mode(0o600))?;
        let control = Control::bind(&dir, Arc::new(|| {}))?;
        Ok((temp, control))
    }
    fn request(op: &str, session: Option<u64>) -> Request {
        Request {
            schema_version: 1,
            op: op.into(),
            session,
        }
    }
    #[test]
    fn inspection_is_scoped_private_and_does_not_claim_or_change_a_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, mut control) = fixture()?;
        let handler = Fake::default();
        handler.state.lock().map_err(|_| "test lock")?.session = Some(99);
        let value = control.execute(&handler, request("inspect", None), Instant::now())?;
        assert_eq!(value["pid"], 123);
        assert_eq!(value["enabledRead"], "noValue");
        assert_eq!(value["enabledErrorCode"], -25212);
        assert_eq!(
            value.as_object().ok_or("metadata object missing")?.len(),
            17
        );
        assert!(!value.to_string().contains("private"));
        let state = handler.state.lock().map_err(|_| "test lock")?;
        assert_eq!(state.inspections, [123]);
        assert_eq!(state.session, Some(99));
        assert_eq!(state.starts, 0);
        assert!(state.stopped.is_empty());
        assert_eq!(state.shutdowns, 0);
        assert!(!control.start_pending);
        assert_eq!(control.owned, None);
        assert_eq!(control.start_deadline, None);
        assert!(validate_request(&request("inspect", Some(99))).is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"schemaVersion":1,"op":"inspect","pid":999}"#)
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn inspection_rejects_wrong_process_and_untrusted_metadata_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let handler = Fake::default();
        let original = handler.inspect(123)?;
        for (key, invalid) in [
            ("pid", json!(999)),
            ("role", json!("private field text")),
            ("enabledRead", json!("private error text")),
            ("secure", json!("private text")),
            ("enabledErrorCode", json!(2_147_483_648_u64)),
        ] {
            let mut value = original.clone();
            value[key] = invalid;
            handler
                .state
                .lock()
                .map_err(|_| "test lock")?
                .inspection_override = Some(value);
            assert!(
                public_inspection(&handler, 123).is_err(),
                "accepted invalid {key}"
            );
        }
        Ok(())
    }
    #[test]
    fn pending_start_never_claims_an_unrelated_session_and_expiry_revokes_pending()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, mut control) = fixture()?;
        let handler = Fake::default();
        let now = Instant::now();
        control.execute(&handler, request("start", None), now)?;
        {
            let mut state = handler.state.lock().map_err(|_| "test lock")?;
            state.session = Some(9);
        }
        control.poll_at(&handler, now);
        assert!(
            control
                .execute(&handler, request("stop", Some(9)), now)
                .is_err()
        );
        assert!(
            control
                .execute(&handler, request("start", None), now)
                .is_err()
        );
        control.poll_at(&handler, now + RECORDING_LIMIT);
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.shutdowns, 1);
        assert!(
            handler
                .state
                .lock()
                .map_err(|_| "test lock")?
                .stopped
                .is_empty()
        );
        Ok(())
    }
    #[test]
    fn failed_async_admission_releases_transport_for_explicit_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, mut control) = fixture()?;
        let handler = Fake::default();
        let now = Instant::now();
        control.execute(&handler, request("start", None), now)?;
        assert!(
            control
                .execute(&handler, request("start", None), now)
                .is_err()
        );
        handler.state.lock().map_err(|_| "test lock")?.pending = false;
        control.poll_at(&handler, now);
        control.execute(&handler, request("start", None), now)?;
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.starts, 2);
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.shutdowns, 0);
        Ok(())
    }
    #[test]
    fn only_actor_receipt_session_can_stop_and_expiry_removes_endpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, mut control) = fixture()?;
        let handler = Fake::default();
        let now = Instant::now();
        control.execute(&handler, request("start", None), now)?;
        {
            let mut state = handler.state.lock().map_err(|_| "test lock")?;
            state.session = Some(7);
            state.controlled = Some(7);
        }
        control.poll_at(&handler, now);
        assert!(
            control
                .execute(&handler, request("stop", Some(8)), now)
                .is_err()
        );
        control.execute(&handler, request("stop", Some(7)), now)?;
        assert_eq!(
            handler.state.lock().map_err(|_| "test lock")?.stopped,
            vec![7]
        );
        control.poll_at(&handler, control.expires);
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.shutdowns, 1);
        assert!(!control.socket.path.exists());
        assert!(
            control
                .execute(&handler, request("start", None), now)
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn config_and_socket_collisions_are_rejected_without_overwrite()
    -> Result<(), Box<dyn std::error::Error>> {
        let (temp, control) = fixture()?;
        drop(control);
        let directory = fs::canonicalize(temp.path())?;
        let socket = directory.join("control.sock");
        fs::write(&socket, b"existing")?;
        assert!(Control::bind(&directory, Arc::new(|| {})).is_err());
        assert_eq!(fs::read(&socket)?, b"existing");
        fs::remove_file(&socket)?;
        let config = directory.join("config.json");
        fs::remove_file(&config)?;
        let target = directory.join("actual-config.json");
        fs::write(&target, br#"{"recipientPid":123}"#)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
        std::os::unix::fs::symlink(&target, &config)?;
        assert!(Control::bind(&directory, Arc::new(|| {})).is_err());
        assert!(!socket.exists());
        fs::remove_file(&config)?;
        fs::write(&config, vec![b' '; 257])?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600))?;
        assert!(Control::bind(&directory, Arc::new(|| {})).is_err());
        Ok(())
    }
    #[test]
    fn real_socket_rejects_malformed_and_routes_status_without_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let (temp, mut control) = fixture()?;
        let path = fs::canonicalize(temp.path())?.join("control.sock");
        let client = thread::spawn(move || -> Result<Value, String> {
            let mut stream = UnixStream::connect(path).map_err(io_error)?;
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .map_err(io_error)?;
            stream
                .write_all(b"{\"schemaVersion\":1,\"op\":\"status\"}\n")
                .map_err(io_error)?;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).map_err(io_error)?;
            serde_json::from_slice(&bytes).map_err(io_error)
        });
        let handler = Fake::default();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !client.is_finished() && Instant::now() < deadline {
            control.poll(&handler);
            thread::sleep(Duration::from_millis(5));
        }
        let reply = client.join().map_err(|_| "client panicked")??;
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["op"], "status");
        assert!(
            reply["snapshot"].get("transcript").is_none(),
            "transport must never export raw text even if a later status provider adds it"
        );
        assert_eq!(handler.state.lock().map_err(|_| "test lock")?.starts, 0);
        for bytes in [
            b"{\"schemaVersion\":1,\"op\":\"start\",\"extra\":1}\n".as_slice(),
            b"{\"schemaVersion\":1,\"op\":\"stop\",\"session\":0}\n".as_slice(),
        ] {
            let mut stream = UnixStream::connect(&control.socket.path)?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.write_all(bytes)?;
            let mut response = Vec::new();
            stream.read_to_end(&mut response)?;
            let value: Value = serde_json::from_slice(&response)?;
            assert_eq!(value["ok"], false);
        }
        Ok(())
    }
    #[test]
    fn real_socket_inspection_routes_only_configured_pid_and_filters_private_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, mut control) = fixture()?;
        let path = control.socket.path.clone();
        let client = thread::spawn(move || -> Result<Value, String> {
            let mut stream = UnixStream::connect(path).map_err(io_error)?;
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .map_err(io_error)?;
            stream
                .write_all(b"{\"schemaVersion\":1,\"op\":\"inspect\"}\n")
                .map_err(io_error)?;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).map_err(io_error)?;
            serde_json::from_slice(&bytes).map_err(io_error)
        });
        let handler = Fake::default();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !client.is_finished() && Instant::now() < deadline {
            control.poll(&handler);
            thread::sleep(Duration::from_millis(5));
        }
        let reply = client.join().map_err(|_| "client panicked")??;
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["op"], "inspect");
        assert_eq!(reply["snapshot"]["pid"], 123);
        assert!(reply["snapshot"].get("privateText").is_none());
        assert!(reply["snapshot"].get("windowTitle").is_none());
        let state = handler.state.lock().map_err(|_| "test lock")?;
        assert_eq!(state.inspections, [123]);
        assert_eq!(state.starts, 0);
        assert!(state.stopped.is_empty());
        Ok(())
    }
}
