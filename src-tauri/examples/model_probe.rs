//! Developer probe; this does not seed the installed app's first-run directory.
use everlast_dictation::model::{self, REFERENCE, SMALL, VAD};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err(
            "usage: model_probe <verify-reference|load-reference|download-reference|download-small|download-vad|setup-check|setup-download> <directory>"
                .into(),
        );
    }
    let directory = Path::new(&arguments[2]);
    match arguments[1].as_str() {
        "setup-check" | "setup-download" => {
            use everlast_dictation::setup::{Phase, SetupService};
            use std::time::{Duration, Instant};
            let setup = SetupService::new(directory.to_path_buf(), REFERENCE);
            setup.start(arguments[1] == "setup-download")?;
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut previous = None;
            let mut received = 0;
            let outcome: Result<(), Box<dyn std::error::Error>> = loop {
                let state = setup.snapshot()?;
                if state.total != REFERENCE.bytes() + VAD.bytes()
                    || state.received > state.total
                    || state.received < received
                {
                    break Err("invalid combined setup progress".into());
                }
                received = state.received;
                if previous != Some(state.phase) {
                    println!(
                        "setup {:?}: {} / {}",
                        state.phase, state.received, state.total
                    );
                    previous = Some(state.phase);
                }
                match state.phase {
                    Phase::Ready => break Ok(()),
                    Phase::Missing | Phase::Failed | Phase::Cancelled => {
                        break Err(format!(
                            "setup not ready: {:?} {:?}",
                            state.phase, state.failure
                        )
                        .into());
                    }
                    _ if Instant::now() >= deadline => {
                        break Err("setup observation timed out".into());
                    }
                    _ => std::thread::sleep(Duration::from_millis(5)),
                }
            };
            // Join initialization even on observation failure; a timeout is not
            // a claimed hard backend interrupt.
            setup.shutdown()?;
            outcome?;
        }
        "load-reference" => {
            let engine = everlast_dictation::transcription::Engine::load(
                &directory.join(REFERENCE.file_name()),
                REFERENCE,
            )?;
            println!("context loaded");
            drop(engine);
        }
        "verify-reference" => {
            model::verify_file(&directory.join(REFERENCE.file_name()), REFERENCE)?
        }
        "download-reference" | "download-small" | "download-vad" => {
            let definition = match arguments[1].as_str() {
                "download-small" => SMALL,
                "download-vad" => VAD,
                _ => REFERENCE,
            };
            let cancel = tokio_util::sync::CancellationToken::new();
            let mut displayed = 0;
            let path = model::download_model(directory, definition, &cancel, |bytes| {
                if bytes - displayed >= 16 * 1024 * 1024 {
                    eprintln!("received {bytes} / {} bytes", definition.bytes());
                    displayed = bytes;
                }
            })?;
            model::verify_file(&path, definition)?;
        }
        _ => return Err("unknown model probe action".into()),
    }
    println!("verified");
    Ok(())
}
