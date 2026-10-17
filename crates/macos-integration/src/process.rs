//! Read-only process-existence checks. An uncertain result never proves absence.

/// True only when the OS explicitly reports that this positive PID is absent.
/// Permission failures and invalid identifiers are inconclusive. This is a
/// point-in-time observation; it does not establish a process's identity.
pub fn definitely_absent(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: pid is a positive, representable process identifier. Signal zero
    // performs only existence/permission checking and sends no signal. No
    // pointers or borrowed resources cross the FFI boundary.
    let result = unsafe { libc::kill(pid, 0) };
    let errno = if result == -1 {
        // Capture this thread's errno immediately after the failed OS query.
        std::io::Error::last_os_error().raw_os_error()
    } else {
        None
    };
    query_proves_absence(result, errno)
}

fn query_proves_absence(result: libc::c_int, errno: Option<libc::c_int>) -> bool {
    result == -1 && errno == Some(libc::ESRCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_and_invalid_identifiers_are_not_declared_absent() {
        assert!(!definitely_absent(std::process::id()));
        assert!(!definitely_absent(0));
        assert!(!definitely_absent(u32::MAX));
    }

    #[test]
    fn operating_system_confirms_absence_outside_its_process_identifier_range() {
        assert!(definitely_absent(i32::MAX as u32));
    }

    #[test]
    fn only_explicit_esrch_from_a_failed_query_proves_absence() {
        assert!(query_proves_absence(-1, Some(libc::ESRCH)));
        for errno in [
            Some(libc::EPERM),
            Some(libc::EACCES),
            Some(libc::EINTR),
            None,
        ] {
            assert!(!query_proves_absence(-1, errno));
        }
        // errno is unspecified after success and must not override its result.
        assert!(!query_proves_absence(0, Some(libc::ESRCH)));
        assert!(!query_proves_absence(1, Some(libc::ESRCH)));
    }
}
