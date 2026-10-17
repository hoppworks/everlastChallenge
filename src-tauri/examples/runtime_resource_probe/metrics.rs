//! Bounded developer OS observations; no leak/heap qualification is inferred.

use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

const MAX_OUTPUT: u64 = 1024 * 1024;
const COMMAND_BUDGET: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub rss_bytes: Option<u64>,
    pub rss_error: Option<String>,
    pub vmmap_summary: Option<String>,
    pub vmmap_error: Option<String>,
    pub elapsed_micros: u128,
}

/// Read-only process observations. Each child has a 2-second polling budget
/// and a 1-MiB combined output limit. OS calls themselves are not hard real-time
/// operations. Tempfiles avoid pipe backpressure and reader-thread lifetimes;
/// polling detects oversized output, which may briefly overshoot on disk.
pub fn observe(pid: u32, include_vmmap: bool) -> Observation {
    let started = Instant::now();
    let mut observation = Observation {
        rss_bytes: None,
        rss_error: None,
        vmmap_summary: None,
        vmmap_error: None,
        elapsed_micros: 0,
    };
    if pid == 0 || pid > i32::MAX as u32 {
        observation.rss_error = Some("PID must be a positive macOS process ID".into());
        if include_vmmap {
            observation.vmmap_error = observation.rss_error.clone();
        }
    } else {
        let pid = pid.to_string();
        match run_command("/bin/ps", &["-o", "rss=", "-p", &pid], COMMAND_BUDGET)
            .and_then(|output| parse_rss(&output))
        {
            Ok(bytes) => observation.rss_bytes = Some(bytes),
            Err(error) => observation.rss_error = Some(error),
        }
        if include_vmmap {
            match run_command("/usr/bin/vmmap", &["-summary", &pid], COMMAND_BUDGET).and_then(
                |output| String::from_utf8(output).map_err(|_| "vmmap output is not UTF-8".into()),
            ) {
                Ok(summary) if !summary.trim().is_empty() => {
                    observation.vmmap_summary = Some(summary)
                }
                Ok(_) => observation.vmmap_error = Some("vmmap returned an empty summary".into()),
                Err(error) => observation.vmmap_error = Some(error),
            }
        }
    }
    observation.elapsed_micros = started.elapsed().as_micros();
    observation
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl OwnedChild {
    fn terminate(&mut self) -> Result<(), String> {
        // A child may exit between try_wait and kill; successful wait confirms
        // reaping in either case. Never mistake a kill request for cleanup.
        let kill = self.child.kill();
        match self.child.wait() {
            Ok(_) => {
                self.reaped = true;
                Ok(())
            }
            Err(error) => Err(format!(
                "child cleanup not confirmed: kill={kill:?}, wait={error}"
            )),
        }
    }
    fn failure(&mut self, reason: &str) -> String {
        match self.terminate() {
            Ok(()) => format!("{reason}; child reaped"),
            Err(error) => format!("{reason}; {error}"),
        }
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            // Best-effort unwind guard; ordinary failures report cleanup status.
            let _cleanup = self.terminate();
        }
    }
}

fn within_output_bound(stdout: &File, stderr: &File) -> Result<(), String> {
    let stdout = stdout
        .metadata()
        .map_err(|error| format!("stdout metadata failed: {error}"))?
        .len();
    let stderr = stderr
        .metadata()
        .map_err(|error| format!("stderr metadata failed: {error}"))?
        .len();
    if stdout
        .checked_add(stderr)
        .is_none_or(|total| total > MAX_OUTPUT)
    {
        return Err("process output exceeded 1 MiB".into());
    }
    Ok(())
}

fn read_bounded(file: &mut File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("output seek failed: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("output read failed: {error}"))?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("process output exceeded 1 MiB".into());
    }
    Ok(bytes)
}

