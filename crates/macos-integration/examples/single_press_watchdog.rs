//! Independent bounded watchdog for an explicitly armed, owned product process.
//! No recording, UI, permissions, AX reads, clipboard access or product commands.
#![allow(
    unsafe_code,
    reason = "Uses O_NOFOLLOW, kernel identity and signals only for the explicitly owned process"
)]
use macos_integration::target::ProcessIdentity;
use serde::Deserialize;
use serde_json::json;
use std::{
    fs::{self, DirBuilder, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const CONTRACT: &str = "single-press-2026-10-19";
const TICK: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum Phase {
    Idle,
    Starting,
    Recording,
    Finalizing,
    Delivering,
    Cancelling,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    WaitStartTimeout,
    RecordingTimeout,
    ProcessingTimeout,
    SessionChanged,
    LifetimeExceeded,
}
impl Failure {
    fn reason(self) -> &'static str {
        match self {
            Self::WaitStartTimeout => "waitStartTimeout",
            Self::RecordingTimeout => "recordingTimeout",
            Self::ProcessingTimeout => "processingTimeout",
            Self::SessionChanged => "sessionChanged",
            Self::LifetimeExceeded => "lifetimeExceeded",
        }
    }
}
struct Deadlines {
    started: Option<Duration>,
    processing: Option<Duration>,
    completed: bool,
    session: Option<u64>,
}
impl Deadlines {
    fn new() -> Self {
        Self {
            started: None,
            processing: None,
            completed: false,
            session: None,
        }
    }
    fn observe(
        &mut self,
        elapsed: Duration,
        phase: Phase,
        session: Option<u64>,
    ) -> Result<&'static str, Failure> {
        if elapsed >= Duration::from_secs(900) {
            return Err(Failure::LifetimeExceeded);
        }
        if self.started.is_none() {
            if elapsed >= Duration::from_secs(120) {
                return Err(Failure::WaitStartTimeout);
            }
            if matches!(phase, Phase::Starting | Phase::Recording) {
                let session = session
                    .filter(|id| *id != 0)
                    .ok_or(Failure::SessionChanged)?;
                self.started = Some(elapsed);
                self.session = Some(session);
            } else if phase != Phase::Idle {
                return Err(Failure::SessionChanged);
            } else {
                return Ok("waiting-for-start");
            }
        }
        if session.is_some() && session != self.session {
            return Err(Failure::SessionChanged);
        }
        if !self.completed
            && self
                .processing
                .is_some_and(|since| elapsed.saturating_sub(since) >= Duration::from_secs(120))
        {
            return Err(Failure::ProcessingTimeout);
        }
        if phase == Phase::Idle {
            self.completed = true;
            return Ok("verifying");
        }
        if self.completed || session != self.session {
            return Err(Failure::SessionChanged);
        }
        if matches!(phase, Phase::Starting | Phase::Recording) {
            if self.processing.is_some() {
                return Err(Failure::SessionChanged);
            }
            if elapsed.saturating_sub(self.started.ok_or(Failure::SessionChanged)?)
                >= Duration::from_secs(330)
            {
                return Err(Failure::RecordingTimeout);
            }
            Ok("recording")
        } else {
            let since = *self.processing.get_or_insert(elapsed);
            if elapsed.saturating_sub(since) >= Duration::from_secs(120) {
                return Err(Failure::ProcessingTimeout);
            }
            Ok("processing")
        }
    }
}
fn allowed_identity(expected: ProcessIdentity, current: Option<ProcessIdentity>) -> bool {
    current == Some(expected)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    schema_version: u32,
    contract_id: String,
    run_id: String,
    parent_pid: i32,
    product_pid: i32,
    product_report: PathBuf,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    schema_version: u32,
    kind: String,
    contract_id: String,
    run_id: String,
    process_id: i32,
    sequence: u64,
    observed_unix_millis: u64,
    observer_state: String,
    running: bool,
    phase: Phase,
    session_id: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    run_id: String,
}

