//! Startup preferences with an explicit, recoverable reset boundary.
use crate::settings::{Settings, SettingsError, SettingsStore};
use std::path::Path;

pub struct StartupSettings {
    store: SettingsStore,
    ready: Option<Settings>,
    error: Option<String>,
}
impl StartupSettings {
    pub fn load(directory: impl AsRef<Path>) -> Self {
        let store = SettingsStore::new(directory);
        let loaded = (|| {
            if let Some(settings) = store.load()? {
                return Ok(settings);
            }
            let defaults = Settings::default();
            store.save(&defaults)?;
            match store.load()? {
                Some(actual) if actual == defaults => Ok(actual),
                _ => Err(SettingsError::Format),
            }
        })();
        match loaded {
            Ok(settings) => Self {
                store,
                ready: Some(settings),
                error: None,
            },
            Err(error) => Self {
                store,
                ready: None,
                error: Some(message(&error).into()),
            },
        }
    }
    pub fn ready(&self) -> Option<&Settings> {
        self.ready.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn reset(&mut self) -> Result<Settings, String> {
        if self.ready.is_some() {
            return Err("Gültige Einstellungen können hier nicht zurückgesetzt werden.".into());
        }
        let result = (|| {
            let defaults = self.store.reset_to_defaults()?;
            match self.store.load()? {
                Some(actual) if actual == defaults && actual == Settings::default() => Ok(actual),
                _ => Err(SettingsError::Format),
            }
        })();
        match result {
            Ok(settings) => {
                self.ready = Some(settings.clone());
                self.error = None;
                Ok(settings)
            }
            Err(error) => {
                let message = message(&error).to_owned();
                self.error = Some(message.clone());
                Err(message)
            }
        }
    }
}

fn message(error: &SettingsError) -> &'static str {
    match error {
        SettingsError::Io(_) => {
            "Die Einstellungsdatei konnte nicht gelesen oder gespeichert werden."
        }
        SettingsError::Version => "Diese Version der Einstellungsdatei wird nicht unterstützt.",
        SettingsError::TooLarge => "Die Einstellungsdatei ist zu groß.",
        SettingsError::FileType => "Der Einstellungspfad ist keine reguläre Datei.",
        SettingsError::Format | SettingsError::Device | SettingsError::Hotkey => {
            "Die Einstellungsdatei ist ungültig."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_valid_preferences_are_preserved_byte_for_byte()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let settings = Settings {
            device: Some("Studio Mikrofon".into()),
            model: crate::settings::ModelChoice::Small,
            ..Settings::default()
        };
        let bytes = format!("  {}\n", serde_json::to_string(&settings)?);
        let path = directory.path().join("settings.json");
        std::fs::write(&path, &bytes)?;
        let mut startup = StartupSettings::load(directory.path());
        assert_eq!(startup.ready(), Some(&settings));
        assert!(startup.reset().is_err());
        assert_eq!(startup.error(), None);
        assert_eq!(std::fs::read(&path)?, bytes.as_bytes());
        Ok(())
    }
    #[test]
    fn symlink_is_preserved_until_reset_and_its_target_is_never_modified()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("source.json");
        let bytes = serde_json::to_vec(&Settings::default())?;
        std::fs::write(&target, &bytes)?;
        let path = directory.path().join("settings.json");
        symlink(&target, &path)?;
        let mut startup = StartupSettings::load(directory.path());
        assert_eq!(startup.ready(), None);
        assert!(startup.error().is_some());
        assert!(std::fs::symlink_metadata(&path)?.file_type().is_symlink());
        assert_eq!(startup.reset()?, Settings::default());
        assert!(std::fs::symlink_metadata(&path)?.file_type().is_file());
        assert_eq!(std::fs::read(&target)?, bytes);
        Ok(())
    }
    #[test]
    fn failed_reset_preserves_recoverability_and_can_be_retried()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        // A directory at the file path makes atomic replacement fail reliably,
        // including when tests run with privileges that bypass chmod protection.
        std::fs::create_dir(&path)?;
        let mut startup = StartupSettings::load(directory.path());
        assert!(startup.error().is_some());
        assert!(startup.reset().is_err());
        assert_eq!(startup.ready(), None);
        assert!(startup.error().is_some());
        assert!(path.is_dir());
        std::fs::remove_dir(&path)?;
        assert_eq!(startup.reset()?, Settings::default());
        assert_eq!(startup.ready(), Some(&Settings::default()));
        assert_eq!(startup.error(), None);
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
    #[test]
    fn failed_initial_default_write_waits_for_explicit_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let obstructed = directory.path().join("data");
        std::fs::write(&obstructed, b"do not replace")?;
        let mut startup = StartupSettings::load(&obstructed);
        assert_eq!(startup.ready(), None);
        assert!(startup.error().is_some());
        assert!(startup.reset().is_err());
        assert_eq!(std::fs::read(&obstructed)?, b"do not replace");
        std::fs::remove_file(&obstructed)?;
        assert_eq!(startup.ready(), None);
        assert_eq!(startup.reset()?, Settings::default());
        Ok(())
    }
    #[test]
    fn invalid_and_future_files_survive_until_explicit_verified_reset()
    -> Result<(), Box<dyn std::error::Error>> {
        for bytes in [
            b"private invalid content".to_vec(),
            serde_json::to_vec(
                &serde_json::json!({"schema_version":999,"device":null,"language":"german","model":"reference","hotkey":{"key":"D","option":true,"control":false,"shift":false,"command":false}}),
            )?,
        ] {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("settings.json");
            std::fs::write(&path, &bytes)?;
            let mut startup = StartupSettings::load(directory.path());
            assert!(startup.ready().is_none());
            assert!(
                startup
                    .error()
                    .is_some_and(|error| error.len() < 200 && !error.contains("private"))
            );
            assert_eq!(std::fs::read(&path)?, bytes);
            assert_eq!(startup.reset()?, Settings::default());
            assert_eq!(startup.ready(), Some(&Settings::default()));
            assert_eq!(startup.error(), None);
            assert_eq!(
                SettingsStore::new(directory.path()).load()?,
                Some(Settings::default())
            );
        }
        Ok(())
    }
    #[test]
    fn missing_file_is_initialized_and_ready_state_rejects_reset()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let mut startup = StartupSettings::load(directory.path());
        assert_eq!(startup.ready(), Some(&Settings::default()));
        assert_eq!(startup.error(), None);
        assert_eq!(
            SettingsStore::new(directory.path()).load()?,
            Some(Settings::default())
        );
        assert!(startup.reset().is_err());
        assert_eq!(startup.ready(), Some(&Settings::default()));
        assert_eq!(startup.error(), None);
        Ok(())
    }
}
