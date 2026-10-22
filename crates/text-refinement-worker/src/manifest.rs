//! Fixed public model assets and a deterministic in-memory tokenizer adapter.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use text_refinement_protocol::FailureKind;

pub const MANIFEST_JSON: &str = include_str!("model-manifest.json");
pub const ADAPTER_VERSION: u16 = 1;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    pub repository: String,
    pub revision: String,
    pub files: Vec<Asset>,
    pub adapter_version: u16,
    pub adapter_sha256: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

pub fn pinned_manifest() -> Result<Manifest, FailureKind> {
    serde_json::from_str(MANIFEST_JSON).map_err(|_| FailureKind::ModelLoad)
}
pub fn fingerprint() -> String {
    hex(Sha256::digest(MANIFEST_JSON.as_bytes()))
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn verify(directory: &Path, cancel: &AtomicBool) -> Result<Manifest, FailureKind> {
    if cancel.load(Ordering::Acquire) {
        return Err(FailureKind::Incomplete);
    }
    let manifest = pinned_manifest()?;
    if manifest.adapter_version != ADAPTER_VERSION
        || !directory.is_absolute()
        || fs::symlink_metadata(directory)
            .map_err(|_| FailureKind::ModelLoad)?
            .file_type()
            .is_symlink()
    {
        return Err(FailureKind::ModelLoad);
    }
    let mut count = 0;
    for entry in fs::read_dir(directory).map_err(|_| FailureKind::ModelLoad)? {
        let entry = entry.map_err(|_| FailureKind::ModelLoad)?;
        if !manifest
            .files
            .iter()
            .any(|asset| entry.file_name() == std::ffi::OsStr::new(&asset.name))
            || !entry
                .file_type()
                .map_err(|_| FailureKind::ModelLoad)?
                .is_file()
        {
            return Err(FailureKind::ModelLoad);
        }
        count += 1;
    }
    if count != manifest.files.len() {
        return Err(FailureKind::ModelLoad);
    }
    for asset in &manifest.files {
        if cancel.load(Ordering::Acquire) {
            return Err(FailureKind::Incomplete);
        }
        let mut file = open_asset(&directory.join(&asset.name))?;
        if !file
            .metadata()
            .map_err(|_| FailureKind::ModelLoad)?
            .is_file()
            || file.metadata().map_err(|_| FailureKind::ModelLoad)?.len() != asset.bytes
        {
            return Err(FailureKind::ModelLoad);
        }
        let mut digest = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(FailureKind::Incomplete);
            }
            let n = file.read(&mut buffer).map_err(|_| FailureKind::ModelLoad)?;
            if n == 0 {
                break;
            }
            bytes = bytes.checked_add(n as u64).ok_or(FailureKind::ModelLoad)?;
            if bytes > asset.bytes {
                return Err(FailureKind::ModelLoad);
            }
            digest.update(&buffer[..n]);
        }
        if bytes != asset.bytes || hex(digest.finalize()) != asset.sha256 {
            return Err(FailureKind::ModelLoad);
        }
    }
    Ok(manifest)
}

fn open_asset(path: &Path) -> Result<File, FailureKind> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Reject symlinks and avoid blocking if a regular file was replaced by a FIFO.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| FailureKind::ModelLoad)?;
    if !file
        .metadata()
        .map_err(|_| FailureKind::ModelLoad)?
        .is_file()
    {
        return Err(FailureKind::ModelLoad);
    }
    Ok(file)
}

