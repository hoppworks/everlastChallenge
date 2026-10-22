//! Cooperative crash recovery. Same-user path replacement is outside the trust
//! boundary, as for the model cache. Never infer ownership from a PID or age.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
const PREFIX: &str = "text-worker-v1-";
const MARKER: &[u8] = b"everlast-text-owner-v1\n";
const MAX_ENTRIES: usize = 1024;

struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        // Unlock explicitly: an unrelated concurrent fork may briefly inherit
        // this open-file description before exec closes the descriptor.
        if self.0.unlock().is_err() {
            eprintln!("Could not release text worker directory lock");
        }
    }
}
fn regular_lock(path: &Path, create: bool) -> io::Result<File> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.nlink() != 1 => {
            return Err(io::Error::other("Invalid text worker lock"));
        }
        Ok(_) => {}
        Err(error) if create && error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(io::Error::other("Invalid text worker lock"));
    }
    Ok(file)
}
fn directory_lock(root: &Path) -> io::Result<Lock> {
    let metadata = fs::symlink_metadata(root)?;
    if !root.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("Invalid text runtime directory"));
    }
    let file = regular_lock(&root.join(".text-worker.lock"), true)?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(Lock(file))
}
fn recover_locked(root: &Path) -> io::Result<()> {
    for (count, entry) in fs::read_dir(root)?.enumerate() {
        if count >= MAX_ENTRIES {
            return Err(io::Error::other(
                "Text runtime directory exceeds its entry limit",
            ));
        }
        let entry = entry?;
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(PREFIX))
            || !entry.file_type()?.is_dir()
        {
            continue;
        }
        let path = entry.path();
        let mut file = match regular_lock(&path.join(".owner.lock"), false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let mut marker = Vec::new();
        (&mut file).take(64).read_to_end(&mut marker)?;
        if marker != MARKER {
            continue;
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => continue,
            Err(fs::TryLockError::Error(error)) => return Err(error),
        }
        let _lease = Lock(file);
        // A held exclusive lease proves both the host and the worker released
        // their shared leases. remove_dir_all does not follow contained symlinks.
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
/// Called on the application service thread at startup and before every spawn.
/// Leaves live workers, legacy directories and unrecognized contents untouched.
pub fn recover(root: &Path) -> io::Result<()> {
    let _root_lock = directory_lock(root)?;
    recover_locked(root)
}
pub(super) struct Scratch {
    directory: tempfile::TempDir,
    lease: Lock,
}
impl Scratch {
    pub(super) fn create(root: &Path) -> io::Result<Self> {
        let _root_lock = directory_lock(root)?;
        recover_locked(root)?;
        let directory = tempfile::Builder::new().prefix(PREFIX).tempdir_in(root)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.path().join(".owner.lock"))?;
        file.write_all(MARKER)?;
        file.lock_shared()?;
        Ok(Self {
            directory,
            lease: Lock(file),
        })
    }
    pub(super) fn path(&self) -> &Path {
        self.directory.path()
    }
    pub(super) fn close(self) -> io::Result<()> {
        let Self { directory, lease } = self;
        drop(lease);
        directory.close()
    }
    pub(super) fn keep(self) -> PathBuf {
        let Self { directory, lease } = self;
        drop(lease);
        directory.keep()
    }
}
