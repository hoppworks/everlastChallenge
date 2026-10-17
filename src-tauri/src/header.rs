//! Projection for native window controls; session authority remains in Runtime.
use crate::contracts::{PermissionStatus, SessionOutcome, SessionPhase, SetupPhase, Snapshot};
use macos_integration::glass_header::{HeaderPrimary, HeaderState};
use std::num::NonZeroU64;

pub fn project(snapshot: &Snapshot) -> HeaderState {
    if !snapshot.running {
        return HeaderState {
            status: "Anwendung wurde beendet".into(),
            ..HeaderState::default()
        };
    }
    let idle = snapshot.session.phase == SessionPhase::Idle;
    let session = snapshot
        .session
        .id
        .filter(|id| *id <= 9_007_199_254_740_991)
        .and_then(NonZeroU64::new);
    let primary = match snapshot.session.phase {
        SessionPhase::Idle
            if snapshot.setup.phase == SetupPhase::Ready
                && snapshot.permissions.microphone == PermissionStatus::Granted
                && !snapshot.settings_busy =>
        {
            Some(HeaderPrimary::Start)
        }
        SessionPhase::Recording => session.map(HeaderPrimary::Stop),
        _ => None,
    };
    let status = match snapshot.session.phase {
        SessionPhase::Starting => "Mikrofon wird gestartet",
        SessionPhase::Recording if snapshot.session.maximum_warning => {
            "Aufnahme endet bei 5 Minuten"
        }
        SessionPhase::Recording => "Aufnahme läuft",
        SessionPhase::Finalizing => "Diktat wird verarbeitet",
        SessionPhase::Delivering => "Text wird übergeben",
        SessionPhase::Cancelling => "Aufnahme wird beendet",
        SessionPhase::Idle => match snapshot.setup.phase {
            SetupPhase::Missing => "Modell einrichten",
            SetupPhase::Checking => "Modell wird geprüft",
            SetupPhase::Downloading => "Modell wird geladen",
            SetupPhase::Loading => "Spracherkennung wird gestartet",
            SetupPhase::Cancelling => "Einrichtung wird abgebrochen",
            SetupPhase::Cancelled => "Einrichtung abgebrochen",
            SetupPhase::Failed => "Modell konnte nicht eingerichtet werden",
            SetupPhase::Ready => match &snapshot.session.outcome {
                Some(SessionOutcome::Local) => "Diktat ist fertig",
                Some(SessionOutcome::Empty) => "Kein Sprachinhalt erkannt",
                Some(SessionOutcome::Sent) => "Einfügebefehl gesendet",
                Some(SessionOutcome::Cancelled) => "Diktat abgebrochen",
                Some(SessionOutcome::Held { .. }) => "Text wurde zurückgehalten",
                Some(SessionOutcome::Failed { .. }) => "Diktat konnte nicht abgeschlossen werden",
                None if snapshot.permissions.microphone == PermissionStatus::Granted => {
                    "Bereit zum Diktieren"
                }
                None => "Mikrofonzugriff erforderlich",
            },
        },
    };
    HeaderState {
        status: status.into(),
        primary,
        cancel: (!idle && snapshot.session.phase != SessionPhase::Cancelling)
            .then_some(session)
            .flatten(),
        settings_enabled: !snapshot.settings_busy,
    }
}
