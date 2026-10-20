//! Actual pinned one-time model conversion; no inference or network required.
#![forbid(unsafe_code)]
use everlast_dictation::model::{MLX_REFERENCE_COEFFICIENTS, download_model, verify_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(1..=2).contains(&args.len()) {
        return Err("OWN_MODEL_DIRECTORY_WITH_REFERENCE".into());
    }
    let directory = std::path::Path::new(&args[0]);
    let mut last = 0;
    let cancel = tokio_util::sync::CancellationToken::new();
    if args.get(1).is_some_and(|s| s == "--cancel-after-progress") {
        let result = download_model(directory, MLX_REFERENCE_COEFFICIENTS, &cancel, |n| {
            if n > 0 {
                cancel.cancel();
            }
        });
        assert!(matches!(
            result,
            Err(everlast_dictation::model::ModelError::Cancelled)
        ));
        verify_file(
            &directory.join(MLX_REFERENCE_COEFFICIENTS.file_name()),
            MLX_REFERENCE_COEFFICIENTS,
        )?;
        assert!(
            !std::fs::read_dir(directory)?
                .filter_map(Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "part"))
        );
        println!("GREEN: cancellation removed partial and preserved verified model");
        return Ok(());
    }
    let path = download_model(directory, MLX_REFERENCE_COEFFICIENTS, &cancel, |n| {
        assert!(n >= last);
        last = n;
    })?;
    assert_eq!(last, MLX_REFERENCE_COEFFICIENTS.bytes());
    verify_file(&path, MLX_REFERENCE_COEFFICIENTS)?;
    println!(
        "GREEN: exact {} bytes and SHA {}",
        last,
        MLX_REFERENCE_COEFFICIENTS.sha256()
    );
    Ok(())
}
