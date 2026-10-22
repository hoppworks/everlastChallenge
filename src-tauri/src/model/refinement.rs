//! Atomic complete-set publication for the fixed local text model. I/O worker only.
use super::{ModelDefinition, ModelError};
use std::{
    collections::HashSet,
    fs, io,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

const MANIFEST: &str =
    include_str!("../../../crates/text-refinement-worker/src/model-manifest.json");
const REPOSITORY: &str = "mlx-community/Qwen3-4B-Instruct-2507-4bit";
const CACHE_ROOT: &str = "text-refinement";
const RESERVE: u64 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Text model installation is incomplete")]
    IncompleteModel,
    #[error("Text model cache contains an unsupported entry")]
    InvalidCache,
    #[error("Insufficient model storage: {available} bytes available, {required} required")]
    InsufficientSpace { available: u64, required: u64 },
    #[error("Text model publication failed: {publish}; restoring previous cache failed: {restore}")]
    PublicationRecovery {
        publish: io::Error,
        restore: io::Error,
    },
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest<'a> {
    repository: &'a str,
    revision: &'a str,
    #[serde(borrow)]
    files: Vec<Asset<'a>>,
    adapter_version: u16,
    adapter_sha256: &'a str,
}
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset<'a> {
    name: &'a str,
    bytes: u64,
    sha256: &'a str,
}
impl Asset<'static> {
    fn definition(self) -> ModelDefinition {
        ModelDefinition {
            id: self.name,
            file: self.name,
            bytes: self.bytes,
            sha256: self.sha256,
        }
    }
}

