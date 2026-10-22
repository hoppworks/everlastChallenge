//! Opt-in, read-only product observation. Never invokes a product command or OS UI.
use everlast_dictation::contracts as view;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const TTL: Duration = Duration::from_secs(15 * 60);
const INTERVAL: Duration = Duration::from_millis(250);
const CONTRACT: &str = "single-press-2026-10-19";
const MAX_REPORTS: u64 = 3602;

pub struct Observer {
    stop: Sender<()>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl Observer {
    pub fn from_env(
        desktop: everlast_dictation::desktop::Desktop,
        app_data: PathBuf,
    ) -> io::Result<Self> {
        Self::start(
            move || desktop.snapshot(),
            std::env::var_os("EVERLAST_RUN_OBSERVER_DIR").map(PathBuf::from),
            app_data,
            TTL,
            INTERVAL,
        )
    }
    fn start(
        read: impl Fn() -> Result<view::Snapshot, String> + Send + 'static,
        directory: Option<PathBuf>,
        app_data: PathBuf,
        ttl: Duration,
        interval: Duration,
    ) -> io::Result<Self> {
        let (stop, receive) = mpsc::channel();
        let started = Instant::now();
        let worker = thread::Builder::new()
            .name("run-observer".into())
            .spawn(move || {
                let Some(mut report) = configured(directory, &app_data)? else {
                    return Ok(());
                };
                let mut sequence = 0;
                let mut stopping = false;
                loop {
                    let state = if started.elapsed() >= ttl {
                        "expired"
                    } else if stopping || receive.try_recv().is_ok() {
                        "stopped"
                    } else {
                        "observed"
                    };
                    sequence += 1;
                    let source = read().map_err(|_| error("observer source unavailable"))?;
                    let mut value = project(&source).map_err(error)?;
                    drop(source);
                    let object = value
                        .as_object_mut()
                        .ok_or_else(|| error("observer projection invalid"))?;
                    object.insert("schemaVersion".into(), json!(1));
                    object.insert("kind".into(), json!("everlast-one-press-product-status"));
                    object.insert("contractId".into(), json!(CONTRACT));
                    object.insert("sequence".into(), json!(sequence));
                    object.insert("observedUnixMillis".into(), json!(unix_millis()?));
                    object.insert(
                        "monotonicSinceLaunchMillis".into(),
                        json!(
                            u64::try_from(started.elapsed().as_millis())
                                .map_err(|_| error("observer clock overflow"))?
                        ),
                    );
                    object.insert("processId".into(), json!(std::process::id()));
                    object.insert("runId".into(), json!(report.run_id));
                    object.insert("observerState".into(), json!(state));
                    report.publish(&value, sequence)?;
                    if state != "observed" {
                        return Ok(());
                    }
                    match receive.recv_timeout(interval.min(ttl.saturating_sub(started.elapsed())))
                    {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            // Next iteration re-reads actual product state for the final report.
                            stopping = true;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    pub fn close(&mut self) -> io::Result<()> {
        if self.stop.send(()).is_err() { /* Worker has already ended. */ }
        self.worker
            .take()
            .map(|worker| worker.join().map_err(|_| error("observer worker failed"))?)
            .unwrap_or(Ok(()))
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        if self.close().is_err() { /* Explicit close reports errors; Drop still joins. */ }
    }
}

fn error(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn unix_millis() -> io::Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| error("observer clock invalid"))?
            .as_millis(),
    )
    .map_err(|_| error("observer clock overflow"))
}
fn identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}
fn canonical(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || path.canonicalize()? != path
    {
        return Err(error("observer path is not canonical"));
    }
    Ok(())
}
fn private(metadata: &Metadata, directory: bool, uid: u32) -> bool {
    metadata.uid() == uid
        && if directory {
            metadata.is_dir() && metadata.mode() & 0o777 == 0o700
        } else {
            metadata.is_file() && metadata.nlink() == 1 && metadata.mode() & 0o777 == 0o600
        }
}

struct ReportDirectory {
    path: PathBuf,
    directory: (u64, u64),
    marker: (u64, u64),
    snapshot: Option<(u64, u64)>,
    uid: u32,
    run_id: String,
}
impl ReportDirectory {
    fn create(path: PathBuf, run_id: String) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(error("observer output path invalid"));
        }
        canonical(
            path.parent()
                .ok_or_else(|| error("observer parent missing"))?,
        )?;
        DirBuilder::new().mode(0o700).create(&path)?;
        let directory = fs::symlink_metadata(&path)?;
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("owner"))?;
        marker.write_all(run_id.as_bytes())?;
        marker.sync_all()?;
        let result = Self {
            path,
            directory: identity(&directory),
            marker: identity(&marker.metadata()?),
            snapshot: None,
            uid: directory.uid(),
            run_id,
        };
        result.verify()?;
        Ok(result)
    }
    fn verify(&self) -> io::Result<()> {
        canonical(&self.path)?;
        let directory = fs::symlink_metadata(&self.path)?;
        let marker_path = self.path.join("owner");
        let marker = fs::symlink_metadata(&marker_path)?;
        if identity(&directory) != self.directory
            || !private(&directory, true, self.uid)
            || identity(&marker) != self.marker
            || !private(&marker, false, self.uid)
            || marker.len() != 32
        {
            return Err(error("observer owner changed"));
        }
        let mut owner = File::open(marker_path)?;
        if identity(&owner.metadata()?) != self.marker {
            return Err(error("observer owner changed"));
        }
        let mut bytes = [0; 32];
        owner.read_exact(&mut bytes)?;
        if bytes != self.run_id.as_bytes() {
            return Err(error("observer owner changed"));
        }
        match (
            self.snapshot,
            fs::symlink_metadata(self.path.join("snapshot.json")),
        ) {
            (None, Err(e)) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            (Some(expected), Ok(metadata))
                if identity(&metadata) == expected && private(&metadata, false, self.uid) =>
            {
                Ok(())
            }
            _ => Err(error("observer snapshot changed")),
        }
    }
    fn publish(&mut self, value: &Value, sequence: u64) -> io::Result<()> {
        if sequence == 0 || sequence > MAX_REPORTS {
            return Err(error("observer report bound exceeded"));
        }
        self.verify()?;
        let bytes =
            serde_json::to_vec(value).map_err(|_| error("observer serialization failed"))?;
        if bytes.len() > 4096 {
            return Err(error("observer metadata bound exceeded"));
        }
        let pending = self.path.join("pending.json");
        let target = self.path.join("snapshot.json");
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)?;
        output.write_all(&bytes)?;
        output.sync_all()?;
        let pending_identity = identity(&output.metadata()?);
        self.verify()?;
        let pending_meta = fs::symlink_metadata(&pending)?;
        if identity(&pending_meta) != pending_identity || !private(&pending_meta, false, self.uid) {
            return Err(error("observer pending changed"));
        }
        if self.snapshot.is_none() {
            fs::hard_link(&pending, &target)?;
            fs::remove_file(&pending)?;
        } else {
            fs::rename(&pending, &target)?;
        }
        self.snapshot = Some(identity(&fs::symlink_metadata(&target)?));
        self.verify()
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: u32,
    contract_id: String,
    run_id: String,
    expires_unix_millis: u64,
}
fn nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn configured(directory: Option<PathBuf>, app_data: &Path) -> io::Result<Option<ReportDirectory>> {
    if let Some(path) = directory {
        let mut bytes = [0; 16];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let run_id = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        return ReportDirectory::create(path, run_id).map(Some);
    }
    let root = app_data.join("one-press-observer");
    let request_path = root.join("request.json");
    let metadata = match fs::symlink_metadata(&request_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        result => result?,
    };
    canonical(app_data)?;
    canonical(&root)?;
    let uid = fs::metadata(app_data)?.uid();
    let root_identity = identity(&fs::symlink_metadata(&root)?);
    if !private(&fs::symlink_metadata(&root)?, true, uid)
        || !private(&metadata, false, uid)
        || metadata.len() > 1024
    {
        return Err(error("observer request ownership invalid"));
    }
    let request_identity = identity(&metadata);
    let mut input = File::open(&request_path)?;
    if identity(&input.metadata()?) != request_identity {
        return Err(error("observer request changed"));
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut input)
        .take(1025)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 {
        return Err(error("observer request too large"));
    }
    let request: Request =
        serde_json::from_slice(&bytes).map_err(|_| error("observer request invalid"))?;
    let now = unix_millis()?;
    if request.schema_version != 1
        || request.contract_id != CONTRACT
        || !nonce(&request.run_id)
        || request.expires_unix_millis <= now
        || request.expires_unix_millis.saturating_sub(now) > TTL.as_millis() as u64
    {
        return Err(error("observer request invalid or expired"));
    }
    let runs = root.join("runs");
    match DirBuilder::new().mode(0o700).create(&runs) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    canonical(&runs)?;
    if !private(&fs::symlink_metadata(&runs)?, true, uid) {
        return Err(error("observer runs ownership invalid"));
    }
    let report = ReportDirectory::create(runs.join(&request.run_id), request.run_id)?;
    if identity(&fs::symlink_metadata(&root)?) != root_identity
        || identity(&fs::symlink_metadata(&request_path)?) != request_identity
    {
        return Err(error("observer request changed"));
    }
    canonical(&root)?;
    fs::remove_file(request_path)?;
    Ok(Some(report))
}

