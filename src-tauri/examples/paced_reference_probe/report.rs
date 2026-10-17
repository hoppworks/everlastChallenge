//! Private, no-replace publication of complete hypotheses and numeric metadata.
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
type Error = Box<dyn std::error::Error>;
pub fn metadata_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".metadata.json");
    name.into()
}
pub fn check_new(path: &Path) -> Result<(), Error> {
    // symlink_metadata also rejects dangling links as existing destinations.
    for path in [path.to_path_buf(), metadata_path(path)] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Err("report destination already exists".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn write(path: &Path, hypotheses: &[u8], metadata: &[u8]) -> Result<(), Error> {
    check_new(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fn stage(parent: &Path, bytes: &[u8]) -> Result<tempfile::NamedTempFile, Error> {
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(bytes)?;
        file.as_file().sync_all()?;
        Ok(file)
    }
    let hypotheses = stage(parent, hypotheses)?;
    let metadata = stage(parent, metadata)?;
    hypotheses.persist_noclobber(path)?;
    // Each file is atomic, not a cross-file transaction. A publication failure
    // is returned; a successfully published first file remains intact evidence.
    metadata.persist_noclobber(metadata_path(path))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_reports_are_private_and_existing_evidence_is_never_replaced() -> Result<(), Error> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("new.json");
        write(&path, b"private hypotheses", b"numeric metadata")?;
        for file in [&path, &metadata_path(&path)] {
            assert_eq!(std::fs::metadata(file)?.permissions().mode() & 0o777, 0o600);
        }
        assert!(write(&path, b"changed", b"changed").is_err());
        assert_eq!(std::fs::read(&path)?, b"private hypotheses");
        let collision = dir.path().join("collision.json");
        std::fs::write(metadata_path(&collision), b"original")?;
        assert!(write(&collision, b"new", b"new").is_err());
        assert!(!collision.exists());
        Ok(())
    }
}