pub struct RefinementModel {
    manifest: Manifest<'static>,
    total: u64,
}
impl RefinementModel {
    pub fn fingerprint() -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(MANIFEST.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    pub fn cache_directory(&self, models: &Path) -> PathBuf {
        self.directory(models)
    }
    pub fn bundled() -> Result<Self, Error> {
        Self::parse(MANIFEST)
    }
    fn parse(json: &'static str) -> Result<Self, Error> {
        let manifest: Manifest<'static> =
            serde_json::from_str(json).map_err(|_| Error::InvalidCache)?;
        let mut names = HashSet::new();
        let hex = |value: &str, len| {
            value.len() == len
                && value
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if manifest.repository != REPOSITORY
            || !hex(manifest.revision, 40)
            || manifest.adapter_version != 1
            || !hex(manifest.adapter_sha256, 64)
            || manifest.files.is_empty()
            || manifest.files.len() > 32
        {
            return Err(Error::InvalidCache);
        }
        let mut total = 0u64;
        for asset in &manifest.files {
            if asset.name.is_empty()
                || matches!(asset.name, "." | "..")
                || !asset
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
                || !names.insert(asset.name)
                || !hex(asset.sha256, 64)
                || asset.bytes == 0
            {
                return Err(Error::InvalidCache);
            }
            total = total.checked_add(asset.bytes).ok_or(ModelError::Size)?;
        }
        Ok(Self { manifest, total })
    }
    pub fn installation_bytes(&self) -> u64 {
        self.total
    }
    pub fn staging_and_snapshot_bytes(&self) -> Result<u64, Error> {
        Ok(self
            .total
            .checked_mul(2)
            .and_then(|n| n.checked_add(RESERVE))
            .ok_or(ModelError::Size)?)
    }
    fn directory(&self, models: &Path) -> PathBuf {
        models.join(CACHE_ROOT).join(format!(
            "qwen3-4b-{}-a{}",
            self.manifest.revision, self.manifest.adapter_version
        ))
    }
    /// Verify the complete local cache without constructing a network client.
    pub fn inspect(
        &self,
        models: &Path,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, Error> {
        cancelled(cancel)?;
        let root = models.join(CACHE_ROOT);
        if !models.is_absolute() {
            return Err(Error::InvalidCache);
        }
        match fs::symlink_metadata(&root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(m) if !m.is_dir() => return Err(Error::InvalidCache),
            Ok(_) => {}
        }
        let _lock = super::cache::DirectoryLock::acquire(&root, CACHE_ROOT, cancel)?;
        self.recover_stages(&root, cancel)?;
        self.existing(models, cancel)
    }
    /// Run only after explicit setup consent, on a dedicated I/O thread. A
    /// returned directory means verified assets, not successful model loading.
    pub fn install(
        &self,
        models: &Path,
        cancel: &CancellationToken,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<PathBuf, Error> {
        let mut transport = None;
        self.install_with(
            models,
            cancel,
            &mut progress,
            |path| macos_integration::storage::available_bytes(path).map_err(ModelError::Io),
            &mut |directory, asset, cancel, progress| {
                if transport.is_none() {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    let client = super::download_client_builder()?
                        .build()
                        .map_err(super::network_error)?;
                    transport = Some((runtime, client));
                }
                let (runtime, client) = transport.as_ref().ok_or(ModelError::Closed)?;
                let url = format!(
                    "https://huggingface.co/{}/resolve/{}/{}",
                    self.manifest.repository, self.manifest.revision, asset.name
                );
                runtime.block_on(super::transfer(
                    directory,
                    asset.definition(),
                    cancel,
                    &mut |bytes| progress(bytes),
                    client,
                    &url,
                ))
            },
        )
    }
    fn existing(
        &self,
        models: &Path,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, Error> {
        let directory = self.directory(models);
        match fs::symlink_metadata(&directory) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(m) if !m.is_dir() => return Err(Error::InvalidCache),
            Ok(_) => {}
        }
        self.verify_set(&directory, cancel)?;
        Ok(Some(directory))
    }
    fn verify_set(&self, directory: &Path, cancel: &CancellationToken) -> Result<(), Error> {
        cancelled(cancel)?;
        for (index, entry) in fs::read_dir(directory)?.enumerate() {
            cancelled(cancel)?;
            let entry = entry?;
            if index >= self.manifest.files.len()
                || !self
                    .manifest
                    .files
                    .iter()
                    .any(|a| entry.file_name() == a.name)
                || !entry.file_type()?.is_file()
                || entry.metadata()?.nlink() != 1
            {
                return Err(Error::InvalidCache);
            }
        }
        for asset in &self.manifest.files {
            let path = directory.join(asset.name);
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Err(Error::IncompleteModel);
                }
                Err(e) => return Err(e.into()),
                Ok(m) if !m.is_file() || m.nlink() != 1 => return Err(Error::InvalidCache),
                Ok(_) => {}
            }
            super::verify_file_cancellable(&path, asset.definition(), cancel)?;
        }
        cancelled(cancel)
    }
    fn install_with(
        &self,
        models: &Path,
        cancel: &CancellationToken,
        progress: &mut impl FnMut(u64, u64),
        available: impl FnOnce(&Path) -> Result<u64, ModelError>,
        fetch: &mut impl FnMut(
            &Path,
            Asset<'static>,
            &CancellationToken,
            &mut dyn FnMut(u64),
        ) -> Result<PathBuf, ModelError>,
    ) -> Result<PathBuf, Error> {
        cancelled(cancel)?;
        if !models.is_absolute() {
            return Err(Error::InvalidCache);
        }
        let root = models.join(CACHE_ROOT);
        match fs::symlink_metadata(&root) {
            Ok(m) if !m.is_dir() => return Err(Error::InvalidCache),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir_all(&root)?,
            Err(e) => return Err(e.into()),
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let _lock = super::cache::DirectoryLock::acquire(&root, CACHE_ROOT, cancel)?;
        self.recover_stages(&root, cancel)?;
        let replace = match self.existing(models, cancel) {
            Ok(Some(path)) => {
                progress(self.total, self.total);
                return Ok(path);
            }
            Ok(None) => false,
            Err(Error::IncompleteModel | Error::Model(ModelError::Hash | ModelError::Size)) => true,
            Err(e) => return Err(e),
        };
        let required = self.staging_and_snapshot_bytes()?;
        let free = available(&root)?;
        if free < required {
            return Err(Error::InsufficientSpace {
                available: free,
                required,
            });
        }
        cancelled(cancel)?;
        let stage = tempfile::Builder::new()
            .prefix(&format!(".everlast-text-stage-{}-", std::process::id()))
            .tempdir_in(&root)?;
        let mut completed = 0u64;
        progress(0, self.total);
        for asset in &self.manifest.files {
            cancelled(cancel)?;
            let mut invalid_progress = false;
            fetch(stage.path(), *asset, cancel, &mut |bytes| {
                if bytes > asset.bytes {
                    invalid_progress = true;
                    return;
                }
                if let Some(current) = completed.checked_add(bytes) {
                    progress(current, self.total);
                } else {
                    invalid_progress = true;
                }
            })?;
            if invalid_progress {
                return Err(ModelError::Size.into());
            }
            completed = completed.checked_add(asset.bytes).ok_or(ModelError::Size)?;
        }
        self.verify_set(stage.path(), cancel)?;
        cancelled(cancel)?;
        let target = publish_set(stage, self.directory(models), replace, &mut |from, to| {
            fs::rename(from, to)
        })?;
        // Rename is the publication commit point. Cancellation afterward does
        // not undo a complete cache; callers must still bind setup activation.
        progress(self.total, self.total);
        Ok(target)
    }
    fn recover_stages(&self, root: &Path, cancel: &CancellationToken) -> Result<(), Error> {
        let mut candidates = Vec::new();
        // Finish the bounded root scan before removing anything. Quarantines,
        // other revisions, unrecognized names and redirected entries survive.
        for (index, entry) in fs::read_dir(root)?.enumerate() {
            cancelled(cancel)?;
            if index >= 1024 {
                return Err(Error::InvalidCache);
            }
            let entry = entry?;
            let Some(pid) = entry.file_name().to_str().and_then(stage_pid) else {
                continue;
            };
            if entry.file_type()?.is_dir() && macos_integration::process::definitely_absent(pid) {
                candidates.push((entry.path(), pid));
            }
        }
        for (path, pid) in candidates {
            cancelled(cancel)?;
            let mut owned = true;
            for (index, entry) in fs::read_dir(&path)?.enumerate() {
                cancelled(cancel)?;
                let entry = entry?;
                let name = entry.file_name();
                let known = name.to_str().is_some_and(|name| {
                    self.manifest.files.iter().any(|asset| {
                        name == asset.name
                            || super::cache::part_pid(name, &format!("everlast-{}-", asset.name))
                                == Some(pid)
                    })
                });
                if index > self.manifest.files.len()
                    || !known
                    || !entry.file_type()?.is_file()
                    || entry.metadata()?.nlink() != 1
                {
                    owned = false;
                    break;
                }
            }
            cancelled(cancel)?;
            if owned
                && fs::symlink_metadata(&path)?.is_dir()
                && macos_integration::process::definitely_absent(pid)
            {
                fs::remove_dir_all(&path)?;
            }
        }
        Ok(())
    }
}

// The syscall seam permits deterministic rename failures while tests retain
// actual directories. Production always uses fs::rename.
fn publish_set(
    stage: tempfile::TempDir,
    target: PathBuf,
    replace: bool,
    rename: &mut impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<PathBuf, Error> {
    if replace {
        let root = target.parent().ok_or(Error::InvalidCache)?;
        // Preserve the previous corrupt set; a valid set never reaches here.
        let backup = tempfile::Builder::new()
            .prefix(".everlast-text-replaced-")
            .tempdir_in(root)?;
        rename(&target, backup.path())?;
        let backup = backup.keep();
        if let Err(publish) = rename(stage.path(), &target) {
            return match rename(&backup, &target) {
                Ok(()) => Err(publish.into()),
                Err(restore) => Err(Error::PublicationRecovery { publish, restore }),
            };
        }
    } else {
        rename(stage.path(), &target)?;
    }
    drop(stage.keep());
    Ok(target)
}

fn stage_pid(name: &str) -> Option<u32> {
    let (pid, suffix) = name
        .strip_prefix(".everlast-text-stage-")?
        .split_once('-')?;
    let value = pid.parse::<u32>().ok()?;
    (value != 0
        && value.to_string() == pid
        && (6..=32).contains(&suffix.len())
        && suffix.bytes().all(|c| c.is_ascii_alphanumeric()))
    .then_some(value)
}

fn cancelled(cancel: &CancellationToken) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(ModelError::Cancelled.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
