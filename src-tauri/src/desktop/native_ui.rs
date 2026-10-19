//! Main-thread native callbacks. Commands reuse Desktop admission and backend ownership.
use super::*;
use macos_integration::{native_preferences::PreferencesAction, native_workspace::WorkspaceAction};

impl Desktop {
    pub fn native_workspace_after(
        &self,
        revision: Option<u64>,
    ) -> Result<Option<(u64, macos_integration::native_workspace::WorkspaceState)>, String> {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok((revision != Some(shared.view.revision)).then(|| {
            (
                shared.view.revision,
                crate::native_presentation::workspace(&shared.view),
            )
        }))
    }
    pub fn native_preferences_after(
        &self,
        revision: Option<u64>,
    ) -> Result<Option<(u64, macos_integration::native_preferences::PreferencesState)>, String>
    {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok((revision != Some(shared.view.revision)).then(|| {
            (
                shared.view.revision,
                crate::native_presentation::preferences(&shared.view),
            )
        }))
    }
    /// Navigation metadata only; never clones the transcript.
    pub fn native_settings_request(&self) -> Result<(u32, bool), String> {
        let shared = self.0.shared.lock().map_err(|_| "Desktop lock failed")?;
        Ok((
            shared.view.settings_request,
            shared.view.preferences.recovery_required,
        ))
    }
    fn native_begin_action(&self) -> Result<(), String> {
        if self.0.closing.load(Ordering::Acquire) {
            return Err("Anwendung wird beendet".into());
        }
        self.0
            .shared
            .lock()
            .map_err(|_| "Desktop lock failed")?
            .action_error = None;
        Ok(())
    }
    /// Called by AppKit on the main thread. Only acknowledgement waits leave it.
    pub fn native_workspace_action(&self, action: WorkspaceAction) {
        let result = self.native_begin_action().and_then(|()| match action {
            WorkspaceAction::Copy { text } => {
                let _permit = self.ui_permit()?;
                ui_services::manual_copy(&text).map_err(|error| match error {
                    ui_services::UiServicesError::InvalidText => "Der Text ist leer oder zu lang zum Kopieren.".into(),
                    ui_services::UiServicesError::Clipboard => "Der Text konnte nicht in die Zwischenablage geschrieben werden. Die Zwischenablage kann jetzt leer sein.".into(),
                    ui_services::UiServicesError::MainThread | ui_services::UiServicesError::Unavailable => "Kopieren ist derzeit nicht verfügbar.".into(),
                })
            }
            WorkspaceAction::Clear { expected_text } => {
                let permit = self.ui_permit()?;
                let reply = self.runtime()?.clear_text_if_unchanged(expected_text).map_err(|error| error.to_string())?;
                self.native_wait(permit, reply, "Textanfrage beendet");
                Ok(())
            }
            WorkspaceAction::Download => self.download(),
            WorkspaceAction::CancelSetup => self.cancel_setup(),
            WorkspaceAction::Settings => self.request_settings().map(|_| ()),
            WorkspaceAction::ResetSettingsConfirmed => {
                let permit = self.ui_permit()?;
                let reply = self.enqueue_reset()?;
                self.native_wait(permit, reply, "Zurücksetzen wurde beendet");
                Ok(())
            }
        });
        if let Err(error) = result {
            self.action_error(error);
        }
    }
    pub fn native_preferences_action(&self, action: PreferencesAction) {
        // A focus notification is read-only and must not dismiss an action error.
        if action == PreferencesAction::RefreshPermissions {
            self.refresh_permissions();
            return;
        }
        let result = self.native_begin_action().and_then(|()| match action {
            PreferencesAction::Save(draft) => {
                let permit = self.ui_permit()?;
                let reply = self.enqueue_settings(settings_draft(draft)?)?;
                self.native_wait(permit, reply, "Einstellungsanfrage beendet");
                Ok(())
            }
            PreferencesAction::RefreshDevices => {
                let permit = self.ui_permit()?;
                let reply = self.enqueue_devices()?;
                self.native_wait(permit, reply, "Mikrofonanfrage beendet");
                Ok(())
            }
            PreferencesAction::RefreshPermissions => {
                self.refresh_permissions();
                Ok(())
            }
            PreferencesAction::RequestAX => {
                let _permit = self.prompt_permit()?;
                ui_services::request_accessibility_access().map_err(|_| {
                    "Die Bedienungshilfen konnten nicht angefragt werden".to_owned()
                })?;
                self.refresh_permissions();
                Ok(())
            }
            PreferencesAction::RequestMic => {
                let permit = self.prompt_permit()?;
                let reply = ui_services::request_microphone_access()
                    .map_err(|_| "Der Mikrofonzugriff konnte nicht angefragt werden".to_owned())?;
                let owner = self.clone();
                // Exactly one outstanding OS permission reply; no GUI-thread receive.
                tauri::async_runtime::spawn_blocking(move || {
                    let _permit = permit;
                    if reply.recv_timeout(Duration::from_secs(300)).is_err() {
                        owner.action_error("Mikrofonfreigabe noch nicht bestätigt".into());
                    }
                    owner.refresh_permissions();
                });
                Ok(())
            }
        });
        if let Err(error) = result {
            self.action_error(error);
        }
    }
    fn native_wait(
        &self,
        permit: tokio::sync::OwnedSemaphorePermit,
        reply: oneshot::Receiver<Result<(), String>>,
        closed: &'static str,
    ) {
        let owner = self.clone();
        tauri::async_runtime::spawn(async move {
            let _permit = permit;
            if let Err(error) = reply.await.unwrap_or_else(|_| Err(closed.into())) {
                owner.action_error(error);
            }
        });
    }
}
fn settings_draft(
    draft: macos_integration::native_preferences::PreferencesDraft,
) -> Result<SettingsDraft, String> {
    let key = ('A'..='Z')
        .zip(crate::hotkey_labels::KEYS)
        .find_map(|(label, key)| (label == draft.key).then_some(key))
        .ok_or("Ungültige physische Taste")?;
    let language = match draft.language.as_str() {
        "german" => crate::settings::SettingsLanguage::German,
        "english" => crate::settings::SettingsLanguage::English,
        _ => return Err("Ungültige Sprache".into()),
    };
    Ok(SettingsDraft {
        device: draft.device,
        language,
        hotkey: crate::settings::Hotkey {
            key,
            option: draft.option,
            command: draft.command,
            control: draft.control,
            shift: draft.shift,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shutdown_rejects_native_permission_actions_before_any_os_call() -> Result<(), String> {
        let desktop = super::super::tests::unstarted_desktop();
        desktop.begin_shutdown();
        desktop.native_preferences_action(PreferencesAction::RequestMic);
        let shared = desktop.0.shared.lock().map_err(|_| "lock failed")?;
        assert_eq!(
            shared.action_error.as_deref(),
            Some("Anwendung wird beendet")
        );
        Ok(())
    }
    #[test]
    fn native_copy_and_prompt_admission_fail_before_native_side_effects() -> Result<(), String> {
        let desktop = super::super::tests::unstarted_desktop();
        let permits = (0..16)
            .map(|_| desktop.ui_permit())
            .collect::<Result<Vec<_>, _>>()?;
        desktop.native_workspace_action(WorkspaceAction::Copy {
            text: "displayed text".into(),
        });
        assert_eq!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock failed")?
                .action_error
                .as_deref(),
            Some("Zu viele laufende Anfragen")
        );
        drop(permits);
        let prompt = desktop.prompt_permit()?;
        desktop.native_preferences_action(PreferencesAction::RequestMic);
        assert_eq!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock failed")?
                .action_error
                .as_deref(),
            Some("Eine Berechtigungsanfrage läuft bereits")
        );
        drop(prompt);
        assert!(desktop.prompt_permit().is_ok());
        Ok(())
    }
    #[test]
    fn projections_follow_published_revision_and_recovery_metadata() -> Result<(), String> {
        let desktop = super::super::tests::unstarted_desktop();
        let (revision, _) = desktop
            .native_workspace_after(None)?
            .ok_or("missing initial view")?;
        assert!(desktop.native_workspace_after(Some(revision))?.is_none());
        assert!(desktop.native_preferences_after(Some(revision))?.is_none());
        {
            let mut shared = desktop.0.shared.lock().map_err(|_| "lock failed")?;
            shared.view.revision += 1;
            shared.view.settings_request = 9;
            shared.view.preferences.recovery_required = true;
        }
        assert_eq!(desktop.native_settings_request()?, (9, true));
        let (_, changed) = desktop
            .native_workspace_after(Some(revision))?
            .ok_or("missing recovery")?;
        assert!(changed.recovery.is_some());
        assert!(!changed.can_download);
        assert!(!changed.can_settings);
        Ok(())
    }
    #[test]
    fn native_draft_preserves_physical_key_modifiers_and_rejects_unknown_values()
    -> Result<(), String> {
        use macos_integration::native_preferences::PreferencesDraft;
        let draft = PreferencesDraft {
            device: Some("USB".into()),
            language: "german".into(),
            key: 'K',
            option: true,
            command: true,
            control: false,
            shift: true,
        };
        let mapped = settings_draft(draft.clone())?;
        assert_eq!(mapped.hotkey.key, crate::settings::PhysicalKey::K);
        assert!(mapped.hotkey.command && mapped.hotkey.shift && mapped.hotkey.option);
        assert_eq!(mapped.device.as_deref(), Some("USB"));
        let mut invalid = draft.clone();
        invalid.key = 'k';
        assert!(settings_draft(invalid).is_err());
        let mut invalid = draft;
        invalid.language = "unknown".into();
        assert!(settings_draft(invalid).is_err());
        Ok(())
    }
    #[test]
    fn readonly_permission_refresh_preserves_the_last_action_error() -> Result<(), String> {
        let desktop = super::super::tests::unstarted_desktop();
        desktop.action_error("Speichern fehlgeschlagen".into());
        desktop.native_preferences_action(PreferencesAction::RefreshPermissions);
        assert!(desktop.0.refresh.load(Ordering::Acquire));
        assert_eq!(
            desktop
                .0
                .shared
                .lock()
                .map_err(|_| "lock failed")?
                .action_error
                .as_deref(),
            Some("Speichern fehlgeschlagen")
        );
        Ok(())
    }
}
