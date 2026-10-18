//! Explicit developer-only shutdown export. No IPC or recording command.
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const ENVIRONMENT: &str = "EVERLAST_NATIVE_START_OBSERVATIONS";
pub const MAX_BYTES: usize = 128 * 1024;
#[derive(Clone)]
pub struct Target {
    path: PathBuf,
}
impl Target {
    pub fn from_option(value: Option<OsString>) -> Result<Option<Self>, &'static str> {
        let Some(value) = value else {
            return Ok(None);
        };
        let path = PathBuf::from(value);
        parent(&path)?;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Some(Self { path })),
            _ => Err("observation-target-not-fresh"),
        }
    }
}
fn parent(path: &Path) -> Result<(), &'static str> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err("observation-absolute-target-required");
    }
    let p = path.parent().ok_or("observation-parent-required")?;
    let actual = p
        .canonicalize()
        .map_err(|_| "observation-parent-unavailable")?;
    let metadata = fs::symlink_metadata(p).map_err(|_| "observation-parent-unavailable")?;
    if actual != p
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err("observation-canonical-private-parent-required");
    }
    Ok(())
}
pub fn export_if_selected(
    target: Option<&Target>,
    snapshot: impl FnOnce() -> Result<Vec<u8>, &'static str>,
) -> Result<bool, &'static str> {
    let Some(target) = target else {
        return Ok(false);
    };
    let bytes = std::panic::catch_unwind(std::panic::AssertUnwindSafe(snapshot))
        .map_err(|_| "observation-getter-panicked")??;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("observation-output-bound");
    }
    parent(&target.path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target.path)
        .map_err(|_| "observation-target-create-failed")?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| "observation-private-mode-failed")?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "observation-write-incomplete")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::{
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let p = std::env::temp_dir().canonicalize()?.join(format!(
                "everlast-start-export-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&p)?;
            Ok(Self(p))
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _cleanup = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn no_optin_does_not_read_getter_or_create_a_file() -> Result<(), Box<dyn std::error::Error>> {
        let count = Cell::new(0);
        assert!(Target::from_option(None)?.is_none());
        assert!(!export_if_selected(None, || {
            count.set(count.get() + 1);
            Ok(b"unused".to_vec())
        })?);
        assert_eq!(count.get(), 0);
        Ok(())
    }
    #[test]
    fn incomplete_observation_is_kept_exactly_in_private_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let d = Directory::new()?;
        let p = d.0.join("observation.json");
        let target = Target::from_option(Some(p.clone().into_os_string()))?.ok_or("target")?;
        let bytes=br#"{"pendingOrdinal":1,"observerContentionOrPoison":true,"records":[{"observedUpperNanos":null,"reason":"shutdown"}],"qualification":false}"#;
        assert!(export_if_selected(Some(&target), || Ok(bytes.to_vec()))?);
        assert_eq!(fs::read(&p)?, bytes);
        assert_eq!(fs::metadata(&p)?.permissions().mode() & 0o777, 0o600);
        Ok(())
    }
    #[test]
    fn target_created_after_optin_is_never_replaced() -> Result<(), Box<dyn std::error::Error>> {
        let d = Directory::new()?;
        let p = d.0.join("observation.json");
        let target = Target::from_option(Some(p.clone().into_os_string()))?.ok_or("target")?;
        fs::write(&p, b"existing evidence")?;
        assert!(export_if_selected(Some(&target), || Ok(b"new evidence".to_vec())).is_err());
        assert_eq!(fs::read(&p)?, b"existing evidence");
        Ok(())
    }
    #[test]
    fn public_parent_and_oversize_do_not_produce_an_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let d = Directory::new()?;
        let p = d.0.join("observation.json");
        fs::set_permissions(&d.0, fs::Permissions::from_mode(0o755))?;
        assert!(Target::from_option(Some(p.clone().into_os_string())).is_err());
        fs::set_permissions(&d.0, fs::Permissions::from_mode(0o700))?;
        let target = Target::from_option(Some(p.clone().into_os_string()))?.ok_or("target")?;
        assert!(export_if_selected(Some(&target), || Ok(vec![b'x'; MAX_BYTES + 1])).is_err());
        assert!(!p.exists());
        Ok(())
    }
    #[test]
    fn getter_panic_is_an_export_error_and_does_not_create_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let d = Directory::new()?;
        let p = d.0.join("observation.json");
        let target = Target::from_option(Some(p.clone().into_os_string()))?.ok_or("target")?;
        let result = export_if_selected(Some(&target), || -> Result<Vec<u8>, &'static str> {
            std::panic::resume_unwind(Box::new("controlled export panic"))
        });
        assert!(result.is_err());
        assert!(!p.exists());
        Ok(())
    }
}
