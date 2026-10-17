//! Verified atomic model publication. Only the owned partial path is cleaned up.
//! Directory-entry crash durability is not promised: publication completes at
//! rename, so every returned I/O error still belongs to the precommit stage.
use super::{ModelDefinition, ModelError, hex_digest};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static PART_NONCE: AtomicU64 = AtomicU64::new(1);

// Private syscall boundary: tests inject faults while retaining real files.
trait FileIo {
    fn create_directory(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }
    fn create_new(&self, path: &Path) -> io::Result<File> {
        OpenOptions::new().write(true).create_new(true).open(path)
    }
    fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }
    fn flush(&self, file: &mut File) -> io::Result<()> {
        file.flush()
    }
    fn sync_file(&self, file: &File) -> io::Result<()> {
        file.sync_all()
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }
}
struct NativeIo;
impl FileIo for NativeIo {}

pub(super) struct VerifiedFile {
    definition: ModelDefinition,
    file: Option<File>,
    partial: PathBuf,
    target: PathBuf,
    received: u64,
    hasher: Sha256,
    published: bool,
    failed: bool,
}
impl VerifiedFile {
    pub(super) fn new(directory: &Path, definition: ModelDefinition) -> Result<Self, ModelError> {
        Self::new_with(directory, definition, &NativeIo)
    }
    fn new_with(
        directory: &Path,
        definition: ModelDefinition,
        io: &impl FileIo,
    ) -> Result<Self, ModelError> {
        io.create_directory(directory)?;
        for _ in 0..8 {
            let nonce = PART_NONCE.fetch_add(1, Ordering::Relaxed);
            let partial = directory.join(format!(
                "everlast-{}-{}-{nonce}.part",
                definition.id,
                std::process::id()
            ));
            match io.create_new(&partial) {
                Ok(file) => {
                    return Ok(Self {
                        definition,
                        file: Some(file),
                        partial,
                        target: directory.join(definition.file),
                        received: 0,
                        hasher: Sha256::new(),
                        published: false,
                        failed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "model temporary name collisions",
        )
        .into())
    }
    pub(super) fn received(&self) -> u64 {
        self.received
    }
    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        self.write_with(bytes, &NativeIo)
    }
    fn write_with(&mut self, bytes: &[u8], io: &impl FileIo) -> Result<(), ModelError> {
        if self.failed {
            return Err(ModelError::Closed);
        }
        let next = self
            .received
            .checked_add(bytes.len() as u64)
            .ok_or(ModelError::Size)?;
        if next > self.definition.bytes {
            return Err(ModelError::Size);
        }
        if let Err(error) = io.write(self.file.as_mut().ok_or(ModelError::Closed)?, bytes) {
            self.failed = true;
            return Err(error.into());
        }
        self.received = next;
        self.hasher.update(bytes);
        Ok(())
    }
    pub(super) fn finish(self) -> Result<PathBuf, ModelError> {
        self.finish_with(&NativeIo)
    }
    fn finish_with(mut self, io: &impl FileIo) -> Result<PathBuf, ModelError> {
        if self.failed {
            return Err(ModelError::Closed);
        }
        if self.received != self.definition.bytes {
            return Err(ModelError::Size);
        }
        if hex_digest(self.hasher.clone().finalize()) != self.definition.sha256 {
            return Err(ModelError::Hash);
        }
        io.flush(self.file.as_mut().ok_or(ModelError::Closed)?)?;
        io.sync_file(self.file.as_ref().ok_or(ModelError::Closed)?)?;
        drop(self.file.take());
        io.rename(&self.partial, &self.target)?;
        self.published = true;
        Ok(self.target.clone())
    }
}
impl Drop for VerifiedFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.published
            && let Err(error) = fs::remove_file(&self.partial)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("model-part-cleanup-failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> ModelDefinition {
        ModelDefinition {
            id: "fixture",
            file: "fixture.bin",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        }
    }
    #[derive(Clone, Copy)]
    enum Stage {
        Create,
        FullDisk,
        WriteDenied,
        Flush,
        Sync,
        Rename,
    }
    struct FailingIo(Stage);
    impl FileIo for FailingIo {
        fn create_new(&self, path: &Path) -> io::Result<File> {
            if matches!(self.0, Stage::Create) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected create denial",
                ));
            }
            NativeIo.create_new(path)
        }
        fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
            match self.0 {
                Stage::FullDisk => {
                    // A real prefix reaches disk before ENOSPC, as can happen
                    // during write_all. Such a writer must never publish later.
                    file.write_all(&bytes[..bytes.len().min(1)])?;
                    Err(io::Error::from_raw_os_error(28))
                }
                Stage::WriteDenied => Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected write denial",
                )),
                _ => NativeIo.write(file, bytes),
            }
        }
        fn flush(&self, file: &mut File) -> io::Result<()> {
            if matches!(self.0, Stage::Flush) {
                return Err(io::Error::other("injected flush failure"));
            }
            NativeIo.flush(file)
        }
        fn sync_file(&self, file: &File) -> io::Result<()> {
            if matches!(self.0, Stage::Sync) {
                return Err(io::Error::other("injected sync failure"));
            }
            NativeIo.sync_file(file)
        }
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            if matches!(self.0, Stage::Rename) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected rename denial",
                ));
            }
            NativeIo.rename(from, to)
        }
    }
    struct CollisionIo;
    impl FileIo for CollisionIo {
        fn create_new(&self, path: &Path) -> io::Result<File> {
            fs::write(path, b"another writer")?;
            NativeIo.create_new(path)
        }
    }
    #[test]
    fn partial_disk_full_write_cannot_be_retried_into_a_false_verified_publish()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("fixture.bin");
        fs::write(&target, b"abc")?;
        let mut output = VerifiedFile::new(directory.path(), fixture())?;
        output.write(b"a")?;
        assert!(
            matches!(output.write_with(b"bc",&FailingIo(Stage::FullDisk)),Err(ModelError::Io(error)) if error.raw_os_error()==Some(28))
        );
        assert_eq!(output.received(), 1);
        assert!(matches!(output.write(b"bc"), Err(ModelError::Closed)));
        assert!(matches!(output.finish(), Err(ModelError::Closed)));
        assert_eq!(fs::read(&target)?, b"abc");
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
    #[test]
    fn occupied_temporary_names_are_never_deleted_or_overwritten()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        assert!(
            matches!(VerifiedFile::new_with(directory.path(),fixture(),&CollisionIo),Err(ModelError::Io(error)) if error.kind()==io::ErrorKind::AlreadyExists)
        );
        assert_eq!(fs::read_dir(directory.path())?.count(), 8);
        for entry in fs::read_dir(directory.path())? {
            assert_eq!(fs::read(entry?.path())?, b"another writer");
        }
        Ok(())
    }
    #[test]
    fn size_hash_and_abandonment_failures_leave_old_model_and_foreign_part_untouched()
    -> Result<(), Box<dyn std::error::Error>> {
        for bytes in [b"ab".as_slice(), b"abd".as_slice(), b"abcd".as_slice()] {
            let directory = tempfile::tempdir()?;
            let target = directory.path().join("fixture.bin");
            fs::write(&target, b"abc")?;
            let foreign = directory.path().join("another-download.part");
            fs::write(&foreign, b"keep")?;
            let mut output = VerifiedFile::new(directory.path(), fixture())?;
            let result = output.write(bytes).and_then(|()| output.finish());
            assert!(matches!(result, Err(ModelError::Size | ModelError::Hash)));
            assert_eq!(fs::read(&target)?, b"abc");
            assert_eq!(fs::read(&foreign)?, b"keep");
            assert_eq!(fs::read_dir(directory.path())?.count(), 2);
            let mut abandoned = VerifiedFile::new(directory.path(), fixture())?;
            abandoned.write(b"a")?;
            drop(abandoned);
            assert_eq!(fs::read_dir(directory.path())?.count(), 2);
        }
        Ok(())
    }
    #[test]
    fn io_faults_preserve_old_target_and_remove_only_owned_partial()
    -> Result<(), Box<dyn std::error::Error>> {
        for stage in [
            Stage::Create,
            Stage::FullDisk,
            Stage::WriteDenied,
            Stage::Flush,
            Stage::Sync,
            Stage::Rename,
        ] {
            let directory = tempfile::tempdir()?;
            let target = directory.path().join("fixture.bin");
            fs::write(&target, b"abc")?;
            let foreign = directory.path().join("foreign.part");
            fs::write(&foreign, b"unowned")?;
            let io = FailingIo(stage);
            let result = (|| {
                let mut output = VerifiedFile::new_with(directory.path(), fixture(), &io)?;
                output.write_with(b"abc", &io)?;
                output.finish_with(&io)
            })();
            assert!(matches!(result, Err(ModelError::Io(_))));
            assert_eq!(fs::read(&target)?, b"abc");
            assert_eq!(fs::read(&foreign)?, b"unowned");
            assert_eq!(fs::read_dir(directory.path())?.count(), 2);
        }
        Ok(())
    }
    #[test]
    fn publishes_only_verified_bytes_and_preserves_unowned_partial_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let foreign = directory.path().join("foreign.part");
        fs::write(&foreign, b"foreign")?;
        let target = directory.path().join("fixture.bin");
        fs::write(&target, b"old")?;
        let mut output = VerifiedFile::new(directory.path(), fixture())?;
        output.write(b"ab")?;
        assert_eq!(output.received(), 2);
        assert_eq!(fs::read(&target)?, b"old");
        output.write(b"c")?;
        assert_eq!(output.finish()?, target);
        assert_eq!(fs::read(&target)?, b"abc");
        assert_eq!(fs::read(&foreign)?, b"foreign");
        assert_eq!(fs::read_dir(directory.path())?.count(), 2);
        Ok(())
    }
}