fn run_command(program: &str, arguments: &[&str], budget: Duration) -> Result<Vec<u8>, String> {
    let mut stdout =
        NamedTempFile::new().map_err(|error| format!("stdout tempfile failed: {error}"))?;
    let mut stderr =
        NamedTempFile::new().map_err(|error| format!("stderr tempfile failed: {error}"))?;
    let output = stdout
        .as_file()
        .try_clone()
        .map_err(|error| format!("stdout handle failed: {error}"))?;
    let diagnostic = stderr
        .as_file()
        .try_clone()
        .map_err(|error| format!("stderr handle failed: {error}"))?;
    let started = Instant::now();
    let child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(diagnostic)
        .spawn()
        .map_err(|error| format!("process spawn failed: {error}"))?;
    let mut child = OwnedChild {
        child,
        reaped: false,
    };
    loop {
        if started.elapsed() >= budget {
            return Err(child.failure("process timed out"));
        }
        if let Err(error) = within_output_bound(stdout.as_file(), stderr.as_file()) {
            return Err(child.failure(&error));
        }
        match child.child.try_wait() {
            Ok(Some(status)) => {
                child.reaped = true;
                within_output_bound(stdout.as_file(), stderr.as_file())?;
                if !status.success() {
                    let diagnostic = read_bounded(stderr.as_file_mut())?;
                    let diagnostic: String = String::from_utf8_lossy(&diagnostic)
                        .chars()
                        .take(512)
                        .collect();
                    return Err(format!("process failed ({status}): {diagnostic}"));
                }
                return read_bounded(stdout.as_file_mut());
            }
            Ok(None) => {}
            Err(error) => return Err(child.failure(&format!("process status failed: {error}"))),
        }
        if started.elapsed() >= budget {
            return Err(child.failure("process timed out"));
        }
        thread::sleep(
            budget
                .saturating_sub(started.elapsed())
                .min(Duration::from_millis(2)),
        );
    }
}

fn parse_rss(output: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(output)
        .map_err(|_| "RSS output is not UTF-8")?
        .trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("RSS output must contain one decimal KiB value".into());
    }
    text.parse::<u64>()
        .ok()
        .and_then(|kib| kib.checked_mul(1024))
        .ok_or_else(|| "RSS KiB-to-bytes conversion overflowed".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_is_a_single_decimal_kib_value_with_checked_byte_conversion() -> Result<(), String> {
        assert_eq!(parse_rss(b" 4096\n")?, 4_194_304);
        assert_eq!(parse_rss(b"0\n")?, 0);
        for invalid in [
            b"".as_slice(),
            b"-1",
            b"+1",
            b"1 2",
            b"1KiB",
            b"18446744073709551615",
            &[255],
        ] {
            assert!(parse_rss(invalid).is_err());
        }
        Ok(())
    }
    #[test]
    fn self_rss_probe_keeps_omitted_vmmap_absent() {
        let observation = observe(std::process::id(), false);
        assert!(
            observation.rss_bytes.is_some(),
            "{:?}",
            observation.rss_error
        );
        assert!(observation.rss_error.is_none());
        assert!(observation.vmmap_summary.is_none());
        assert!(observation.vmmap_error.is_none());
    }

    #[test]
    fn owned_child_timeout_is_killed_and_reaped() {
        let error = run_command("/bin/sleep", &["10"], std::time::Duration::from_millis(20));
        assert!(
            matches!(error, Err(message) if message.contains("timed out") && message.contains("child reaped"))
        );
    }
    #[test]
    fn combined_output_bound_is_inclusive_and_bounded_read_rejects_oversize()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = NamedTempFile::new()?;
        let stderr = NamedTempFile::new()?;
        stdout.as_file().set_len(524_288)?;
        stderr.as_file().set_len(524_288)?;
        assert!(within_output_bound(stdout.as_file(), stderr.as_file()).is_ok());
        stderr.as_file().set_len(524_289)?;
        assert!(within_output_bound(stdout.as_file(), stderr.as_file()).is_err());
        stdout.as_file().set_len(1_048_577)?;
        assert!(read_bounded(stdout.as_file_mut()).is_err());
        stdout.as_file().set_len(1_048_576)?;
        assert_eq!(read_bounded(stdout.as_file_mut())?.len(), 1_048_576);
        Ok(())
    }

    #[test]
    fn actual_oversized_child_output_and_nonzero_exit_remain_errors() {
        let oversized = run_command(
            "/usr/bin/head",
            &["-c", "1048577", "/dev/zero"],
            COMMAND_BUDGET,
        );
        assert!(matches!(oversized, Err(error) if error.contains("exceeded 1 MiB")));
        let unsuccessful = run_command(
            "/bin/sh",
            &["-c", "printf diagnostic >&2; exit 7"],
            COMMAND_BUDGET,
        );
        assert!(
            matches!(unsuccessful, Err(error) if error.contains("process failed") && error.contains("diagnostic"))
        );
        assert!(run_command("/definitely-absent-everlast-command", &[], COMMAND_BUDGET).is_err());
    }

    #[test]
    fn invalid_pid_returns_explicit_errors_without_fabricated_zero_values() {
        for pid in [0, u32::MAX] {
            let observation = observe(pid, true);
            assert!(observation.rss_bytes.is_none());
            assert!(observation.rss_error.is_some());
            assert!(observation.vmmap_summary.is_none());
            assert!(observation.vmmap_error.is_some());
        }
    }
}
