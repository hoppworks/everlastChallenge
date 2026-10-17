//! Cooperative per-model cache ownership and conservative legacy-part recovery.
//! Main application cache workers must retain this guard through publication.
//! Metadata/open is not a claim of protection against malicious local path swaps.
use super::{ModelDefinition, ModelError};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;
const MAX_ENTRIES: usize = 1024;
pub(super) struct CacheGuard {
    _lock: File,
    directory: PathBuf,
    definition: ModelDefinition,
}
impl CacheGuard {
    pub(super) fn acquire(
        directory: &Path,
        definition: ModelDefinition,
        cancel: &CancellationToken,
    ) -> Result<Self, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        check_directory(directory, true)?;
        fs::create_dir_all(directory)?;
        check_directory(directory, false)?;
        let path = directory.join(format!(".everlast-{}.lock", definition.id));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "cache lock must be a regular file",
                )
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        let metadata = lock.metadata()?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cache lock must be a regular file",
            )
            .into());
        }
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(ModelError::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        lock.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            _lock: lock,
            directory: directory.to_owned(),
            definition,
        })
    }
    pub(super) fn cleanup_parts(&self, cancel: &CancellationToken) -> Result<(), ModelError> {
        self.cleanup_with(cancel, macos_integration::process::definitely_absent)
    }
    fn cleanup_with(
        &self,
        cancel: &CancellationToken,
        absent: impl Fn(u32) -> bool,
    ) -> Result<(), ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        check_directory(&self.directory, false)?;
        let prefix = format!("everlast-{}-", self.definition.id);
        let mut candidates = Vec::new();
        for (index, entry) in fs::read_dir(&self.directory)?.enumerate() {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            if index >= MAX_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cache directory entry limit exceeded",
                )
                .into());
            }
            let entry = entry?;
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|name| part_pid(name, &prefix)) else {
                continue;
            };
            if entry.file_type()?.is_file() && absent(pid) {
                candidates.push((entry.path(), pid));
            }
        }
        // No deletion until the complete bounded scan has succeeded. A failed
        // scan or cancellation during liveness checks leaves every part intact.
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        for (path, pid) in candidates {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if !metadata.is_file() || !absent(pid) {
                continue;
            }
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}
fn check_directory(path: &Path, allow_missing: bool) -> Result<(), ModelError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache path must be a directory without redirection",
        )
        .into()),
        Err(error) if allow_missing && error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn part_pid(name: &str, prefix: &str) -> Option<u32> {
    let body = name.strip_prefix(prefix)?.strip_suffix(".part")?;
    let (pid, nonce) = body.split_once('-')?;
    let parsed_pid = pid.parse::<u32>().ok()?;
    let parsed_nonce = nonce.parse::<u64>().ok()?;
    // These are exactly the canonical decimal numbers produced by the legacy
    // writer, excluding zero, signs, padding, overflows and extra suffixes.
    if parsed_pid == 0
        || parsed_nonce == 0
        || parsed_pid.to_string() != pid
        || parsed_nonce.to_string() != nonce
    {
        return None;
    }
    Some(parsed_pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SMALL;
    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                if let Err(_already_exited) = self.0.kill() {}
                if let Err(_wait_failed) = self.0.wait() {}
            }
        }
    }
    #[test]
    fn child_fixture_holds_cache() -> Result<(), Box<dyn std::error::Error>> {
        let Some(directory) = std::env::var_os("EVERLAST_CACHE_GUARD_TEST_DIRECTORY") else {
            return Ok(());
        };
        let directory = PathBuf::from(directory);
        let _guard = CacheGuard::acquire(&directory, SMALL, &CancellationToken::new())?;
        fs::write(
            directory.join(format!(
                "everlast-{}-{}-1.part",
                SMALL.id,
                std::process::id()
            )),
            b"child partial",
        )?;
        fs::write(directory.join("child.ready"), b"ready")?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        Err("child fixture timed out waiting for its owner's termination".into())
    }
    #[test]
    fn abrupt_child_exit_releases_os_lock_and_allows_verified_stale_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::process::{Command, Stdio};
        let directory = tempfile::tempdir()?;
        let target = directory.path().join(SMALL.file);
        fs::write(&target, b"previous verified model")?;
        let foreign = directory.path().join("foreign.part");
        fs::write(&foreign, b"keep")?;
        let mut child = OwnedChild(
            Command::new(std::env::current_exe()?)
                .args(["--exact", "model::cache::tests::child_fixture_holds_cache"])
                .env("EVERLAST_CACHE_GUARD_TEST_DIRECTORY", directory.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let child_part =
            directory
                .path()
                .join(format!("everlast-{}-{}-1.part", SMALL.id, child.0.id()));
        let ready = directory.path().join("child.ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !ready.is_file() {
            if child.0.try_wait()?.is_some() {
                return Err("cache child exited before readiness".into());
            }
            if std::time::Instant::now() >= deadline {
                return Err("cache child readiness deadline exceeded".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let cancel = CancellationToken::new();
        assert!(matches!(
            CacheGuard::acquire(directory.path(), SMALL, &cancel),
            Err(ModelError::Busy)
        ));
        assert!(child_part.is_file());
        child.0.kill()?;
        child.0.wait()?;
        let guard = CacheGuard::acquire(directory.path(), SMALL, &cancel)?;
        guard.cleanup_parts(&cancel)?;
        assert!(!child_part.exists());
        assert_eq!(fs::read(&target)?, b"previous verified model");
        assert_eq!(fs::read(&foreign)?, b"keep");
        Ok(())
    }
    #[test]
    fn redirected_cache_directory_is_rejected_before_foreign_folder_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let parent = tempfile::tempdir()?;
        let foreign = tempfile::tempdir()?;
        let part = foreign
            .path()
            .join(format!("everlast-{}-42-1.part", SMALL.id));
        fs::write(&part, b"foreign")?;
        let redirected = parent.path().join("models");
        symlink(foreign.path(), &redirected)?;
        assert!(matches!(
            CacheGuard::acquire(&redirected, SMALL, &CancellationToken::new()),
            Err(ModelError::Io(_))
        ));
        assert_eq!(fs::read(&part)?, b"foreign");
        assert_eq!(fs::read_dir(foreign.path())?.count(), 1);
        Ok(())
    }
    #[test]
    fn bounded_scan_finishes_before_any_deletion() -> Result<(), Box<dyn std::error::Error>> {
        for over_limit in [false, true] {
            let directory = tempfile::tempdir()?;
            let cancel = CancellationToken::new();
            let guard = CacheGuard::acquire(directory.path(), SMALL, &cancel)?;
            let part = directory
                .path()
                .join(format!("everlast-{}-42-1.part", SMALL.id));
            fs::write(&part, b"dead")?;
            for index in 0..(1022 + usize::from(over_limit)) {
                fs::write(directory.path().join(format!("foreign-{index}")), b"keep")?;
            }
            let result = guard.cleanup_with(&cancel, |_| true);
            if over_limit {
                assert!(
                    matches!(result,Err(ModelError::Io(error)) if error.kind()==io::ErrorKind::InvalidData)
                );
                assert!(part.is_file());
            } else {
                result?;
                assert!(!part.exists());
            }
        }
        Ok(())
    }
    #[test]
    fn cancellation_and_changed_liveness_preserve_parts() -> Result<(), Box<dyn std::error::Error>>
    {
        use std::cell::Cell;
        let directory = tempfile::tempdir()?;
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let untouched = directory.path().join("uncreated");
        assert!(matches!(
            CacheGuard::acquire(&untouched, SMALL, &cancelled),
            Err(ModelError::Cancelled)
        ));
        assert!(!untouched.exists());
        let guard = CacheGuard::acquire(directory.path(), SMALL, &CancellationToken::new())?;
        let part = directory
            .path()
            .join(format!("everlast-{}-42-1.part", SMALL.id));
        fs::write(&part, b"keep")?;
        let calls = Cell::new(0);
        assert!(matches!(
            guard.cleanup_with(&cancelled, |_| {
                calls.set(calls.get() + 1);
                true
            }),
            Err(ModelError::Cancelled)
        ));
        assert_eq!(calls.get(), 0);
        assert!(part.is_file());
        let during_scan = CancellationToken::new();
        assert!(matches!(
            guard.cleanup_with(&during_scan, |_| {
                during_scan.cancel();
                true
            }),
            Err(ModelError::Cancelled)
        ));
        assert!(part.is_file());
        let during_delete = CancellationToken::new();
        assert!(matches!(
            guard.cleanup_with(&during_delete, |_| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    during_delete.cancel();
                }
                true
            }),
            Err(ModelError::Cancelled)
        ));
        assert!(part.is_file());
        calls.set(0);
        guard.cleanup_with(&CancellationToken::new(), |_| {
            calls.set(calls.get() + 1);
            calls.get() == 1
        })?;
        assert!(part.is_file());
        Ok(())
    }
    #[test]
    fn symlink_and_directory_lock_paths_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        for linked in [false, true] {
            let directory = tempfile::tempdir()?;
            let lock = directory
                .path()
                .join(format!(".everlast-{}.lock", SMALL.id));
            let target = directory.path().join("foreign");
            fs::write(&target, b"keep")?;
            if linked {
                symlink(&target, &lock)?;
            } else {
                fs::create_dir(&lock)?;
            }
            assert!(matches!(
                CacheGuard::acquire(directory.path(), SMALL, &CancellationToken::new()),
                Err(ModelError::Io(_))
            ));
            assert_eq!(fs::read(&target)?, b"keep");
            assert_eq!(fs::symlink_metadata(lock)?.file_type().is_symlink(), linked);
        }
        Ok(())
    }
    #[test]
    fn cleanup_removes_only_canonical_regular_parts_with_definitely_absent_pids()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir()?;
        let cancel = CancellationToken::new();
        let guard = CacheGuard::acquire(directory.path(), SMALL, &cancel)?;
        let dead = format!("everlast-{}-42-1.part", SMALL.id);
        fs::write(directory.path().join(&dead), b"dead")?;
        let keep = [
            SMALL.file.to_owned(),
            format!("everlast-{}-43-1.part", SMALL.id),
            format!("everlast-{}-44-1.part", SMALL.id),
            "everlast-other-42-1.part".into(),
            format!("everlast-{}-0-1.part", SMALL.id),
            format!("everlast-{}-42-0.part", SMALL.id),
            format!("everlast-{}-042-1.part", SMALL.id),
            format!("everlast-{}-42-01.part", SMALL.id),
            format!("everlast-{}-+42-1.part", SMALL.id),
            format!("everlast-{}-4294967296-1.part", SMALL.id),
            format!("everlast-{}-42-18446744073709551616.part", SMALL.id),
            format!("everlast-{}-42-1.part.extra", SMALL.id),
        ];
        for name in &keep {
            fs::write(directory.path().join(name), b"keep")?;
        }
        let linked = directory
            .path()
            .join(format!("everlast-{}-42-2.part", SMALL.id));
        symlink(directory.path().join(SMALL.file), &linked)?;
        let subdirectory = directory
            .path()
            .join(format!("everlast-{}-42-3.part", SMALL.id));
        fs::create_dir(&subdirectory)?;
        guard.cleanup_with(&cancel, |pid| pid == 42)?;
        assert!(!directory.path().join(dead).exists());
        for name in &keep {
            assert_eq!(fs::read(directory.path().join(name))?, b"keep");
        }
        assert!(fs::symlink_metadata(linked)?.file_type().is_symlink());
        assert!(subdirectory.is_dir());
        Ok(())
    }
    #[test]
    fn hardlinked_lock_is_rejected_without_chmod_or_modifying_its_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("foreign");
        fs::write(&target, b"keep")?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))?;
        let lock = directory
            .path()
            .join(format!(".everlast-{}.lock", SMALL.id));
        fs::hard_link(&target, &lock)?;
        assert!(matches!(
            CacheGuard::acquire(directory.path(), SMALL, &CancellationToken::new()),
            Err(ModelError::Io(_))
        ));
        assert_eq!(fs::read(&target)?, b"keep");
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o644);
        Ok(())
    }
    #[test]
    fn real_lock_contention_and_drop_release_preserve_existing_lock_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let cancel = CancellationToken::new();
        let path = directory
            .path()
            .join(format!(".everlast-{}.lock", SMALL.id));
        fs::write(&path, b"permanent lock payload")?;
        let first = CacheGuard::acquire(directory.path(), SMALL, &cancel)?;
        assert!(matches!(
            CacheGuard::acquire(directory.path(), SMALL, &cancel),
            Err(ModelError::Busy)
        ));
        assert_eq!(fs::read(&path)?, b"permanent lock payload");
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        drop(first);
        drop(CacheGuard::acquire(directory.path(), SMALL, &cancel)?);
        assert!(path.is_file());
        assert_eq!(fs::read(&path)?, b"permanent lock payload");
        Ok(())
    }
}
