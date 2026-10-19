//! Pure native view projections. Runtime and persisted settings remain authoritative.
use crate::contracts::{SessionPhase, SetupPhase, Snapshot};
use macos_integration::{native_preferences::PreferencesState, native_workspace::WorkspaceState};

pub fn workspace(snapshot: &Snapshot) -> WorkspaceState {
    use macos_integration::native_workspace::{DownloadReadout, RecordingReadout};
    let idle = snapshot.session.phase == SessionPhase::Idle;
    let recovery = snapshot.preferences.recovery_required;
    let has_text = !snapshot.session.text.is_empty();
    WorkspaceState {
        status: crate::header::project(snapshot).status,
        detail: match &snapshot.session.outcome {
            Some(crate::contracts::SessionOutcome::Held { message } | crate::contracts::SessionOutcome::Failed { message }) => native_message(message),
            _ => format!("{} · {}", snapshot.labels.device, snapshot.labels.language),
        },
        error: snapshot.error.as_deref().or(snapshot.setup.error.as_deref()).map(native_message),
        setup_info: (!recovery && matches!(snapshot.setup.phase, SetupPhase::Missing | SetupPhase::Cancelled | SetupPhase::Failed)).then(|| {
            let host = reqwest::Url::parse(&snapshot.setup.model.source_url).ok().and_then(|url| url.host_str().map(str::to_owned));
            let source = match host.as_deref() { Some("huggingface.co") => "Hugging Face", Some(host) => host, None => "unbekannter Quelle" };
            let megabytes = format!("{:.1}", f64::from(snapshot.setup.model.download_bytes) / 1_000_000.0).replace('.', ",");
            native_message(&format!("{} · {megabytes} MB\nEinmaliger Download von {source}; danach lokal und offline.", snapshot.setup.model.name))
        }),
        permission_info: match (snapshot.permissions.microphone == crate::contracts::PermissionStatus::Granted, snapshot.permissions.accessibility == crate::contracts::PermissionStatus::Granted) {
            (true, true) => None,
            (true, false) => Some("Lokales Diktieren und Kopieren sind möglich. Bedienungshilfen werden nur zum automatischen Einfügen benötigt.".into()),
            (false, true) => Some("Aufnehmen erfordert Mikrofonzugriff.".into()),
            (false, false) => Some("Aufnehmen erfordert Mikrofonzugriff. Bedienungshilfen werden nur zum automatischen Einfügen benötigt.".into()),
        },
        transcript: snapshot.session.text.clone(),
        hotkey: snapshot.labels.hotkey.clone(),
        recording: (snapshot.session.phase == SessionPhase::Recording).then(|| RecordingReadout {
            elapsed_seconds: (snapshot.session.elapsed_millis / 1_000).min(u64::from(u32::MAX)) as u32,
            level_permille: snapshot.session.level_permille.min(1_000),
        }),
        download: (snapshot.setup.phase == SetupPhase::Downloading).then(|| DownloadReadout {
            received: u64::from(snapshot.setup.downloaded_bytes),
            total: (snapshot.setup.total_bytes != 0).then_some(u64::from(snapshot.setup.total_bytes)),
        }),
        recovery: recovery.then(|| snapshot.preferences.error.as_deref().map(native_message).unwrap_or_else(|| "Die Einstellungen konnten nicht gelesen werden. Erst nach deiner Bestätigung werden Standardwerte gespeichert.".into())),
        can_copy: snapshot.running && has_text && snapshot.session.text.len() <= 256 * 1_024,
        can_clear: snapshot.running && idle && snapshot.setup.phase == SetupPhase::Ready && has_text,
        can_download: snapshot.running && idle && !recovery && matches!(snapshot.setup.phase, SetupPhase::Missing | SetupPhase::Cancelled | SetupPhase::Failed),
        can_cancel_setup: snapshot.running && idle && !recovery && snapshot.setup.can_cancel,
        can_settings: snapshot.running && !recovery && (snapshot.permissions.microphone != crate::contracts::PermissionStatus::Granted || snapshot.permissions.accessibility != crate::contracts::PermissionStatus::Granted),
    }
}

