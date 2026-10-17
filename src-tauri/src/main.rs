#![forbid(unsafe_code)]

fn main() {
    if let Some(result) = macos_integration::clipboard::snapshot_helper_main() {
        if result.is_err() {
            eprintln!("clipboard-backup-failed");
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("Application startup failed: {error}");
        std::process::exit(1);
    }
}
