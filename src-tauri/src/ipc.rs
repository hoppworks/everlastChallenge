#![allow(
    clippy::let_underscore_must_use,
    reason = "Tauri 2.12 async command wrappers generate discarded internal Results; handwritten handlers return and check their Results."
)]
//! The only renderer command surface. Capabilities and the window label both restrict access.
use crate::{contracts::Snapshot, desktop::Desktop, preferences::SettingsDraft};
use macos_integration::ui_services;
use tauri::{AppHandle, State, WebviewWindow};

fn authorize(window: &WebviewWindow) -> Result<(), String> {
    allowed_label(window.label())
}
fn allowed_label(label: &str) -> Result<(), String> {
    if label == "main" {
        Ok(())
    } else {
        Err("Dieses Fenster darf keine Diktierbefehle ausführen".into())
    }
}
async fn main_call<T: Send + 'static>(
    app: AppHandle,
    admission: std::sync::Arc<tokio::sync::OwnedSemaphorePermit>,
    action: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };
    let (send, receive) = tokio::sync::oneshot::channel();
    let gate = Arc::new(AtomicU8::new(0));
    let queued = Arc::clone(&gate);
    app.run_on_main_thread(move || {
        // A timed-out queued request keeps its permit until this closure drops.
        let _admission = admission;
        if queued
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            && send.send(action()).is_err()
        { /* caller no longer waiting */ }
    })
    .map_err(|error| error.to_string())?;
    match tokio::time::timeout(std::time::Duration::from_secs(2), receive).await {
        Ok(result) => result.map_err(|_| "Hauptthread-Anfrage wurde beendet".to_owned())?,
        Err(_) => {
            let message = if gate
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                "Hauptthread-Anfrage hat das Zeitlimit überschritten und wurde verworfen"
            } else {
                "Hauptthread-Anfrage wird noch abgeschlossen; Ergebnis ist unbekannt"
            };
            Err(message.into())
        }
    }
}
#[tauri::command]
pub fn app_snapshot(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
) -> Result<Snapshot, String> {
    authorize(&window)?;
    desktop.snapshot()
}
#[tauri::command]
pub fn model_download(window: WebviewWindow, desktop: State<'_, Desktop>) -> Result<(), String> {
    authorize(&window)?;
    desktop.download()
}
#[tauri::command]
pub fn model_cancel(window: WebviewWindow, desktop: State<'_, Desktop>) -> Result<(), String> {
    authorize(&window)?;
    desktop.cancel_setup()
}
#[tauri::command]
pub async fn recording_start(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.start_recording().await
}
#[tauri::command]
pub fn recording_stop(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
    session: u64,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.stop_recording(session)
}
#[tauri::command]
pub fn recording_cancel(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
    session: u64,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.cancel_recording(session)
}
#[tauri::command]
pub async fn transcript_clear(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.clear_text().await
}
#[tauri::command]
pub async fn transcript_copy(
    window: WebviewWindow,
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    let admission = std::sync::Arc::new(desktop.ui_permit()?);
    let text = desktop.text()?;
    main_call(app, admission, move || {
        ui_services::manual_copy(&text).map_err(|error| error.to_string())
    })
    .await
}
#[tauri::command]
pub async fn microphone_request(
    window: WebviewWindow,
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    let permit = std::sync::Arc::new(desktop.prompt_permit()?);
    let reply = main_call(app, std::sync::Arc::clone(&permit), || {
        ui_services::request_microphone_access().map_err(|error| error.to_string())
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        reply
            .recv_timeout(std::time::Duration::from_secs(300))
            .map_err(|_| "Mikrofonfreigabe noch nicht bestätigt".to_owned())
    })
    .await
    .map_err(|error| error.to_string())??;
    desktop.refresh_permissions();
    Ok(())
}
#[tauri::command]
pub async fn accessibility_request(
    window: WebviewWindow,
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    let permit = std::sync::Arc::new(desktop.prompt_permit()?);
    main_call(app, std::sync::Arc::clone(&permit), || {
        ui_services::request_accessibility_access().map_err(|error| error.to_string())
    })
    .await?;
    desktop.refresh_permissions();
    Ok(())
}

#[tauri::command]
pub async fn settings_save(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
    settings: SettingsDraft,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.save_settings(settings).await
}
#[tauri::command]
pub async fn devices_refresh(
    window: WebviewWindow,
    desktop: State<'_, Desktop>,
) -> Result<(), String> {
    authorize(&window)?;
    desktop.refresh_devices().await
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_main_window_can_invoke_product_commands() {
        assert!(super::allowed_label("main").is_ok());
        for label in ["", "other", "main-2", "MAIN", "main/../other"] {
            assert!(super::allowed_label(label).is_err());
        }
    }
    #[test]
    fn compiled_tauri_acl_denies_foreign_windows_and_remote_origins()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut context: tauri::Context<tauri::Wry> = tauri::generate_context!();
        let authority = context.runtime_authority_mut();
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com/".parse()?,
        };
        for command in [
            "app_snapshot",
            "model_download",
            "model_cancel",
            "recording_start",
            "recording_stop",
            "recording_cancel",
            "transcript_copy",
            "transcript_clear",
            "microphone_request",
            "accessibility_request",
            "settings_save",
            "devices_refresh",
        ] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &tauri::ipc::Origin::Local)
                    .is_some(),
                "{command} unavailable to main"
            );
            assert!(
                authority
                    .resolve_access(command, "foreign", "foreign", &tauri::ipc::Origin::Local)
                    .is_none(),
                "{command} exposed to foreign window"
            );
            assert!(
                authority
                    .resolve_access(command, "main", "main", &remote)
                    .is_none(),
                "{command} exposed to remote origin"
            );
        }
        assert!(
            authority
                .resolve_access(
                    "unknown_command",
                    "main",
                    "main",
                    &tauri::ipc::Origin::Local
                )
                .is_none()
        );
        Ok(())
    }
}
