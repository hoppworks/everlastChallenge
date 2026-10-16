//! Developer probe; this does not seed the installed app's first-run directory.
use everlast_dictation::model::{self, REFERENCE, SMALL};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err(
            "usage: model_probe <verify-reference|load-reference|download-reference|download-small> <directory>"
                .into(),
        );
    }
    let directory = Path::new(&arguments[2]);
    match arguments[1].as_str() {
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
        "download-reference" | "download-small" => {
            let definition = if arguments[1] == "download-small" {
                SMALL
            } else {
                REFERENCE
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
