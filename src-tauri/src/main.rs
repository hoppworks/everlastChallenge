#![forbid(unsafe_code)]
use everlast_dictation::{
    desktop::Desktop,
    feedback::{Feedback, FeedbackState, TrayPresentation},
    ipc,
};
use macos_integration::{
    keyboard::LayoutObserver,
    power::{PowerEvent, PowerObserver},
    termination::TerminationHook,
};
use tauri::{
    Emitter, Listener, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};
#[cfg(feature = "measurement")]
mod native_start_export;
mod run_observer;
#[cfg(feature = "test-control")]
mod test_control;

#[derive(Clone)]
struct NativeTray {
    tray: tauri::tray::TrayIcon<tauri::Wry>,
    status: MenuItem<tauri::Wry>,
}
impl NativeTray {
    fn update(&self, state: &TrayPresentation) -> Result<(), String> {
        self.status
            .set_text(state.text.replace('&', "&&"))
            .map_err(|error| error.to_string())?;
        self.tray
            .set_tooltip(Some(format!("Everlast Coding Challenge · {}", state.text)))
            .map_err(|error| error.to_string())?;
        self.tray
            .set_title(Some(state.title))
            .map_err(|error| error.to_string())
    }
}
fn refresh_hotkey_labels(desktop: &Desktop, app: &tauri::AppHandle) -> Result<(), String> {
    let labels = macos_integration::keyboard::KeyLabels::read_current()
        .map_err(|error| error.to_string())?;
    if let Some(snapshot) = desktop.update_hotkey_labels(labels)? {
        app.emit(everlast_dictation::desktop::SNAPSHOT_EVENT, snapshot)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
// Native views must wake for backend updates without a renderer subscription.
// The listener only queues a wake; it never reads Desktop while emit holds its lock.
struct NativeWakeup {
    app: tauri::AppHandle,
    event: tauri::EventId,
}
impl NativeWakeup {
    fn new(app: &tauri::AppHandle) -> Self {
        let wake = app.clone();
        let event = app.listen(everlast_dictation::desktop::SNAPSHOT_EVENT, move |_| {
            if let Err(error) = wake.run_on_main_thread(|| {}) {
                eprintln!("Native UI wake failed: {error}");
            }
        });
        Self {
            app: app.clone(),
            event,
        }
    }
}
impl Drop for NativeWakeup {
    fn drop(&mut self) {
        self.app.unlisten(self.event);
    }
}

#[cfg(feature = "test-control")]
impl test_control::Handler for Desktop {
    fn inspect(&self, recipient_pid: i32) -> Result<serde_json::Value, String> {
        self.test_control_inspect(recipient_pid)
    }
    fn status(&self) -> Result<serde_json::Value, String> {
        self.test_control_status()
    }
    fn start(&self, recipient_pid: i32) -> Result<(), String> {
        self.test_control_start(recipient_pid)
    }
    #[cfg(feature = "measurement")]
    fn start_host(&self, recipient_pid: i32) -> Result<(), String> {
        self.test_control_start_host(recipient_pid)
    }
    #[cfg(feature = "measurement")]
    fn host_observation(&self) -> Result<serde_json::Value, String> {
        self.test_control_host_observation()
    }
    #[cfg(feature = "measurement")]
    fn handoff_observation(&self) -> Result<serde_json::Value, String> {
        self.test_control_handoff_observation()
    }
    fn stop(&self, session: u64) -> Result<(), String> {
        self.test_control_stop(session)
    }
    fn cancel(&self, session: u64) -> Result<(), String> {
        self.test_control_cancel(session)
    }
    fn shutdown(&self) -> Result<(), String> {
        self.test_control_shutdown()
    }
}

struct NativeUi {
    window: tauri::WebviewWindow,
    settings_request: u32,
    wakeup: Option<NativeWakeup>,
    run_observer: Option<run_observer::Observer>,
    #[cfg(feature = "test-control")]
    test_control: Option<test_control::Control>,
    desktop: Desktop,
    tray: NativeTray,
    feedback: Feedback,
    feedback_revision: Option<u64>,
    feedback_state: Option<FeedbackState>,
    power: PowerObserver,
    layout: LayoutObserver,
}
impl NativeUi {
    fn attach(app: &tauri::AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let window = app
            .get_webview_window("main")
            .ok_or("The main dictation window is unavailable")?;
        // React owns the product surface; native services remain independent.
        let native_desktop = app.state::<Desktop>().inner().clone();
        let native_tray = app.state::<NativeTray>().inner().clone();
        let feedback = Feedback::default();
        let feedback_revision = None;
        let feedback_state: Option<FeedbackState> = None;
        // Tauri run exits the process directly. Remove native observers explicitly
        // on Exit, before its cleanup, rather than relying on a stack destructor.
        let power_desktop = app.state::<Desktop>().inner().clone();
        let power_app = app.clone();
        let power = PowerObserver::new(move |event| {
            if event == PowerEvent::WillPowerOff {
                // ExitRequested alone owns shutdown and the eventual join/retry.
                power_app.exit(0);
            } else if let Err(error) = power_desktop.set_suspended(event == PowerEvent::WillSleep) {
                eprintln!("Power transition failed: {error}");
            }
        })?;
        let label_desktop = native_desktop.clone();
        let label_app = app.clone();
        let layout = LayoutObserver::new(move || {
            if let Err(error) = refresh_hotkey_labels(&label_desktop, &label_app) {
                eprintln!("Keyboard label refresh failed: {error}");
            }
        })?;
        // Subscribe before the initial read, closing the startup notification gap.
        refresh_hotkey_labels(&native_desktop, app)?;
        let wakeup = NativeWakeup::new(app);
        #[cfg(feature = "test-control")]
        let test_control = {
            let wake = app.clone();
            test_control::Control::from_env(std::sync::Arc::new(move || {
                if let Err(error) = wake.run_on_main_thread(|| {}) {
                    eprintln!("Test control wake failed: {error}");
                }
            }))?
        };
        window.show()?;
        let run_observer =
            match app.path().app_data_dir().ok().and_then(|root| {
                run_observer::Observer::from_env(native_desktop.clone(), root).ok()
            }) {
                Some(observer) => Some(observer),
                None => {
                    eprintln!("Read-only run observer unavailable");
                    None
                }
            };
        Ok(Self {
            window,
            settings_request: 0,
            wakeup: Some(wakeup),
            run_observer,
            #[cfg(feature = "test-control")]
            test_control,
            desktop: native_desktop,
            tray: native_tray,
            feedback,
            feedback_revision,
            feedback_state,
            power,
            layout,
        })
    }
    fn update(&mut self) -> Result<(), String> {
        #[cfg(feature = "test-control")]
        if let Some(control) = &mut self.test_control {
            control.poll(&self.desktop);
        }
        let request = self.desktop.settings_request()?;
        if request != self.settings_request {
            self.settings_request = request;
            self.window
                .show()
                .and_then(|()| self.window.set_focus())
                .map_err(|error| error.to_string())?;
        }
        if let Some((revision, state)) =
            self.desktop.native_feedback_after(self.feedback_revision)?
        {
            self.feedback_revision = Some(revision);
            self.feedback_state = Some(state);
        }
        if let Some(state) = &self.feedback_state {
            self.feedback.update(
                state,
                |presentation| self.tray.update(presentation),
                |session| self.desktop.held_feedback_tone(session),
            )?;
        }
        Ok(())
    }
    fn detach(&mut self) {
        // Tauri exits the process directly; clean up native owners on Exit.
        if let Some(mut observer) = self.run_observer.take()
            && observer.close().is_err()
        {
            eprintln!("Read-only run observer failed");
        }
        #[cfg(feature = "test-control")]
        if let Some(mut control) = self.test_control.take() {
            control.close(&self.desktop);
        }
        drop(self.wakeup.take());
        self.layout.remove();
        self.power.remove();
    }
}

fn request_native_settings(app: &tauri::AppHandle) {
    if let Some(desktop) = app.try_state::<Desktop>()
        && let Err(error) = desktop.request_settings()
    {
        eprintln!("Native preferences request failed: {error}");
    }
}

fn toggle_window_recording(app: &tauri::AppHandle) {
    use macos_integration::glass_header::{HeaderAction, HeaderPrimary};
    if let Some(desktop) = app.try_state::<Desktop>() {
        match desktop.native_header_after(None) {
            Ok(Some((_, state))) => match state.primary {
                Some(HeaderPrimary::Start) => desktop.native_action(HeaderAction::Start),
                Some(HeaderPrimary::Stop(session)) => {
                    desktop.native_action(HeaderAction::Stop(session))
                }
                None => {}
            },
            Ok(None) => {}
            Err(error) => eprintln!("Window recording shortcut failed: {error}"),
        }
    }
}

fn cancel_window_recording(app: &tauri::AppHandle) {
    use macos_integration::glass_header::HeaderAction;
    if let Some(desktop) = app.try_state::<Desktop>() {
        match desktop.native_header_after(None) {
            Ok(Some((_, state))) => {
                if let Some(session) = state.cancel {
                    desktop.native_action(HeaderAction::Cancel(session));
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("Window cancellation shortcut failed: {error}"),
        }
    }
}

fn application_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::default(app)?;
    // The pinned Tauri macOS default puts the application submenu first.
    // Its predefined Quit invokes NSApplication.terminate directly, bypassing
    // ExitRequested and the inference-worker join. Own that action instead.
    menu.remove_at(0)?;
    let quit = MenuItem::with_id(
        app,
        "application-quit",
        "Everlast Coding Challenge beenden",
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    let settings = MenuItem::with_id(
        app,
        "application-settings",
        "Einstellungen …",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let application = Submenu::with_items(
        app,
        "Everlast Coding Challenge",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("Über Everlast Coding Challenge"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    menu.insert(&application, 0)?;
    let recording = MenuItem::with_id(
        app,
        "window-recording",
        "Aufnahme starten / stoppen",
        true,
        Some("CmdOrCtrl+R"),
    )?;
    let cancel = MenuItem::with_id(
        app,
        "window-cancel",
        "Aufnahme verwerfen",
        true,
        Some("Escape"),
    )?;
    let dictation = Submenu::with_items(app, "Diktat", true, &[&recording, &cancel])?;
    menu.append(&dictation)?;
    Ok(menu)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "measurement")]
    let native_start_target = match native_start_export::Target::from_option(std::env::var_os(
        native_start_export::ENVIRONMENT,
    )) {
        Ok(target) => target,
        Err(_) => {
            eprintln!("Native start observation opt-in rejected; recording behavior unchanged");
            None
        }
    };
    #[cfg(feature = "measurement")]
    let native_start_enabled = native_start_target.is_some();
    let app = tauri::Builder::default()
        .menu(application_menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "application-quit" => app.exit(0),
            "application-settings" => request_native_settings(app),
            "window-recording" => toggle_window_recording(app),
            "window-cancel" => cancel_window_recording(app),
            _ => {}
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::app_snapshot,
            ipc::model_download,
            ipc::model_cancel,
            ipc::refinement_download,
            ipc::refinement_cancel_setup,
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
        .setup(move |app| {
            #[cfg(feature = "measurement")]
            let desktop = if native_start_enabled {
                Desktop::spawn_observing_native_start(app.handle().clone())
            } else {
                Desktop::spawn(app.handle().clone())
            }
            .map_err(std::io::Error::other)?;
            #[cfg(not(feature = "measurement"))]
            let desktop = Desktop::spawn(app.handle().clone()).map_err(std::io::Error::other)?;
            app.manage(desktop);
            let open = MenuItem::with_id(
                app,
                "open",
                "Everlast Coding Challenge öffnen",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
            let status =
                MenuItem::with_id(app, "status", "Status wird geladen", false, None::<&str>)?;
            let settings =
                MenuItem::with_id(app, "settings", "Einstellungen …", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status, &open, &settings, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Everlast Coding Challenge")
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
                    "settings" => request_native_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let tray = tray.build(app)?;
            app.manage(NativeTray { tray, status });
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
    // The delegate already exists here, before setup or model initialization.
    // Redirect macOS/AppleEvent Quit through the same asynchronous shutdown
    // handshake as our menu, rather than allowing native process teardown.
    let exit_app = app.handle().clone();
    let mut termination = Some(TerminationHook::install(move || exit_app.exit(0))?);
    let mut native_ui: Option<NativeUi> = None;
    app.run(move |app, event| {
        if matches!(event, tauri::RunEvent::Ready) {
            // Tauri creates configured windows and runs setup before Ready.
            // A built App alone does not yet own its main window or Desktop.
            match NativeUi::attach(app) {
                Ok(ui) => native_ui = Some(ui),
                Err(error) => {
                    eprintln!("Native UI startup failed: {error}");
                    app.exit(1);
                }
            }
        }
        if matches!(
            event,
            tauri::RunEvent::MainEventsCleared | tauri::RunEvent::Ready
        ) && let Some(ui) = &mut native_ui
            && let Err(error) = ui.update()
        {
            eprintln!("Native UI update failed: {error}");
            app.exit(1);
        }
        if matches!(event, tauri::RunEvent::Exit)
            && let Some(mut ui) = native_ui.take()
        {
            ui.detach();
        }
        if matches!(event, tauri::RunEvent::Exit) {
            drop(termination.take());
        }
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event
            && let Some(desktop) = app.try_state::<Desktop>()
            && !desktop.is_finished()
        {
            api.prevent_exit();
            if desktop.begin_shutdown() {
                let requested_code = code.unwrap_or(0);
                let desktop = desktop.inner().clone();
                let app = app.clone();
                #[cfg(feature = "measurement")]
                let export_target = native_start_target.clone();
                std::thread::spawn(move || {
                    let joined = desktop.join();
                    let code = match &joined {
                        Ok(()) => requested_code,
                        Err(error) => {
                            eprintln!("Application cleanup failed: {error}");
                            1
                        }
                    };
                    #[cfg(feature = "measurement")]
                    if native_start_export::export_if_selected(export_target.as_ref(), || {
                        serde_json::to_vec(&serde_json::json!({
                            "schemaVersion":1,"kind":"native-start-upper-bound-export-v1",
                            "processId":std::process::id(),"controlledShutdownJoined":joined.is_ok(),
                            "qualification":false,"exactCpalLatency":false,"L":null,
                            "observations":desktop.native_start_observations(),
                        }))
                        .map_err(|_| "observation-serialization-failed")
                    })
                    .is_err()
                    {
                        eprintln!(
                            "Native start observation export failed; cleanup exit status unchanged"
                        );
                    }
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
