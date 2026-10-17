#![forbid(unsafe_code)]
use everlast_dictation::{desktop::Desktop, ipc};
use macos_integration::power::{PowerEvent, PowerObserver};
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::app_snapshot,
            ipc::model_download,
            ipc::model_cancel,
            ipc::recording_start,
            ipc::recording_stop,
            ipc::recording_cancel,
            ipc::transcript_copy,
            ipc::transcript_clear,
            ipc::microphone_request,
            ipc::accessibility_request
        ])
        .setup(|app| {
            let desktop = Desktop::spawn(app.handle().clone()).map_err(std::io::Error::other)?;
            app.manage(desktop);
            let open =
                MenuItem::with_id(app, "open", "Everlast Dictation öffnen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Everlast Dictation")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main")
                            && let Err(error) = window.show().and_then(|()| window.set_focus())
                        {
                            eprintln!("Window open failed: {error}");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    eprintln!("Window hide failed: {error}");
                }
            }
            tauri::WindowEvent::Focused(true) => {
                if let Some(desktop) = window.app_handle().try_state::<Desktop>() {
                    desktop.refresh_permissions();
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())?;
    // Tauri run exits the process directly. Remove native observers explicitly
    // on Exit, before its cleanup, rather than relying on a stack destructor.
    let power_desktop = app.state::<Desktop>().inner().clone();
    let power_app = app.handle().clone();
    let mut power = PowerObserver::new(move |event| {
        if event == PowerEvent::WillPowerOff {
            // ExitRequested alone owns shutdown and the eventual join/retry.
            power_app.exit(0);
        } else if let Err(error) = power_desktop.set_suspended(event == PowerEvent::WillSleep) {
            eprintln!("Power transition failed: {error}");
        }
    })?;
    app.run(move |app, event| {
        if let tauri::RunEvent::Exit = event {
            power.remove();
        }
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && let Some(desktop) = app.try_state::<Desktop>()
            && !desktop.is_finished()
        {
            api.prevent_exit();
            if desktop.begin_shutdown() {
                let desktop = desktop.inner().clone();
                let app = app.clone();
                std::thread::spawn(move || {
                    let code = match desktop.join() {
                        Ok(()) => 0,
                        Err(error) => {
                            eprintln!("Application cleanup failed: {error}");
                            1
                        }
                    };
                    app.exit(code);
                });
            }
        }
    });
    Ok(())
}
fn main() {
    if let Some(result) = macos_integration::clipboard::snapshot_helper_main() {
        if result.is_err() {
            eprintln!("clipboard-backup-failed");
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = run() {
        eprintln!("Application startup failed: {error}");
        std::process::exit(1);
    }
}
