fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[]));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("Tauri build configuration failed: {error}");
        std::process::exit(1);
    }
}
