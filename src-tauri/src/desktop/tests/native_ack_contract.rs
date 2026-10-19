//! Native callback -> actual bounded SettingsCommand queue -> asynchronous ACK.
//! No Desktop owner loop/window is started: action_error is the ACK boundary,
//! not proof of an actual native window projection or a SettingsStore write.
use super::*;
use macos_integration::{
    native_preferences::{PreferencesAction, PreferencesDraft},
    native_workspace::WorkspaceAction,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn draft() -> PreferencesDraft {
    PreferencesDraft {
        device: None,
        language: "german".into(),
        key: 'K',
        option: true,
        command: false,
        control: false,
        shift: false,
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
fn await_ack_error(desktop: &Desktop, expected: &str) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let matches = desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .action_error
            .as_deref()
            == Some(expected);
        if matches && desktop.0.admissions.available_permits() == 16 {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(
                "native ACK did not publish the expected failure/release its permit".into(),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn native_save_holds_admission_until_owner_release_and_retains_failed_ack() -> TestResult {
    let SettingsQueueFixture {
        desktop,
        requests,
        supervisor,
        _directory,
    } = desktop_with_settings_queue()?;
    desktop.native_preferences_action(PreferencesAction::Save(draft()));
    let SettingsCommand::Save {
        draft: received,
        reply,
        _reservation: reservation,
    } = requests.recv_timeout(Duration::from_secs(1))?
    else {
        return Err("native Save did not reach the Settings queue".into());
    };
    assert_eq!(received.hotkey.key, crate::settings::PhysicalKey::K);
    assert_eq!(received.device, None);
    assert!(reserved(&desktop)?);
    assert_eq!(desktop.0.admissions.available_permits(), 15);
    assert!(
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .action_error
            .is_none()
    );
    desktop.native_preferences_action(PreferencesAction::Save(draft()));
    assert!(
        requests.is_empty(),
        "held Save admitted another owner command"
    );
    assert_eq!(
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .action_error
            .as_deref(),
        Some("Eine Aufnahme oder Einstellungsänderung wird noch gestartet")
    );
    reply
        .send(Err("Kontrollierter Speicherfehler".into()))
        .map_err(|_| "native ACK receiver closed")?;
    await_ack_error(&desktop, "Kontrollierter Speicherfehler")?;
    assert!(
        reserved(&desktop)?,
        "ACK delivery must not release the owner's reservation"
    );
    drop(reservation);
    assert!(!reserved(&desktop)?);
    // A fresh explicit user action can retry; no automatic retry was queued.
    assert!(requests.is_empty());
    desktop.native_preferences_action(PreferencesAction::Save(draft()));
    let SettingsCommand::Save {
        reply,
        _reservation: reservation,
        ..
    } = requests.recv_timeout(Duration::from_secs(1))?
    else {
        return Err("explicit native retry did not reach Settings queue".into());
    };
    assert!(reserved(&desktop)?);
    assert!(
        desktop
            .0
            .shared
            .lock()
            .map_err(|_| "desktop lock")?
            .action_error
            .is_none()
    );
    // Close this controlled retry without claiming a real Store commit.
    drop(reply);
    drop(reservation);
    await_ack_error(&desktop, "Einstellungsanfrage beendet")?;
    assert!(!reserved(&desktop)?);
    supervisor.join()?;
    Ok(())
}

#[test]
fn native_confirmed_recovery_reset_retains_closed_ack_without_releasing_owner_early() -> TestResult
{
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
    // The real NSAlert/confirmation gate has its own native contract. This starts
    // at its typed confirmed callback, never opens a dialog or resets a file.
    desktop.native_workspace_action(WorkspaceAction::ResetSettingsConfirmed);
    let SettingsCommand::Reset {
        reply,
        _reservation: reservation,
    } = requests.recv_timeout(Duration::from_secs(1))?
    else {
        return Err("confirmed native Reset did not reach Settings queue".into());
    };
    assert!(reserved(&desktop)?);
    assert_eq!(desktop.0.admissions.available_permits(), 15);
    desktop.native_workspace_action(WorkspaceAction::ResetSettingsConfirmed);
    assert!(
        requests.is_empty(),
        "held Reset admitted a second owner command"
    );
    drop(reply);
    await_ack_error(&desktop, "Zurücksetzen wurde beendet")?;
    assert!(
        reserved(&desktop)?,
        "closed ACK released a still-owned command"
    );
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
    desktop.native_workspace_action(WorkspaceAction::ResetSettingsConfirmed);
    let SettingsCommand::Reset {
        reply,
        _reservation: reservation,
    } = requests.recv_timeout(Duration::from_secs(1))?
    else {
        return Err("explicit confirmed Reset retry was not admitted".into());
    };
    drop(reply);
    drop(reservation);
    await_ack_error(&desktop, "Zurücksetzen wurde beendet")?;
    assert!(!reserved(&desktop)?);
    supervisor.join()?;
    Ok(())
}