pub fn preferences(snapshot: &Snapshot) -> PreferencesState {
    use macos_integration::native_preferences::{PreferenceChoice, PreferencesDraft};
    let current = &snapshot.preferences.current;
    let megabytes = format!(
        "{:.1}",
        f64::from(snapshot.setup.model.download_bytes) / 1_000_000.0
    )
    .replace('.', ",");
    let mut devices = Vec::new();
    if let Some(name) = &current.device
        && !snapshot
            .preferences
            .devices
            .iter()
            .any(|device| &device.name == name)
    {
        devices.push(PreferenceChoice {
            value: name.clone(),
            label: format!("{name} (nicht verfügbar)"),
            enabled: false,
        });
    }
    for device in &snapshot.preferences.devices {
        if devices.iter().any(|choice| choice.value == device.name) {
            continue;
        }
        let available = device.available
            && snapshot
                .preferences
                .devices
                .iter()
                .filter(|other| other.name == device.name)
                .count()
                == 1;
        devices.push(PreferenceChoice {
            value: device.name.clone(),
            label: if available {
                device.name.clone()
            } else {
                format!("{} (nicht eindeutig)", device.name)
            },
            enabled: available,
        });
    }
    let mut languages: Vec<_> = snapshot
        .preferences
        .languages
        .iter()
        .map(|language| PreferenceChoice {
            value: language_id(*language).into(),
            label: language_label(*language).into(),
            enabled: true,
        })
        .collect();
    if !snapshot.preferences.languages.contains(&current.language) {
        languages.push(PreferenceChoice {
            value: language_id(current.language).into(),
            label: format!(
                "{} (noch nicht freigegeben)",
                language_label(current.language)
            ),
            enabled: false,
        });
    }
    PreferencesState {
        current: PreferencesDraft { device: current.device.clone(), language: language_id(current.language).into(), key: physical_char(current.hotkey.key), option: current.hotkey.option, command: current.hotkey.command, control: current.hotkey.control, shift: current.hotkey.shift },
        devices, languages,
        key_labels: snapshot.preferences.key_labels.iter().map(|key| (physical_char(key.key), if key.plain == key.command { key.plain.clone() } else { format!("{} · Command: {}", key.plain, key.command) })).collect(),
        busy: !snapshot.running || snapshot.settings_busy || snapshot.preferences.needs_restart || snapshot.preferences.recovery_required,
        // Prompts are asynchronous OS requests, not recording/settings state.
        // The shared prompt semaphore admits them; no pending state is invented here.
        permissions_busy: !snapshot.running || snapshot.preferences.needs_restart || snapshot.preferences.recovery_required || snapshot.session.phase != SessionPhase::Idle,
        microphone: permission(&snapshot.permissions.microphone), accessibility: permission(&snapshot.permissions.accessibility),
        model_summary: format!("{} · {}", snapshot.setup.model.name, match snapshot.setup.phase {
            SetupPhase::Missing => "Noch nicht eingerichtet", SetupPhase::Checking => "Wird geprüft", SetupPhase::Downloading => "Wird geladen", SetupPhase::Loading => "Wird gestartet", SetupPhase::Cancelling => "Einrichtung wird abgebrochen", SetupPhase::Cancelled => "Einrichtung abgebrochen", SetupPhase::Ready => "Für die lokale Erkennung bereit", SetupPhase::Failed => "Einrichtung fehlgeschlagen",
        }),
        model_details: format!("{megabytes} MB\nQuelle\n{}", snapshot.setup.model.source_url),
        error: snapshot.error.as_deref().or(snapshot.preferences.error.as_deref()).map(native_message).or_else(|| snapshot.preferences.needs_restart.then(|| "Die Einstellungen konnten nicht vollständig angewendet werden. Bitte starte die App neu.".into())),
    }
}
// AppKit labels are bounded independently from the retained transcript.
fn native_message(value: &str) -> String {
    let mut result = value.replace('\0', "�");
    if result.len() > 2_048 {
        let mut end = 2_045;
        while !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
        result.push('…');
    }
    result
}
fn language_id(language: crate::settings::SettingsLanguage) -> &'static str {
    match language {
        crate::settings::SettingsLanguage::German => "german",
        crate::settings::SettingsLanguage::English => "english",
    }
}
fn language_label(language: crate::settings::SettingsLanguage) -> &'static str {
    match language {
        crate::settings::SettingsLanguage::German => "Deutsch",
        crate::settings::SettingsLanguage::English => "Englisch",
    }
}
fn permission(
    value: &crate::contracts::PermissionStatus,
) -> macos_integration::native_preferences::PreferencesPermission {
    use crate::contracts::PermissionStatus as From;
    use macos_integration::native_preferences::PreferencesPermission as To;
    match value {
        From::Granted => To::Granted,
        From::Denied => To::Denied,
        From::NotDetermined => To::NotDetermined,
        From::Restricted => To::Restricted,
        From::Unavailable => To::Unavailable,
    }
}
fn physical_char(key: crate::settings::PhysicalKey) -> char {
    use crate::settings::PhysicalKey;
    match key {
        PhysicalKey::A => 'A',
        PhysicalKey::B => 'B',
        PhysicalKey::C => 'C',
        PhysicalKey::D => 'D',
        PhysicalKey::E => 'E',
        PhysicalKey::F => 'F',
        PhysicalKey::G => 'G',
        PhysicalKey::H => 'H',
        PhysicalKey::I => 'I',
        PhysicalKey::J => 'J',
        PhysicalKey::K => 'K',
        PhysicalKey::L => 'L',
        PhysicalKey::M => 'M',
        PhysicalKey::N => 'N',
        PhysicalKey::O => 'O',
        PhysicalKey::P => 'P',
        PhysicalKey::Q => 'Q',
        PhysicalKey::R => 'R',
        PhysicalKey::S => 'S',
        PhysicalKey::T => 'T',
        PhysicalKey::U => 'U',
        PhysicalKey::V => 'V',
        PhysicalKey::W => 'W',
        PhysicalKey::X => 'X',
        PhysicalKey::Y => 'Y',
        PhysicalKey::Z => 'Z',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{contracts as view, preferences::SettingsDraft, settings::Settings};
    fn snapshot() -> Snapshot {
        let settings = Settings::default();
        Snapshot {
            revision: 1,
            running: true,
            setup: view::Setup {
                phase: SetupPhase::Ready,
                model: view::Model {
                    name: "Reference".into(),
                    download_bytes: 100,
                    source_url: "https://example.com/model".into(),
                },
                downloaded_bytes: 100,
                total_bytes: 100,
                can_cancel: false,
                error: None,
            },
            session: view::Session {
                phase: SessionPhase::Idle,
                id: None,
                text: "Aufbewahrter Text".into(),
                outcome: None,
                maximum_warning: false,
                maximum_reached: false,
                last_held_session: None,
                elapsed_millis: 0,
                level_permille: 0,
            },
            permissions: view::Permissions {
                microphone: view::PermissionStatus::Granted,
                accessibility: view::PermissionStatus::Denied,
            },
            labels: view::Labels {
                device: "Systemstandard".into(),
                language: "Deutsch".into(),
                hotkey: "⌥D".into(),
            },
            settings_busy: false,
            settings_request: 0,
            preferences: view::Preferences {
                key_labels: Vec::new(),
                current: SettingsDraft {
                    device: None,
                    language: settings.language,
                    hotkey: settings.hotkey,
                },
                devices: Vec::new(),
                error: None,
                refreshing: false,
                needs_restart: false,
                recovery_required: false,
                languages: vec![crate::settings::SettingsLanguage::German],
            },
            error: None,
        }
    }
    #[test]
    fn workspace_retains_text_while_recording_and_exposes_real_telemetry() {
        let mut snapshot = snapshot();
        snapshot.session.phase = SessionPhase::Recording;
        snapshot.session.id = Some(7);
        snapshot.session.elapsed_millis = 65_950;
        snapshot.session.level_permille = 250;
        let view = workspace(&snapshot);
        assert_eq!(view.transcript, "Aufbewahrter Text");
        assert_eq!(view.status, "Aufnahme läuft");
        let telemetry = view.recording.as_ref();
        assert_eq!(telemetry.map(|v| v.elapsed_seconds), Some(65));
        assert_eq!(telemetry.map(|v| v.level_permille), Some(250));
        assert!(!view.can_clear);
        assert!(view.can_copy);
        assert!(!view.can_download);
    }
    #[test]
    fn preferences_distinguishes_ambiguous_missing_and_actual_saved_values() {
        let mut snapshot = snapshot();
        snapshot.preferences.current.device = Some("Fehlendes Mikrofon".into());
        snapshot.preferences.current.hotkey.key = crate::settings::PhysicalKey::K;
        snapshot.preferences.current.hotkey.command = true;
        snapshot.preferences.devices = vec![crate::devices::InputDevice {
            name: "USB".into(),
            available: false,
        }];
        snapshot.preferences.needs_restart = true;
        let view = preferences(&snapshot);
        assert_eq!(view.current.key, 'K');
        assert!(view.current.command);
        assert!(view.busy);
        assert!(
            view.devices
                .iter()
                .any(|d| d.label == "USB (nicht eindeutig)" && !d.enabled)
        );
        assert!(
            view.devices
                .iter()
                .any(|d| d.label == "Fehlendes Mikrofon (nicht verfügbar)" && !d.enabled)
        );
        assert_eq!(view.languages.len(), 1);
        assert_eq!(view.languages[0].label, "Deutsch");
        assert!(view.languages[0].enabled);
        assert_eq!(
            view.microphone,
            macos_integration::native_preferences::PreferencesPermission::Granted
        );
        assert_eq!(
            view.accessibility,
            macos_integration::native_preferences::PreferencesPermission::Denied
        );
    }
    #[test]
    fn recovery_blocks_model_setup_and_unknown_download_length_stays_indeterminate() {
        let mut snapshot = snapshot();
        snapshot.setup.phase = SetupPhase::Failed;
        snapshot.preferences.recovery_required = true;
        snapshot.preferences.error = Some("Ungültige Einstellungsdatei".into());
        let view = workspace(&snapshot);
        assert_eq!(
            view.recovery.as_deref(),
            Some("Ungültige Einstellungsdatei")
        );
        assert!(!view.can_download && !view.can_settings);
        snapshot.preferences.recovery_required = false;
        snapshot.setup.phase = SetupPhase::Downloading;
        snapshot.setup.total_bytes = 0;
        snapshot.setup.downloaded_bytes = 12;
        let view = workspace(&snapshot);
        assert_eq!(view.download.as_ref().map(|v| v.received), Some(12));
        assert_eq!(view.download.and_then(|v| v.total), None);
        assert!(view.recording.is_none());
    }
    #[test]
    fn oversized_error_labels_do_not_block_display_of_retained_text() {
        let mut snapshot = snapshot();
        snapshot.error = Some("界".repeat(1_000));
        let view = workspace(&snapshot);
        assert!(
            view.error
                .as_ref()
                .is_some_and(|message| message.len() <= 2_048)
        );
        assert_eq!(view.transcript, "Aufbewahrter Text");
        assert!(
            preferences(&snapshot)
                .error
                .as_ref()
                .is_some_and(|message| message.len() <= 2_048)
        );
    }

    #[test]
    fn active_session_blocks_prompts_without_claiming_settings_transaction() {
        let mut snapshot = snapshot();
        snapshot.session.phase = SessionPhase::Recording;
        snapshot.settings_busy = false;
        assert!(preferences(&snapshot).permissions_busy);
        snapshot.session.phase = SessionPhase::Idle;
        snapshot.setup.phase = SetupPhase::Loading;
        assert!(!preferences(&snapshot).permissions_busy);
        snapshot.permissions.accessibility = crate::contracts::PermissionStatus::Granted;
        assert!(!workspace(&snapshot).can_settings);
    }
    #[test]
    fn setup_disclosure_uses_the_actual_model_size_source_and_offline_contract() {
        let mut snapshot = snapshot();
        snapshot.setup.model.name = "large-v3-turbo-q5_0".into();
        snapshot.setup.model.download_bytes = 574_041_195;
        snapshot.setup.model.source_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".into();
        for phase in [
            SetupPhase::Missing,
            SetupPhase::Cancelled,
            SetupPhase::Failed,
        ] {
            snapshot.setup.phase = phase;
            let view = workspace(&snapshot);
            let info = view.setup_info.as_deref();
            assert!(info.is_some_and(|text| text.contains("large-v3-turbo-q5_0 · 574,0 MB")));
            assert!(info.is_some_and(|text| text.contains("Download von Hugging Face")));
            assert!(info.is_some_and(|text| {
                text.contains("Einmaliger Download von Hugging Face; danach lokal und offline.")
            }));
        }
        snapshot.setup.phase = SetupPhase::Ready;
        assert!(workspace(&snapshot).setup_info.is_none());
    }
    #[test]
    fn permission_explanation_separates_recording_from_automatic_insertion() {
        use crate::contracts::PermissionStatus;
        let mut snapshot = snapshot();
        snapshot.permissions.microphone = PermissionStatus::Granted;
        snapshot.permissions.accessibility = PermissionStatus::Denied;
        let view = workspace(&snapshot);
        assert!(view.permission_info.as_ref().is_some_and(|text| {
            text.contains("Lokales Diktieren und Kopieren sind möglich")
                && text.contains("nur zum automatischen Einfügen")
        }));
        snapshot.permissions.microphone = PermissionStatus::Unavailable;
        let view = workspace(&snapshot);
        assert!(
            view.permission_info
                .as_ref()
                .is_some_and(|text| text.contains("Aufnehmen erfordert Mikrofonzugriff")
                    && text.contains("nur zum automatischen Einfügen"))
        );
        snapshot.permissions.microphone = PermissionStatus::Granted;
        snapshot.permissions.accessibility = PermissionStatus::Granted;
        assert!(workspace(&snapshot).permission_info.is_none());
    }
    #[test]
    fn native_preferences_owns_position_suffix_and_model_details_stay_readable() {
        let mut snapshot = snapshot();
        snapshot.preferences.key_labels = vec![
            crate::contracts::KeyLabel {
                key: crate::settings::PhysicalKey::D,
                plain: "D".into(),
                command: "D".into(),
            },
            crate::contracts::KeyLabel {
                key: crate::settings::PhysicalKey::K,
                plain: "E".into(),
                command: "D".into(),
            },
        ];
        snapshot.setup.model.download_bytes = 574_041_195;
        snapshot.setup.model.source_url = "https://huggingface.co/example/model".into();
        let view = preferences(&snapshot);
        assert_eq!(
            view.key_labels,
            vec![('D', "D".into()), ('K', "E · Command: D".into())]
        );
        assert_eq!(
            view.model_details,
            "574,0 MB\nQuelle\nhttps://huggingface.co/example/model"
        );
    }
}
