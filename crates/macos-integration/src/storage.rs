//! Available filesystem capacity for model staging. No main-thread affinity.
use std::{ffi::CString, io, mem::MaybeUninit, os::unix::ffi::OsStrExt, path::Path};

pub fn available_bytes(path: &Path) -> io::Result<u64> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "filesystem path contains NUL"))?;
    let mut statistics = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: the CString remains alive and NUL-terminated for the call; the
    // output pointer is aligned storage for exactly one statfs structure.
    if unsafe { libc::statfs(path.as_ptr(), statistics.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful statfs initialized the complete output structure.
    let statistics = unsafe { statistics.assume_init() };
    statistics
        .f_bavail
        .checked_mul(u64::from(statistics.f_bsize))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "filesystem capacity overflows"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_filesystem_reports_capacity_and_missing_path_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        assert!(available_bytes(&std::env::temp_dir())? > 0);
        let missing =
            std::env::temp_dir().join(format!("everlast-missing-capacity-{}", std::process::id()));
        assert!(available_bytes(&missing).is_err());
        let invalid = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/invalid\0path"));
        assert_eq!(
            available_bytes(invalid)
                .err()
                .ok_or("NUL path accepted")?
                .kind(),
            io::ErrorKind::InvalidInput
        );
        Ok(())
    }
}
