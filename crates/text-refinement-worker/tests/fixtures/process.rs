//! Opt-in process fixture, never part of the distributed application.
use std::{io::Write, num::NonZeroU64, path::PathBuf};
use text_refinement_protocol::{Request, Response, read_request, write_response};
struct ExitMarker(PathBuf);
impl Drop for ExitMarker {
    fn drop(&mut self) {
        if std::fs::write(self.0.join("child-exited"), b"exited").is_err() {
            eprintln!("fixture exit marker failed");
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let model = PathBuf::from(args.next().ok_or("model")?);
    let generation = args
        .next()
        .ok_or("generation")?
        .into_string()
        .map_err(|_| "generation")?
        .parse::<NonZeroU64>()?;
    let scratch = PathBuf::from(args.next().ok_or("scratch")?);
    let _exit = ExitMarker(model.clone());
    let _lease =
        text_refinement_worker::manifest::owner_lease(&scratch).map_err(|_| "owner lease")?;
    std::fs::write(scratch.join("model-snapshot"), b"fixture")?;
    let mode = model.file_name().and_then(|s| s.to_str()).ok_or("mode")?;
    if mode == "startup-hang" {
        loop {
            std::thread::park();
        }
    }
    let mut output = std::io::stdout();
    if mode == "malformed" {
        output.write_all(&u32::MAX.to_be_bytes())?;
        return Ok(());
    }
    write_response(
        &mut output,
        &Response::Ready {
            generation: if mode == "wrong-generation" {
                NonZeroU64::new(generation.get() + 1).ok_or("generation")?
            } else {
                generation
            },
            model_manifest_sha256: if mode == "wrong-manifest" {
                "f".repeat(64)
            } else {
                "0".repeat(64)
            },
            prompt_version: if mode == "wrong-prompt" { 5 } else { 6 },
            policy_version: if mode == "wrong-policy" { 1 } else { 2 },
        },
    )?;
    if mode == "stdout-flood" {
        loop {
            write_response(
                &mut output,
                &Response::Ready {
                    generation,
                    model_manifest_sha256: "0".repeat(64),
                    prompt_version: 6,
                    policy_version: 2,
                },
            )?;
        }
    }
    if mode == "no-read" {
        loop {
            std::thread::park();
        }
    }
    let mut input = std::io::stdin();
    while let Some(request) = read_request(&mut input)? {
        match request {
            Request::Shutdown {} => return Ok(()),
            Request::Cancel { job } => write_response(&mut output, &Response::Cancelled { job })?,
            Request::Run { job, .. } => {
                std::fs::write(scratch.join("run-started"), b"started")?;
                let mut trace = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(model.join("requests.log"))?;
                writeln!(
                    trace,
                    "{} {} {}",
                    std::process::id(),
                    generation,
                    job.session
                )?;
                if mode == "late" {
                    while let Some(request) = read_request(&mut input)? {
                        if matches!(request, Request::Shutdown {}) {
                            write_response(
                                &mut output,
                                &Response::Completed {
                                    job,
                                    edits: Vec::new(),
                                },
                            )?;
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                if mode == "stale" {
                    let mut stale = job;
                    stale.session = NonZeroU64::new(job.session.get() + 1).ok_or("session")?;
                    write_response(
                        &mut output,
                        &Response::Completed {
                            job: stale,
                            edits: Vec::new(),
                        },
                    )?;
                    continue;
                }
                if mode == "invalid-edit" {
                    use dictation_core::refinement::{EditKind, ProposedEdit};
                    write_response(
                        &mut output,
                        &Response::Completed {
                            job,
                            edits: vec![ProposedEdit {
                                kind: EditKind::Filler,
                                before: "Hello".into(),
                                after: "Changed".into(),
                                left_context: None,
                                right_context: None,
                            }],
                        },
                    )?;
                    continue;
                }
                if mode == "stderr-flood" {
                    let mut error_output = std::io::stderr();
                    for _ in 0..32 {
                        error_output.write_all(&[b'x'; 4096])?;
                    }
                }
                if mode == "hang" {
                    loop {
                        std::thread::park();
                    }
                }
                write_response(
                    &mut output,
                    &Response::Completed {
                        job,
                        edits: Vec::new(),
                    },
                )?;
                if mode == "self-idle" || mode == "idle-crash" {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                    while !model.join("allow-idle-exit").exists() {
                        if std::time::Instant::now() >= deadline {
                            return Err("idle fixture deadline".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    return if mode == "self-idle" {
                        Ok(())
                    } else {
                        Err("idle fixture crash".into())
                    };
                }
            }
        }
    }
    Ok(())
}