fn project(snapshot: &view::Snapshot) -> Result<Value, &'static str> {
    if snapshot.session.text.len() > 256 * 1024 {
        return Err("observer text bound exceeded");
    }
    let outcome = snapshot
        .session
        .outcome
        .as_ref()
        .map(|outcome| match outcome {
            view::SessionOutcome::Local => "local",
            view::SessionOutcome::Empty => "empty",
            view::SessionOutcome::Sent => "sent",
            view::SessionOutcome::Cancelled => "cancelled",
            view::SessionOutcome::Held { .. } => "held",
            view::SessionOutcome::Failed { .. } => "failed",
        });
    let hash = (!snapshot.session.text.is_empty()).then(|| {
        (Sha256::digest(snapshot.session.text.as_bytes()))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    Ok(json!({
        "setupReady": snapshot.setup.phase == view::SetupPhase::Ready,
        "microphoneGranted": snapshot.permissions.microphone == view::PermissionStatus::Granted,
        "accessibilityGranted": snapshot.permissions.accessibility == view::PermissionStatus::Granted,
        "running": snapshot.running, "phase": snapshot.session.phase,
        "sessionId": snapshot.session.id, "outcome": outcome,
        "maximumWarning": snapshot.session.maximum_warning,
        "maximumReached": snapshot.session.maximum_reached,
        "elapsedMillis": snapshot.session.elapsed_millis,
        "transcriptUtf8Bytes": snapshot.session.text.len(), "transcriptSha256": hash,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    fn snapshot() -> view::Snapshot {
        let settings = everlast_dictation::settings::Settings::default();
        view::Snapshot {
            refinement: view::Refinement {
                setup: view::RefinementSetup {
                    available: false,
                    phase: view::SetupPhase::Missing,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    can_cancel: false,
                    error: None,
                },
                session_id: None,
                original: None,
                status: view::RefinementStatus::Disabled,
                message: None,
            },
            revision: 1,
            running: true,
            setup: view::Setup {
                phase: view::SetupPhase::Ready,
                model: view::Model {
                    name: "private model".into(),
                    download_bytes: 0,
                    source_url: "private source".into(),
                },
                downloaded_bytes: 0,
                total_bytes: 0,
                can_cancel: false,
                error: Some("private setup error".into()),
            },
            session: view::Session {
                phase: view::SessionPhase::Recording,
                id: Some(7),
                text: "abc".into(),
                outcome: Some(view::SessionOutcome::Held {
                    message: "private hold message".into(),
                }),
                maximum_warning: true,
                maximum_reached: false,
                last_held_session: None,
                elapsed_millis: 270_000,
                level_permille: 0,
            },
            permissions: view::Permissions {
                microphone: view::PermissionStatus::Granted,
                accessibility: view::PermissionStatus::Denied,
            },
            labels: view::Labels {
                device: "private device".into(),
                language: "Deutsch".into(),
                hotkey: "private hotkey".into(),
            },
            settings_busy: false,
            settings_request: 0,
            preferences: view::Preferences {
                key_labels: vec![],
                current: everlast_dictation::preferences::SettingsDraft {
                    refinement: false,
                    model: None,
                    device: settings.device,
                    language: settings.language,
                    hotkey: settings.hotkey,
                },
                devices: vec![],
                error: None,
                refreshing: false,
                needs_restart: false,
                recovery_required: false,
                languages: vec![],
            },
            error: Some("private app error".into()),
        }
    }

    #[test]
    fn projection_retains_real_phase_and_hash_but_no_text_or_private_messages()
    -> Result<(), &'static str> {
        let mut source = snapshot();
        let value = project(&source)?;
        assert_eq!(value["phase"], "Recording");
        assert_eq!(value["sessionId"], 7);
        assert_eq!(value["outcome"], "held");
        assert_eq!(value["transcriptUtf8Bytes"], 3);
        assert_eq!(
            value["transcriptSha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!value.to_string().contains("private"));
        assert!(!value.to_string().contains("abc"));
        source.session.text.clear();
        assert!(project(&source)?["transcriptSha256"].is_null());
        source.session.text = "x".repeat(256 * 1024 + 1);
        assert!(project(&source).is_err());
        Ok(())
    }

    #[test]
    fn disabled_creates_nothing_and_enabled_exports_atomic_private_metadata_until_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let read_calls = calls.clone();
        let mut disabled = Observer::start(
            move || {
                read_calls.fetch_add(1, Ordering::Relaxed);
                Ok(snapshot())
            },
            None,
            root.clone(),
            Duration::from_secs(1),
            Duration::from_millis(10),
        )?;
        disabled.close()?;
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(fs::read_dir(&root)?.count(), 0);
        let output = root.join("new-report");
        let mut enabled = Observer::start(
            || Ok(snapshot()),
            Some(output.clone()),
            root,
            Duration::from_secs(1),
            Duration::from_millis(10),
        )?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !output.join("snapshot.json").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(output.join("snapshot.json").is_file());
        enabled.close()?;
        let bytes = fs::read(output.join("snapshot.json"))?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["observerState"], "stopped");
        assert_eq!(value["phase"], "Recording");
        assert_eq!(value["running"], true);
        assert_eq!(value["contractId"], "single-press-2026-10-19");
        assert_eq!(value["kind"], "everlast-one-press-product-status");
        assert!(!String::from_utf8(bytes)?.contains("private"));
        Ok(())
    }

    #[test]
    fn one_shot_request_selects_only_its_fixed_run_directory_and_is_consumed()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let app_data = temp.path().canonicalize()?;
        let request_root = app_data.join("one-press-observer");
        DirBuilder::new().mode(0o700).create(&request_root)?;
        let nonce = "0123456789abcdef0123456789abcdef";
        let request = json!({"schemaVersion":1,"contractId":CONTRACT,"runId":nonce,"expiresUnixMillis":unix_millis()? + 60_000});
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(request_root.join("request.json"))?;
        file.write_all(serde_json::to_string(&request)?.as_bytes())?;
        let report = configured(None, &app_data)?.ok_or("request did not activate observer")?;
        assert_eq!(report.path, request_root.join("runs").join(nonce));
        assert_eq!(report.run_id, nonce);
        assert!(!request_root.join("request.json").exists());
        assert!(configured(None, &app_data)?.is_none());
        Ok(())
    }

    #[test]
    fn expiry_and_failed_snapshot_end_the_worker_without_fabricating_product_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let output = root.join("expires");
        let mut observer = Observer::start(
            || Ok(snapshot()),
            Some(output.clone()),
            root.clone(),
            Duration::from_millis(30),
            Duration::from_millis(10),
        )?;
        observer
            .worker
            .take()
            .ok_or("worker missing")?
            .join()
            .map_err(|_| "worker panicked")??;
        let value: Value = serde_json::from_slice(&fs::read(output.join("snapshot.json"))?)?;
        assert_eq!(value["observerState"], "expired");
        assert_eq!(value["phase"], "Recording");
        assert!(value["sequence"].as_u64().ok_or("sequence missing")? >= 2);
        let calls = AtomicUsize::new(0);
        let output = root.join("failure");
        let mut failure = Observer::start(
            move || {
                if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                    Ok(snapshot())
                } else {
                    Err("private source failure".into())
                }
            },
            Some(output.clone()),
            root,
            Duration::from_secs(1),
            Duration::from_millis(10),
        )?;
        let result = failure
            .worker
            .take()
            .ok_or("worker missing")?
            .join()
            .map_err(|_| "worker panicked")?;
        assert!(result.is_err());
        let value: Value = serde_json::from_slice(&fs::read(output.join("snapshot.json"))?)?;
        assert_eq!(value["sequence"], 1);
        assert_eq!(value["observerState"], "observed");
        assert!(!value.to_string().contains("private"));
        Ok(())
    }

    #[test]
    fn output_collisions_symlinks_replacements_and_limits_preserve_foreign_files()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let run_id = "0123456789abcdef0123456789abcdef".to_owned();
        let occupied = root.join("occupied");
        fs::create_dir(&occupied)?;
        assert!(ReportDirectory::create(occupied, run_id.clone()).is_err());
        symlink(&root, root.join("link"))?;
        assert!(ReportDirectory::create(root.join("link/report"), run_id.clone()).is_err());
        let mut report = ReportDirectory::create(root.join("report"), run_id)?;
        report.publish(&project(&snapshot())?, 1)?;
        let target = report.path.join("snapshot.json");
        fs::remove_file(&target)?;
        let foreign = root.join("foreign");
        fs::write(&foreign, "foreign sentinel")?;
        symlink(&foreign, &target)?;
        assert!(report.publish(&project(&snapshot())?, 2).is_err());
        assert_eq!(fs::read_to_string(&foreign)?, "foreign sentinel");
        assert!(
            report
                .publish(&project(&snapshot())?, MAX_REPORTS + 1)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn invalid_expired_or_redirected_request_never_creates_output_or_consumes_request()
    -> Result<(), Box<dyn std::error::Error>> {
        for (field, value) in [
            ("runId", json!("../foreign")),
            ("expiresUnixMillis", json!(0)),
            ("expiresUnixMillis", json!(unix_millis()? + 2_000_000)),
            ("outputPath", json!("/foreign")),
        ] {
            let temp = tempfile::tempdir()?;
            let app_data = temp.path().canonicalize()?;
            let root = app_data.join("one-press-observer");
            DirBuilder::new().mode(0o700).create(&root)?;
            let mut request = json!({"schemaVersion":1,"contractId":CONTRACT,"runId":"0123456789abcdef0123456789abcdef","expiresUnixMillis":unix_millis()? + 60_000});
            request[field] = value;
            let request_path = root.join("request.json");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&request_path)?;
            file.write_all(serde_json::to_string(&request)?.as_bytes())?;
            assert!(configured(None, &app_data).is_err());
            assert!(request_path.exists());
            assert!(!root.join("runs").exists());
        }
        Ok(())
    }
}