fn error() -> io::Error {
    io::Error::other("watchdog validation or I/O failed")
}
fn now() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| u64::try_from(value.as_millis()).ok())
        .ok_or_else(error)
}
fn uid() -> u32 {
    // SAFETY: current effective UID query has no pointer parameters or side effects.
    unsafe { libc::geteuid() }
}
fn ppid() -> i32 {
    // SAFETY: read-only query for this watchdog's direct parent.
    unsafe { libc::getppid() }
}
fn identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}
fn canonical(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|value| matches!(value, Component::ParentDir | Component::CurDir))
        || path.canonicalize()? != path
    {
        return Err(error());
    }
    Ok(())
}
fn private(metadata: &Metadata, directory: bool) -> bool {
    metadata.uid() == uid()
        && if directory {
            metadata.is_dir() && metadata.mode() & 0o777 == 0o700
        } else {
            metadata.is_file() && metadata.nlink() == 1 && metadata.mode() & 0o777 == 0o600
        }
}
fn read_private(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    let parent = path.parent().ok_or_else(error)?;
    canonical(parent)?;
    if !private(&fs::symlink_metadata(parent)?, true) {
        return Err(error());
    }
    let before = fs::symlink_metadata(path)?;
    if !private(&before, false) || before.len() > max as u64 {
        return Err(error());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if identity(&file.metadata()?) != identity(&before) {
        return Err(error());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max || !private(&file.metadata()?, false) {
        return Err(error());
    }
    Ok(bytes)
}
// Only product snapshots are replaced by their publisher. Config, owner marker
// and requests retain read_private's strict immutable inode/link validation.
const SNAPSHOT_ATTEMPTS: usize = 8;
#[derive(Clone, Copy)]
enum SnapshotCheckpoint {
    BeforeOpen,
    AfterRead,
}
enum SnapshotReadError {
    Replaced,
    Invalid(io::Error),
}
impl From<io::Error> for SnapshotReadError {
    fn from(value: io::Error) -> Self {
        Self::Invalid(value)
    }
}
fn snapshot_descriptor_private(metadata: &Metadata) -> Result<(), SnapshotReadError> {
    if private(metadata, false) {
        return Ok(());
    }
    // An opened old version becomes unlinked when rename replaces it. Never
    // accept its bytes; retry a fresh pathname. Hard links and unsafe metadata
    // are not replacement evidence and remain immediate failures.
    if metadata.nlink() == 0
        && metadata.uid() == uid()
        && metadata.is_file()
        && metadata.mode() & 0o777 == 0o600
    {
        return Err(SnapshotReadError::Replaced);
    }
    Err(SnapshotReadError::Invalid(error()))
}
fn snapshot_attempt(
    path: &Path,
    max: usize,
    attempt: usize,
    checkpoint: &mut impl FnMut(SnapshotCheckpoint, usize) -> io::Result<()>,
) -> Result<Vec<u8>, SnapshotReadError> {
    let parent = path.parent().ok_or_else(error)?;
    canonical(parent)?;
    if !private(&fs::symlink_metadata(parent)?, true) {
        return Err(error().into());
    }
    let before = fs::symlink_metadata(path)?;
    if !private(&before, false) || before.len() > max as u64 {
        return Err(error().into());
    }
    checkpoint(SnapshotCheckpoint::BeforeOpen, attempt)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let opened = file.metadata()?;
    snapshot_descriptor_private(&opened)?;
    if opened.len() > max as u64 {
        return Err(error().into());
    }
    if identity(&opened) != identity(&before) {
        return Err(SnapshotReadError::Replaced);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    checkpoint(SnapshotCheckpoint::AfterRead, attempt)?;
    let after = file.metadata()?;
    snapshot_descriptor_private(&after)?;
    if bytes.len() > max || after.len() > max as u64 {
        return Err(error().into());
    }
    let current = fs::symlink_metadata(path)?;
    if !private(&current, false) || current.len() > max as u64 {
        return Err(error().into());
    }
    if identity(&current) != identity(&opened) {
        return Err(SnapshotReadError::Replaced);
    }
    Ok(bytes)
}
fn read_replaceable_snapshot_with(
    path: &Path,
    max: usize,
    mut checkpoint: impl FnMut(SnapshotCheckpoint, usize) -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    for attempt in 0..SNAPSHOT_ATTEMPTS {
        match snapshot_attempt(path, max, attempt, &mut checkpoint) {
            Ok(bytes) => return Ok(bytes),
            Err(SnapshotReadError::Invalid(error)) => return Err(error),
            Err(SnapshotReadError::Replaced) => std::thread::yield_now(),
        }
    }
    Err(error())
}

fn nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

struct ProductReport {
    path: PathBuf,
    directory: (u64, u64),
    marker: (u64, u64),
    run_id: String,
    pid: i32,
}
impl ProductReport {
    fn arm(config: &Config) -> io::Result<Self> {
        if config
            .product_report
            .file_name()
            .and_then(|value| value.to_str())
            != Some("snapshot.json")
        {
            return Err(error());
        }
        let parent = config.product_report.parent().ok_or_else(error)?;
        canonical(parent)?;
        let metadata = fs::symlink_metadata(parent)?;
        if !private(&metadata, true) {
            return Err(error());
        }
        let marker_path = parent.join("owner");
        if read_private(&marker_path, 32)? != config.run_id.as_bytes() {
            return Err(error());
        }
        let result = Self {
            path: config.product_report.clone(),
            directory: identity(&metadata),
            marker: identity(&fs::symlink_metadata(marker_path)?),
            run_id: config.run_id.clone(),
            pid: config.product_pid,
        };
        let first = result.read()?;
        if first.phase != Phase::Idle || first.session_id.is_some() {
            return Err(error());
        }
        Ok(result)
    }
    fn verify_owner(&self) -> io::Result<()> {
        let parent = self.path.parent().ok_or_else(error)?;
        canonical(parent)?;
        if identity(&fs::symlink_metadata(parent)?) != self.directory
            || !private(&fs::symlink_metadata(parent)?, true)
            || identity(&fs::symlink_metadata(parent.join("owner"))?) != self.marker
            || read_private(&parent.join("owner"), 32)? != self.run_id.as_bytes()
        {
            return Err(error());
        }
        Ok(())
    }
    fn read(&self) -> io::Result<Product> {
        self.read_with_checkpoint(|_, _| Ok(()))
    }
    fn read_with_checkpoint(
        &self,
        checkpoint: impl FnMut(SnapshotCheckpoint, usize) -> io::Result<()>,
    ) -> io::Result<Product> {
        self.verify_owner()?;
        let bytes = read_replaceable_snapshot_with(&self.path, 4096, checkpoint)?;
        // Stable immutable directory/marker identity must survive every snapshot
        // read, including replacement retries. No retry covers owner/schema/PID
        // or freshness failures.
        self.verify_owner()?;
        let value: Product = serde_json::from_slice(&bytes).map_err(|_| error())?;
        let wall = now()?;
        if value.schema_version != 1
            || value.kind != "everlast-one-press-product-status"
            || value.contract_id != CONTRACT
            || value.run_id != self.run_id
            || value.process_id != self.pid
            || value.sequence == 0
            || value.observer_state != "observed"
            || !value.running
            || value.observed_unix_millis > wall.saturating_add(100)
            || wall.saturating_sub(value.observed_unix_millis) > 2000
        {
            return Err(error());
        }
        Ok(value)
    }
}

struct Report {
    path: PathBuf,
    directory: (u64, u64),
    marker: (u64, u64),
    snapshot: Option<(u64, u64)>,
    sequence: u64,
    run_id: String,
    product_pid: i32,
    parent_pid: i32,
}
impl Report {
    fn create(path: PathBuf, config: &Config) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|value| matches!(value, Component::ParentDir | Component::CurDir))
        {
            return Err(error());
        }
        canonical(path.parent().ok_or_else(error)?)?;
        DirBuilder::new().mode(0o700).create(&path)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("owner"))?;
        file.write_all(config.run_id.as_bytes())?;
        file.sync_all()?;
        let result = Self {
            directory: identity(&fs::symlink_metadata(&path)?),
            marker: identity(&file.metadata()?),
            path,
            snapshot: None,
            sequence: 0,
            run_id: config.run_id.clone(),
            product_pid: config.product_pid,
            parent_pid: config.parent_pid,
        };
        result.verify()?;
        Ok(result)
    }
    fn verify(&self) -> io::Result<()> {
        canonical(&self.path)?;
        if identity(&fs::symlink_metadata(&self.path)?) != self.directory
            || !private(&fs::symlink_metadata(&self.path)?, true)
            || identity(&fs::symlink_metadata(self.path.join("owner"))?) != self.marker
            || read_private(&self.path.join("owner"), 32)? != self.run_id.as_bytes()
        {
            return Err(error());
        }
        match (
            self.snapshot,
            fs::symlink_metadata(self.path.join("snapshot.json")),
        ) {
            (None, Err(e)) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            (Some(expected), Ok(metadata))
                if identity(&metadata) == expected && private(&metadata, false) =>
            {
                Ok(())
            }
            _ => Err(error()),
        }
    }
    fn request(&self, filename: &'static str) -> io::Result<bool> {
        self.verify()?;
        let path = self.path.join(filename);
        if !path.try_exists()? {
            return Ok(false);
        }
        let modified = fs::symlink_metadata(&path)?.modified()?;
        if SystemTime::now()
            .duration_since(modified)
            .map_or(true, |age| age > Duration::from_secs(2))
        {
            return Err(error());
        }
        let request: Request =
            serde_json::from_slice(&read_private(&path, 128)?).map_err(|_| error())?;
        Ok(request.run_id == self.run_id)
    }
    fn publish(
        &mut self,
        state: &'static str,
        reason: Option<&'static str>,
        started: bool,
        cleanup: Option<bool>,
    ) -> io::Result<()> {
        self.verify()?;
        self.sequence += 1;
        if self.sequence > 9005 {
            return Err(error());
        }
        let value = json!({"schemaVersion":1,"kind":"everlast-single-press-watchdog","contractId":CONTRACT,
            "runId":self.run_id,"sequence":self.sequence,"observedUnixMillis":now()?,"processId":std::process::id(),
            "parentPid":self.parent_pid,"productPid":self.product_pid,"state":state,"armed":cleanup.is_none() && state!="disarmed",
            "startObserved":started,"cleanupAttempted":cleanup.is_some(),"cleanupConfirmed":cleanup.unwrap_or(false),"reason":reason});
        let pending = self.path.join("pending.json");
        let target = self.path.join("snapshot.json");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)?;
        file.write_all(&serde_json::to_vec(&value).map_err(|_| error())?)?;
        file.sync_all()?;
        let pending_id = identity(&file.metadata()?);
        self.verify()?;
        if identity(&fs::symlink_metadata(&pending)?) != pending_id
            || !private(&fs::symlink_metadata(&pending)?, false)
        {
            return Err(error());
        }
        if self.snapshot.is_none() {
            fs::hard_link(&pending, &target)?;
            fs::remove_file(&pending)?;
        } else {
            fs::rename(&pending, &target)?;
        }
        self.snapshot = Some(identity(&fs::symlink_metadata(target)?));
        self.verify()
    }
}

