#![forbid(unsafe_code)]

fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "app_snapshot",
            "model_download",
            "model_cancel",
            "refinement_download",
            "refinement_cancel_setup",
            "recording_start",
            "recording_stop",
            "recording_cancel",
            "transcript_clear",
            "transcript_copy",
            "microphone_request",
            "accessibility_request",
            "settings_save",
            "settings_reset",
            "devices_refresh",
        ]));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("Tauri build configuration failed: {error}");
        std::process::exit(1);
    }
}
