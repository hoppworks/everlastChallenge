#![forbid(unsafe_code)]

#[cfg(all(feature = "mlx", target_os = "macos", target_arch = "aarch64"))]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::{num::NonZeroU64, path::PathBuf, sync::Arc, time::Duration};
    use text_refinement_worker::{
        Identity, manifest,
        model::{MlxGenerator, POLICY_VERSION, PROMPT_VERSION},
        serve,
    };
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(args.next().ok_or("model directory required")?);
    let generation = args
        .next()
        .ok_or("generation required")?
        .into_string()
        .map_err(|_| "invalid generation")?
        .parse::<NonZeroU64>()?;
    let owner_directory = PathBuf::from(args.next().ok_or("snapshot owner directory required")?);
    if args.next().is_some() {
        return Err("unexpected worker argument".into());
    }
    let _owner_lease =
        manifest::owner_lease(&owner_directory).map_err(|_| "snapshot owner lease unavailable")?;
    serve(
        std::io::stdin(),
        std::io::stdout(),
        Identity {
            generation,
            model_manifest_sha256: manifest::fingerprint(),
            prompt_version: PROMPT_VERSION,
            policy_version: POLICY_VERSION,
        },
        Duration::from_secs(120),
        |cancel| MlxGenerator::load(&directory, &owner_directory, cancel),
        Arc::new(|code| std::process::exit(code)),
    )?;
    Ok(())
}

#[cfg(not(all(feature = "mlx", target_os = "macos", target_arch = "aarch64")))]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    Err("local MLX text refinement requires Apple Silicon macOS".into())
}

fn main() -> std::process::ExitCode {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("text-refinement-worker: unexpected failure")
    }));
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("text-refinement-worker: failed");
            std::process::ExitCode::FAILURE
        }
    }
}