fn zombie(pid: i32) -> bool {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let Ok(size) = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()) else {
        return false;
    };
    // SAFETY: exact-sized aligned output buffer is read only after kernel confirms full initialization.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    // SAFETY: success returned the exact proc_bsdinfo size.
    read == size && unsafe { info.assume_init() }.pbi_status == libc::SZOMB
}
fn ended(expected: ProcessIdentity) -> bool {
    match ProcessIdentity::read(expected.pid()) {
        // A different birth identity proves the old process ended; do not signal its replacement.
        Ok(observed) => observed != expected || zombie(expected.pid()),
        Err(_) => {
            // Failure to read metadata alone is not proof of exit (e.g. a transient error).
            // SAFETY: signal zero queries existence only for the previously captured positive PID.
            let result = unsafe { libc::kill(expected.pid(), 0) };
            result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        }
    }
}
fn signal_owned(expected: ProcessIdentity, signal: i32) -> bool {
    if !allowed_identity(expected, ProcessIdentity::read(expected.pid()).ok()) {
        return false;
    }
    // SAFETY: positive explicitly armed PID is rechecked against its kernel birth identity immediately above.
    unsafe { libc::kill(expected.pid(), signal) == 0 }
}
fn cleanup(expected: ProcessIdentity) -> bool {
    if ended(expected) {
        return true;
    }
    if !signal_owned(expected, libc::SIGTERM) {
        return false;
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ended(expected) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if ended(expected) {
        return true;
    }
    if !signal_owned(expected, libc::SIGKILL) {
        return false;
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ended(expected) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    ended(expected)
}

fn run(config_path: PathBuf, report_path: PathBuf) -> io::Result<bool> {
    let config: Config =
        serde_json::from_slice(&read_private(&config_path, 4096)?).map_err(|_| error())?;
    if config.schema_version != 1
        || config.contract_id != CONTRACT
        || !nonce(&config.run_id)
        || config.parent_pid <= 1
        || config.parent_pid != ppid()
        || config.product_pid <= 1
        || config.product_pid == config.parent_pid
        || config.product_pid == std::process::id() as i32
    {
        return Err(error());
    }
    let parent = ProcessIdentity::read(config.parent_pid).map_err(|_| error())?;
    let product = ProcessIdentity::read(config.product_pid).map_err(|_| error())?;
    let product_report = ProductReport::arm(&config)?;
    let mut report = Report::create(report_path, &config)?;
    if !allowed_identity(product, ProcessIdentity::read(config.product_pid).ok())
        || !allowed_identity(parent, ProcessIdentity::read(config.parent_pid).ok())
    {
        return Err(error());
    }
    let armed = Instant::now();
    let mut deadlines = Deadlines::new();
    let mut previous_sequence = 0;
    let mut previous_time = 0;
    report.publish("ready", None, false, None)?;
    loop {
        let result = (|| -> Result<&'static str, &'static str> {
            if ppid() != config.parent_pid
                || !allowed_identity(parent, ProcessIdentity::read(config.parent_pid).ok())
                || zombie(config.parent_pid)
            {
                return Err("parentLost");
            }
            if !allowed_identity(product, ProcessIdentity::read(config.product_pid).ok())
                || zombie(config.product_pid)
            {
                return Err("productLost");
            }
            if report.request("abort.json").map_err(|_| "requestInvalid")? {
                return Err("requestedAbort");
            }
            let observed = product_report.read().map_err(|_| "observationInvalid")?;
            if report
                .request("disarm.json")
                .map_err(|_| "requestInvalid")?
            {
                if observed.phase != Phase::Idle || observed.session_id.is_some() {
                    return Err("prematureDisarm");
                }
                return Ok("disarmed");
            }
            if observed.sequence < previous_sequence
                || observed.observed_unix_millis < previous_time
            {
                return Err("observationRegressed");
            }
            previous_sequence = observed.sequence;
            previous_time = observed.observed_unix_millis;
            deadlines
                .observe(armed.elapsed(), observed.phase, observed.session_id)
                .map_err(Failure::reason)
        })();
        match result {
            Ok("disarmed") => {
                report.publish("disarmed", None, deadlines.started.is_some(), None)?;
                return Ok(true);
            }
            Ok(state) => {
                if report
                    .publish(state, None, deadlines.started.is_some(), None)
                    .is_err()
                {
                    cleanup(product);
                    return Err(error());
                }
            }
            Err(reason) => {
                let cleaned = cleanup(product);
                report.publish(
                    if reason == "requestedAbort" {
                        "aborted"
                    } else {
                        "failed"
                    },
                    Some(reason),
                    deadlines.started.is_some(),
                    Some(cleaned),
                )?;
                return Ok(false);
            }
        }
        std::thread::sleep(TICK);
    }
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--config" || args[2] != "--report-dir" {
        eprintln!("Usage: single_press_watchdog --config ABS --report-dir NEW_ABS");
        std::process::exit(2)
    }
    match run(PathBuf::from(&args[1]), PathBuf::from(&args[3])) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(_) => {
            eprintln!("Single-press watchdog failed");
            std::process::exit(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temporary(PathBuf);
    impl Temporary {
        fn new() -> io::Result<Self> {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| error())?
                .as_nanos();
            let path = std::env::temp_dir()
                .canonicalize()?
                .join(format!("everlast-watchdog-{}-{time}", std::process::id()));
            DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            if fs::remove_dir_all(&self.0).is_err() { /* Preserve primary assertion. */ }
        }
    }
    fn put_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)
    }
    #[test]
    fn atomic_snapshot_replacement_between_stat_and_open_retries_the_new_inode()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let path = temp.0.join("snapshot.json");
        let next = temp.0.join("next");
        put_private(&path, b"old")?;
        put_private(&next, b"new")?;
        let mut attempts = 0;
        let bytes = read_replaceable_snapshot_with(&path, 32, |stage, attempt| {
            if matches!(stage, SnapshotCheckpoint::BeforeOpen) {
                attempts += 1;
                if attempt == 0 {
                    fs::rename(&next, &path)?;
                }
            }
            Ok(())
        })?;
        assert_eq!(bytes, b"new");
        assert_eq!(attempts, 2);
        Ok(())
    }

    #[test]
    fn atomic_snapshot_unlinked_descriptor_is_not_accepted_as_current()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let path = temp.0.join("snapshot.json");
        let next = temp.0.join("next");
        put_private(&path, b"old")?;
        put_private(&next, b"new")?;
        let old = fs::File::open(&path)?;
        let mut saw_unlinked = false;
        let mut attempts = 0;
        let bytes = read_replaceable_snapshot_with(&path, 32, |stage, attempt| {
            if matches!(stage, SnapshotCheckpoint::BeforeOpen) {
                attempts += 1;
            }
            if matches!(stage, SnapshotCheckpoint::AfterRead) && attempt == 0 {
                fs::rename(&next, &path)?;
                saw_unlinked = old.metadata()?.nlink() == 0;
            }
            Ok(())
        })?;
        assert!(saw_unlinked);
        assert_eq!(bytes, b"new");
        assert_eq!(attempts, 2);
        assert!(!private(&old.metadata()?, false)); // Immutable reads remain strict.
        Ok(())
    }
    #[test]
    fn repeated_snapshot_replacement_has_a_fixed_retry_bound()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let path = temp.0.join("snapshot.json");
        put_private(&path, b"value")?;
        let mut attempts = 0;
        let result = read_replaceable_snapshot_with(&path, 32, |stage, attempt| {
            if matches!(stage, SnapshotCheckpoint::BeforeOpen) {
                attempts += 1;
                let next = temp.0.join(format!("next-{attempt}"));
                put_private(&next, b"value")?;
                fs::rename(next, &path)?;
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(attempts, SNAPSHOT_ATTEMPTS);
        Ok(())
    }
    #[test]
    fn snapshot_retry_does_not_cover_unsafe_mode_hardlinks_symlinks_or_size()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = Temporary::new()?;
        let path = temp.0.join("snapshot.json");
        put_private(&path, b"valid")?;
        let mut attempts = 0;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert!(
            read_replaceable_snapshot_with(&path, 32, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert!(read_private(&path, 32).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        fs::hard_link(&path, temp.0.join("alias"))?;
        assert!(
            read_replaceable_snapshot_with(&path, 32, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert!(read_private(&path, 32).is_err());
        fs::remove_file(&path)?;
        symlink(temp.0.join("alias"), &path)?;
        assert!(
            read_replaceable_snapshot_with(&path, 32, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert!(read_private(&path, 32).is_err());
        fs::remove_file(&path)?;
        put_private(&path, b"too large")?;
        assert!(
            read_replaceable_snapshot_with(&path, 3, |_, _| {
                attempts += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 0);
        Ok(())
    }
    fn product_fixture()
    -> Result<(Temporary, Config, serde_json::Value), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let root = temp.0.join("product");
        DirBuilder::new().mode(0o700).create(&root)?;
        let run_id = "0123456789abcdef0123456789abcdef";
        put_private(&root.join("owner"), run_id.as_bytes())?;
        let config = Config {
            schema_version: 1,
            contract_id: CONTRACT.into(),
            run_id: run_id.into(),
            parent_pid: ppid(),
            product_pid: std::process::id() as i32,
            product_report: root.join("snapshot.json"),
        };
        let value = json!({"schemaVersion":1,"kind":"everlast-one-press-product-status","contractId":CONTRACT,
            "runId":run_id,"processId":config.product_pid,"sequence":1,"observedUnixMillis":now()?,
            "observerState":"observed","running":true,"phase":"Idle","sessionId":null});
        put_private(&config.product_report, &serde_json::to_vec(&value)?)?;
        Ok((temp, config, value))
    }
    #[test]
    fn snapshot_replacement_cannot_replace_immutable_run_marker_or_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        for replace_directory in [false, true] {
            let (temp, config, original) = product_fixture()?;
            let report = ProductReport::arm(&config)?;
            let mut changed = false;
            let result = report.read_with_checkpoint(|stage, _| {
                if matches!(stage, SnapshotCheckpoint::AfterRead) && !changed {
                    changed = true;
                    let root = temp.0.join("product");
                    if replace_directory {
                        fs::rename(&root, temp.0.join("previous"))?;
                        DirBuilder::new().mode(0o700).create(&root)?;
                        put_private(&root.join("owner"), config.run_id.as_bytes())?;
                        put_private(&root.join("snapshot.json"), &serde_json::to_vec(&original)?)?;
                    } else {
                        let next = root.join("next-owner");
                        put_private(&next, config.run_id.as_bytes())?;
                        fs::rename(next, root.join("owner"))?;
                    }
                }
                Ok(())
            });
            assert!(result.is_err());
        }
        Ok(())
    }
    #[test]
    fn replaceable_snapshot_preserves_pid_schema_and_freshness_rejection()
    -> Result<(), Box<dyn std::error::Error>> {
        let (temp, config, original) = product_fixture()?;
        let report = ProductReport::arm(&config)?;
        for (key, value) in [
            ("processId", json!(1)),
            ("runId", json!("foreign")),
            ("schemaVersion", json!(2)),
            ("observedUnixMillis", json!(0)),
            ("observerState", json!("expired")),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            let next = temp.0.join("next");
            put_private(&next, &serde_json::to_vec(&changed)?)?;
            fs::rename(&next, &config.product_report)?;
            let mut reads = 0;
            assert!(
                report
                    .read_with_checkpoint(|stage, _| {
                        if matches!(stage, SnapshotCheckpoint::BeforeOpen) {
                            reads += 1;
                        }
                        Ok(())
                    })
                    .is_err()
            );
            assert_eq!(reads, 1); // Invalid semantic data is never retried.
        }
        Ok(())
    }

    #[test]
    fn actual_start_and_transition_define_independent_deadlines() {
        let mut watch = Deadlines::new();
        assert_eq!(
            watch.observe(Duration::from_secs(119), Phase::Idle, None),
            Ok("waiting-for-start")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(120), Phase::Idle, None),
            Err(Failure::WaitStartTimeout)
        );
        let mut watch = Deadlines::new();
        assert_eq!(
            watch.observe(Duration::from_secs(20), Phase::Starting, Some(7)),
            Ok("recording")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(349), Phase::Recording, Some(7)),
            Ok("recording")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(350), Phase::Recording, Some(7)),
            Err(Failure::RecordingTimeout)
        );
        let mut watch = Deadlines::new();
        assert_eq!(
            watch.observe(Duration::from_secs(1), Phase::Recording, Some(7)),
            Ok("recording")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(301), Phase::Finalizing, Some(7)),
            Ok("processing")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(420), Phase::Delivering, Some(7)),
            Ok("processing")
        );
        assert_eq!(
            watch.observe(Duration::from_secs(421), Phase::Delivering, Some(7)),
            Err(Failure::ProcessingTimeout)
        );
        assert_eq!(
            watch.observe(Duration::from_secs(421), Phase::Idle, None),
            Err(Failure::ProcessingTimeout)
        );
        let mut finished = Deadlines::new();
        assert_eq!(
            finished.observe(Duration::from_secs(1), Phase::Recording, Some(7)),
            Ok("recording")
        );
        assert_eq!(
            finished.observe(Duration::from_secs(301), Phase::Finalizing, Some(7)),
            Ok("processing")
        );
        assert_eq!(
            finished.observe(Duration::from_secs(420), Phase::Idle, None),
            Ok("verifying")
        );
        assert_eq!(
            finished.observe(Duration::from_secs(422), Phase::Starting, Some(8)),
            Err(Failure::SessionChanged)
        );
    }

    #[test]
    fn kernel_identity_guard_rejects_missing_and_different_process_before_signalling()
    -> Result<(), Box<dyn std::error::Error>> {
        let own = ProcessIdentity::read(std::process::id() as i32)?;
        let other = ProcessIdentity::read(ppid())?;
        assert!(allowed_identity(own, Some(own)));
        assert!(!allowed_identity(own, None));
        assert!(!allowed_identity(own, Some(other)));
        Ok(())
    }

    #[test]
    fn real_cleanup_terminates_only_its_spawned_benign_child()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut child = std::process::Command::new("/bin/sleep").arg("30").spawn()?;
        let id = ProcessIdentity::read(child.id() as i32)?;
        // Its actual parent reaps it concurrently, just as the product's OS owner does.
        let reaper = std::thread::spawn(move || child.wait());
        assert!(cleanup(id));
        let status = reaper.join().map_err(|_| error())??;
        assert!(!status.success());
        assert!(ProcessIdentity::read(std::process::id() as i32).is_ok());
        Ok(())
    }

    #[test]
    fn private_output_refuses_collisions_foreign_symlinks_and_disarm_wrong_nonce()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let config = Config {
            schema_version: 1,
            contract_id: CONTRACT.into(),
            run_id: "0123456789abcdef0123456789abcdef".into(),
            parent_pid: ppid(),
            product_pid: std::process::id() as i32,
            product_report: temp.0.join("unused"),
        };
        let mut report = Report::create(temp.0.join("report"), &config)?;
        report.publish("ready", None, false, None)?;
        assert!(Report::create(temp.0.join("report"), &config).is_err());
        let mut request = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(report.path.join("disarm.json"))?;
        request.write_all(b"{\"runId\":\"ffffffffffffffffffffffffffffffff\"}")?;
        assert!(!report.request("disarm.json")?);
        let foreign = temp.0.join("foreign");
        fs::write(&foreign, "foreign sentinel")?;
        fs::remove_file(report.path.join("snapshot.json"))?;
        std::os::unix::fs::symlink(&foreign, report.path.join("snapshot.json"))?;
        assert!(report.publish("recording", None, true, None).is_err());
        assert_eq!(fs::read_to_string(foreign)?, "foreign sentinel");
        Ok(())
    }

    #[test]
    fn product_metadata_is_bound_to_private_run_marker_freshness_and_pid()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = Temporary::new()?;
        let root = temp.0.join("product");
        DirBuilder::new().mode(0o700).create(&root)?;
        let nonce = "0123456789abcdef0123456789abcdef";
        let mut owner = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("owner"))?;
        owner.write_all(nonce.as_bytes())?;
        let pid = std::process::id() as i32;
        let config = Config {
            schema_version: 1,
            contract_id: CONTRACT.into(),
            run_id: nonce.into(),
            parent_pid: ppid(),
            product_pid: pid,
            product_report: root.join("snapshot.json"),
        };
        let original = json!({"schemaVersion":1,"kind":"everlast-one-press-product-status","contractId":CONTRACT,"runId":nonce,"processId":pid,"sequence":1,"observedUnixMillis":now()?,"observerState":"observed","running":true,"phase":"Idle","sessionId":null});
        for (key, value) in [
            ("runId", json!("wrong")),
            ("processId", json!(1)),
            ("observedUnixMillis", json!(0)),
            ("observerState", json!("expired")),
            ("phase", json!("Recording")),
        ] {
            let mut bad = original.clone();
            bad[key] = value;
            let path = root.join("snapshot.json");
            if path.exists() {
                fs::remove_file(&path)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(&serde_json::to_vec(&bad)?)?;
            assert!(ProductReport::arm(&config).is_err());
        }
        fs::remove_file(root.join("snapshot.json"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("snapshot.json"))?;
        file.write_all(&serde_json::to_vec(&original)?)?;
        assert!(ProductReport::arm(&config).is_ok());
        Ok(())
    }
}
