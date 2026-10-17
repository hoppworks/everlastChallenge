//! Bounded read-only host description, captured outside all measured runs.
use serde_json::{Value, json};
use std::{
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn read(program: &str, args: &[&str]) -> Result<String, String> {
    let mut output = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::from(
            output.as_file().try_clone().map_err(|e| e.to_string())?,
        ))
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let size = output.as_file().metadata().map(|m| m.len());
        if size.as_ref().is_err()
            || size.as_ref().is_ok_and(|n| *n > 65536)
            || Instant::now() >= deadline
        {
            let killed = child.kill();
            let waited = child.wait();
            return Err(format!(
                "host command budget/output failure; kill={killed:?}; wait={waited:?}"
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("host command exit {status}"));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(error) => {
                let killed = child.kill();
                let waited = child.wait();
                return Err(format!(
                    "host command observation {error}; kill={killed:?}; wait={waited:?}"
                ));
            }
        }
    }
    output
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    output
        .as_file_mut()
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("host command output exceeds64KiB".into());
    }
    String::from_utf8(bytes)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "host command output is not UTF8".into())
}
fn field(result: Result<String, String>) -> Value {
    match result {
        Ok(value) => json!({"value":value,"error":null}),
        Err(error) => json!({"value":null,"error":error}),
    }
}
pub fn observe() -> Value {
    let power = read("/usr/bin/pmset", &["-g", "batt"]).map(|text| {
        if text.contains("AC Power") {
            "AC Power"
        } else if text.contains("Battery Power") {
            "Battery Power"
        } else {
            "unknown"
        }
        .to_owned()
    });
    json!({"architecture":std::env::consts::ARCH,"os":std::env::consts::OS,
        "cpu":field(read("/usr/sbin/sysctl",&["-n","machdep.cpu.brand_string"])),
        "memoryBytes":field(read("/usr/sbin/sysctl",&["-n","hw.memsize"])),
        "macOS":field(read("/usr/bin/sw_vers",&["-productVersion"])),"macOSBuild":field(read("/usr/bin/sw_vers",&["-buildVersion"])),
        "rustc":field(read("rustc",&["--version"])),"powerSource":field(power),
        "scope":"read-only description before model initialization; no background-load or hardware qualification; OS calls are not hard realtime"})
}
