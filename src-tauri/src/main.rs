#![forbid(unsafe_code)]

pub mod control;

fn main() {
    if let Err(error) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("Application startup failed: {error}");
        std::process::exit(1);
    }
}