/// Owns a private snapshot until all model arrays and memory maps are released.
/// This isolates loading from installer/cache replacement. As with the app's
/// process memory, another process with the same user privileges is trusted.
pub struct VerifiedSnapshot {
    directory: tempfile::TempDir,
    pub manifest: Manifest,
}
impl VerifiedSnapshot {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

pub fn snapshot(
    source: &Path,
    owner_directory: &Path,
    cancel: &AtomicBool,
) -> Result<VerifiedSnapshot, FailureKind> {
    if !source.is_absolute() || cancel.load(Ordering::Acquire) {
        return Err(if cancel.load(Ordering::Acquire) {
            FailureKind::Incomplete
        } else {
            FailureKind::ModelLoad
        });
    }
    // The supervisor owns this parent and removes it after waiting for the
    // worker, including forced termination; Drop alone cannot cover SIGKILL.
    let metadata = fs::symlink_metadata(owner_directory).map_err(|_| FailureKind::ModelLoad)?;
    if !owner_directory.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(FailureKind::ModelLoad);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(FailureKind::ModelLoad);
        }
    }
    let manifest = pinned_manifest()?;
    let directory = tempfile::Builder::new()
        .prefix("dictation-refinement-")
        .tempdir_in(owner_directory)
        .map_err(|_| FailureKind::ModelLoad)?;
    for asset in &manifest.files {
        let mut input = open_asset(&source.join(&asset.name))?;
        if input.metadata().map_err(|_| FailureKind::ModelLoad)?.len() != asset.bytes {
            return Err(FailureKind::ModelLoad);
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.path().join(&asset.name))
            .map_err(|_| FailureKind::ModelLoad)?;
        let mut digest = Sha256::new();
        let mut remaining = asset.bytes;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(FailureKind::Incomplete);
            }
            let n = input
                .read(&mut buffer)
                .map_err(|_| FailureKind::ModelLoad)?;
            if n == 0 {
                break;
            }
            remaining = remaining
                .checked_sub(n as u64)
                .ok_or(FailureKind::ModelLoad)?;
            digest.update(&buffer[..n]);
            std::io::Write::write_all(&mut output, &buffer[..n])
                .map_err(|_| FailureKind::ModelLoad)?;
        }
        if remaining != 0 || hex(digest.finalize()) != asset.sha256 {
            return Err(FailureKind::ModelLoad);
        }
        let mut permissions = output
            .metadata()
            .map_err(|_| FailureKind::ModelLoad)?
            .permissions();
        permissions.set_readonly(true);
        output
            .set_permissions(permissions)
            .map_err(|_| FailureKind::ModelLoad)?;
    }
    Ok(VerifiedSnapshot {
        directory,
        manifest,
    })
}

pub fn read_template(directory: &Path) -> Result<String, FailureKind> {
    let mut bytes = Vec::new();
    open_asset(&directory.join("chat_template.jinja"))?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| FailureKind::ModelLoad)?;
    if bytes.len() > 65536 {
        return Err(FailureKind::ModelLoad);
    }
    String::from_utf8(bytes).map_err(|_| FailureKind::ModelLoad)
}

fn sorted(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut rows = map.into_iter().collect::<Vec<_>>();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(
                rows.into_iter()
                    .map(|(key, value)| (key, sorted(value)))
                    .collect(),
            )
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sorted).collect())
        }
        value => value,
    }
}

/// Does not mutate the downloaded configuration or write any unverified assets.
pub fn adapted_config(config: &[u8], template: &str) -> Result<Vec<u8>, FailureKind> {
    if config.len() > 65536 || template.len() > 65536 {
        return Err(FailureKind::ModelLoad);
    }
    let mut value: serde_json::Value =
        serde_json::from_slice(config).map_err(|_| FailureKind::ModelLoad)?;
    value
        .as_object_mut()
        .ok_or(FailureKind::ModelLoad)?
        .insert("chat_template".into(), template.into());
    serde_json::to_vec(&sorted(value)).map_err(|_| FailureKind::ModelLoad)
}

pub fn verify_adapter(bytes: &[u8], manifest: &Manifest) -> Result<(), FailureKind> {
    if hex(Sha256::digest(bytes)) != manifest.adapter_sha256 {
        return Err(FailureKind::ModelLoad);
    }
    Ok(())
}
