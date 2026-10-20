//! Public renderer-facing settings requests retain ownership through their ACK.
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn draft() -> SettingsDraft {
    SettingsDraft {
        model: None,
        device: None,
        language: crate::settings::SettingsLanguage::German,
        hotkey: crate::settings::Hotkey {
            key: crate::settings::PhysicalKey::K,
            option: true,
            command: false,
            control: false,
            shift: false,
        },
    }
}

fn reserved(desktop: &Desktop) -> Result<bool, String> {
    Ok(desktop
        .0
        .admission
        .lock()
        .map_err(|_| "admission lock")?
        .changing_settings)
}

#[test]
fn renderer_save_retains_owner_reservation_and_returns_failed_or_closed_ack() -> TestResult {
    let SettingsQueueFixture {
        desktop,
        requests,
        supervisor,
        _directory,
    } = desktop_with_settings_queue()?;
    for closed in [false, true] {
        let caller = desktop.clone();
        let task = tauri::async_runtime::spawn(async move { caller.save_settings(draft()).await });
        let SettingsCommand::Save {
            draft: received,
            reply,
            _reservation: reservation,
        } = requests.recv_timeout(Duration::from_secs(1))?
        else {
            return Err("renderer Save did not reach Settings queue".into());
        };
        assert_eq!(received.hotkey.key, crate::settings::PhysicalKey::K);
        assert!(reserved(&desktop)?);
        assert_eq!(desktop.0.admissions.available_permits(), 15);
        assert_eq!(
            tauri::async_runtime::block_on(desktop.save_settings(draft())),
            Err("Eine Aufnahme oder Einstellungsänderung wird noch gestartet".into())
        );
        assert!(requests.is_empty());
        let expected = if closed {
            drop(reply);
            "Einstellungsanfrage beendet"
        } else {
            reply
                .send(Err("Kontrollierter Speicherfehler".into()))
                .map_err(|_| "ACK receiver closed")?;
            "Kontrollierter Speicherfehler"
        };
        assert_eq!(tauri::async_runtime::block_on(task)?, Err(expected.into()));
        assert_eq!(desktop.0.admissions.available_permits(), 16);
        assert!(
            reserved(&desktop)?,
            "caller ACK must not release the owner's reservation"
        );
        drop(reservation);
        assert!(!reserved(&desktop)?);
        assert!(
            requests.is_empty(),
            "failed request must not retry automatically"
        );
    }
    supervisor.join()?;
    Ok(())
}

#[test]
fn renderer_reset_preserves_recovery_and_owner_after_closed_ack() -> TestResult {
    let SettingsQueueFixture {
        desktop,
        requests,
        supervisor,
        _directory,
    } = desktop_with_settings_queue()?;
    {
        let mut shared = desktop.0.shared.lock().map_err(|_| "desktop lock")?;
        shared.preferences.recovery_required = true;
        shared.view.preferences.recovery_required = true;
    }
    let caller = desktop.clone();
    let task = tauri::async_runtime::spawn(async move { caller.reset_settings().await });
    let SettingsCommand::Reset {
        reply,
        _reservation: reservation,
    } = requests.recv_timeout(Duration::from_secs(1))?
    else {
        return Err("renderer Reset did not reach Settings queue".into());
    };
    assert!(reserved(&desktop)?);
    assert_eq!(desktop.0.admissions.available_permits(), 15);
    assert_eq!(
        tauri::async_runtime::block_on(desktop.reset_settings()),
        Err("Eine Einstellungsanfrage läuft bereits".into())
    );
    assert!(requests.is_empty());
    drop(reply);
    assert_eq!(
        tauri::async_runtime::block_on(task)?,
        Err("Zurücksetzen wurde beendet".into())
    );
    assert!(reserved(&desktop)?);
    assert!(
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .preferences
            .recovery_required
    );
    drop(reservation);
    assert!(!reserved(&desktop)?);
    assert!(requests.is_empty());
    supervisor.join()?;
    Ok(())
}
