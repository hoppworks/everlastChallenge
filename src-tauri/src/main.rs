#![forbid(unsafe_code)]
use everlast_dictation::{desktop::Desktop, ipc};
use macos_integration::{
    glass_header::GlassHeader,
    keyboard::LayoutObserver,
    power::{PowerEvent, PowerObserver},
};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
fn refresh_hotkey_labels(desktop: &Desktop, app: &tauri::AppHandle) -> Result<(), String> {
    let labels = macos_integration::keyboard::KeyLabels::read_current()
        .map_err(|error| error.to_string())?;
    if let Some(snapshot) = desktop.update_hotkey_labels(labels)? {
        app.emit(everlast_dictation::desktop::SNAPSHOT_EVENT, snapshot)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
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
            ipc::accessibility_request,
            ipc::settings_save,
            ipc::settings_reset,
            ipc::devices_refresh
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
                    if let Err(error) = refresh_hotkey_labels(&desktop, window.app_handle()) {
                        eprintln!("Keyboard label refresh failed: {error}");
                    }
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())?;
    let window = app
        .get_webview_window("main")
        .ok_or("The main dictation window is unavailable")?;
    // Install native controls above Wry's unchanged, readable content root.
    // The header owns this window handle until its explicit exit cleanup.
    let native_desktop = app.state::<Desktop>().inner().clone();
    let actions = native_desktop.clone();
    let mut header = Some(GlassHeader::attach(window.clone(), move |action| {
        actions.native_action(action);
    })?);
    let mut header_revision = None;
    if let Some((revision, state)) = native_desktop.native_header_after(None)? {
        if let Some(header) = &mut header {
            header.update(state)?;
        }
        header_revision = Some(revision);
    }
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
    let label_desktop = native_desktop.clone();
    let label_app = app.handle().clone();
    let mut layout = LayoutObserver::new(move || {
        if let Err(error) = refresh_hotkey_labels(&label_desktop, &label_app) {
            eprintln!("Keyboard label refresh failed: {error}");
        }
    })?;
    // Subscribe before the initial read, closing the startup notification gap.
    refresh_hotkey_labels(&native_desktop, app.handle())?;
    window.show()?;
    app.run(move |app, event| {
        if let tauri::RunEvent::Exit = event {
            layout.remove();
            power.remove();
            if let Some(mut attachment) = header.take()
                && let Err(error) = attachment.detach()
            {
                eprintln!("Native window cleanup failed: {error}");
            }
        }
        if matches!(
            event,
            tauri::RunEvent::Ready | tauri::RunEvent::MainEventsCleared
        ) && let Some(header) = &mut header
        {
            let updated = native_desktop
                .native_header_after(header_revision)
                .and_then(|next| {
                    if let Some((revision, state)) = next {
                        header.update(state).map_err(|error| error.to_string())?;
                        header_revision = Some(revision);
                    }
                    Ok(())
                });
            if let Err(error) = updated {
                eprintln!("Native window update failed: {error}");
                app.exit(1);
            }
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
