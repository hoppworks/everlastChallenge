//! Validated, versioned preferences; no session state or recorded content.

use crate::{
    model::{ModelDefinition, REFERENCE, SMALL},
    transcription::Language,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use ts_rs::TS;

pub const SCHEMA_VERSION: u32 = 1;
const MAX_FILE_BYTES: usize = 16 * 1024;
const MAX_DEVICE_BYTES: usize = 512;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SettingsLanguage {
    German,
    English,
}
impl From<SettingsLanguage> for Language {
    fn from(value: SettingsLanguage) -> Self {
        match value {
            SettingsLanguage::German => Self::German,
            SettingsLanguage::English => Self::English,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelChoice {
    Reference,
    Small,
    #[cfg(feature = "mlx")]
    MlxFp16,
    #[cfg(feature = "mlx")]
    MlxReferenceEncoder,
}
impl ModelChoice {
    pub fn definition(self) -> ModelDefinition {
        match self {
            Self::Reference => REFERENCE,
            Self::Small => SMALL,
            #[cfg(feature = "mlx")]
            Self::MlxFp16 => crate::model::MLX_REFERENCE,
            #[cfg(feature = "mlx")]
            Self::MlxReferenceEncoder => crate::model::MLX_REFERENCE_COEFFICIENTS,
        }
    }
}

/// Physical ANSI letter position, independent of the current keyboard layout.
/// The native shortcut adapter resolves its displayed label from that layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum PhysicalKey {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Hotkey {
    pub key: PhysicalKey,
    pub option: bool,
    pub command: bool,
    pub control: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub device: Option<String>,
    pub language: SettingsLanguage,
    pub hotkey: Hotkey,
    pub model: ModelChoice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            device: None,
            language: SettingsLanguage::German,
            #[cfg(feature = "mlx")]
            model: ModelChoice::MlxReferenceEncoder,
            #[cfg(not(feature = "mlx"))]
            model: ModelChoice::Reference,
            hotkey: Hotkey {
                key: PhysicalKey::D,
                option: true,
                command: false,
                control: false,
                shift: false,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Preferences file I/O failed")]
    Io(#[from] std::io::Error),
    #[error("Preferences are malformed or contain unsupported fields")]
    Format,
    #[error("Preferences schema version is unsupported")]
    Version,
    #[error("Preferences exceed the maximum file size")]
    TooLarge,
    #[error("Microphone selection is invalid")]
    Device,
    #[error("A shortcut needs Option, Command or Control")]
    Hotkey,
    #[error("Preferences path is not a regular file")]
    FileType,
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(SettingsError::Version);
        }
        if self.device.as_ref().is_some_and(|name| {
            name.is_empty()
                || name.trim() != name
                || name.len() > MAX_DEVICE_BYTES
                || name.chars().any(char::is_control)
        }) {
            return Err(SettingsError::Device);
        }
        if !(self.hotkey.option || self.hotkey.command || self.hotkey.control) {
            return Err(SettingsError::Hotkey);
        }
        Ok(())
    }
}

/// Caller chooses the application's own data directory. Calls are serialized
/// by the app supervisor; active-session/model restrictions belong there.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    directory: PathBuf,
}
impl SettingsStore {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_owned(),
        }
    }

    /// Absence permits first-run defaults; every existing invalid file is an error.
    pub fn load(&self) -> Result<Option<Settings>, SettingsError> {
        let path = self.directory.join("settings.json");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            return Err(SettingsError::FileType);
        }
        let mut bytes = Vec::new();
        File::open(path)?
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let settings: Settings =
            serde_json::from_slice(&bytes).map_err(|_| SettingsError::Format)?;
        settings.validate()?;
        Ok(Some(settings))
    }

    /// Never overwrites an unrecognized or invalid existing configuration.
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        settings.validate()?;
        self.load()?;
        self.write(settings)
    }

    /// Only call after the user explicitly requests resetting preferences.
    pub fn reset_to_defaults(&self) -> Result<Settings, SettingsError> {
        let settings = Settings::default();
        self.write(&settings)?;
        Ok(settings)
    }

    fn write(&self, settings: &Settings) -> Result<(), SettingsError> {
        let bytes = serde_json::to_vec_pretty(settings).map_err(|_| SettingsError::Format)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(SettingsError::TooLarge);
        }
        fs::create_dir_all(&self.directory)?;
        let (mut file, temporary) = self.create_temporary()?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary.0, self.directory.join("settings.json"))?;
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }

    fn create_temporary(&self) -> Result<(File, Temporary), SettingsError> {
        for _ in 0..16 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = self
                .directory
                .join(format!(".settings-{}-{sequence}.tmp", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => return Ok((file, Temporary(path))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Preferences temporary names are occupied",
        )
        .into())
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Preferences temporary file cleanup failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[cfg(feature = "mlx")]
    #[test]
    fn mlx_build_defaults_to_the_qualified_reference_encoder() {
        let selected = Settings::default().model;
        assert_eq!(selected, ModelChoice::MlxReferenceEncoder);
        assert_eq!(
            selected.definition().id(),
            crate::model::MLX_REFERENCE_COEFFICIENTS.id()
        );
    }

    #[cfg(feature = "mlx")]
    #[test]
    fn reference_encoder_model_round_trips_with_its_source_and_derived_assets()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        let selected = Settings {
            model: ModelChoice::MlxReferenceEncoder,
            ..Settings::default()
        };
        store.save(&selected)?;
        let loaded = store.load()?.ok_or("derived settings missing")?;
        assert_eq!(loaded.model, ModelChoice::MlxReferenceEncoder);
        let assets = loaded
            .model
            .definition()
            .installation_assets()
            .collect::<Vec<_>>();
        assert_eq!(assets.len(), 6);
        assert_eq!(assets[0].id(), REFERENCE.id());
        assert_eq!(
            assets[5].id(),
            crate::model::MLX_REFERENCE_COEFFICIENTS.id()
        );
        assert_eq!(
            loaded.model.definition().installation_bytes(),
            3_813_494_037
        );
        assert!(assets[5].url().is_empty());
        Ok(())
    }

    #[cfg(feature = "mlx")]
    #[test]
    fn explicit_mlx_model_survives_settings_storage_with_its_verified_assets()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        let settings = Settings {
            model: ModelChoice::MlxFp16,
            ..Settings::default()
        };
        store.save(&settings)?;
        let loaded = store.load()?.ok_or("missing stored MLX choice")?;
        assert_eq!(
            loaded.model.definition().id(),
            crate::model::MLX_REFERENCE.id()
        );
        assert_eq!(loaded.model.definition().installation_assets().count(), 5);
        Ok(())
    }
    #[test]
    fn missing_preferences_are_distinct_from_a_saved_default()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        assert_eq!(store.load()?, None);
        let saved = Settings::default();
        store.save(&saved)?;
        let settings = store.load()?.ok_or("missing settings")?;
        assert_eq!(settings.language, SettingsLanguage::German);
        assert_eq!(settings.device, None);
        assert_eq!(settings.hotkey.key, PhysicalKey::D);
        assert!(settings.hotkey.option);
        assert_eq!(settings, saved);
        Ok(())
    }

    #[test]
    fn invalid_existing_preferences_survive_save_until_explicit_reset()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        let store = SettingsStore::new(directory.path());
        for invalid in [b"broken JSON".as_slice(), br#"{"schema_version":99}"#] {
            fs::write(&path, invalid)?;
            assert!(store.load().is_err());
            assert!(store.save(&Settings::default()).is_err());
            assert_eq!(fs::read(&path)?, invalid);
        }
        let reset = store.reset_to_defaults()?;
        assert_eq!(store.load()?, Some(reset));
        Ok(())
    }

    #[test]
    fn unsupported_schema_fields_models_and_unbounded_files_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        let store = SettingsStore::new(directory.path());
        let base = serde_json::to_value(Settings::default())?;
        let mut version = base.clone();
        version["schema_version"] = 2.into();
        fs::write(&path, serde_json::to_vec(&version)?)?;
        assert!(matches!(store.load(), Err(SettingsError::Version)));
        let mut model = base.clone();
        model["model"] = "/tmp/arbitrary.bin".into();
        let mut extra = base.clone();
        extra["transcript"] = "must not be accepted".into();
        let mut hotkey = base;
        hotkey["hotkey"]["key"] = "Escape".into();
        for invalid in [model, extra, hotkey] {
            fs::write(&path, serde_json::to_vec(&invalid)?)?;
            assert!(matches!(store.load(), Err(SettingsError::Format)));
        }
        fs::write(&path, vec![b' '; MAX_FILE_BYTES + 1])?;
        assert!(matches!(store.load(), Err(SettingsError::TooLarge)));
        Ok(())
    }

    #[test]
    fn invalid_changes_preserve_previous_values_and_custom_values_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        let mut custom = Settings {
            device: Some("Studio Mikrofon".into()),
            language: SettingsLanguage::English,
            model: ModelChoice::Small,
            ..Settings::default()
        };
        custom.hotkey.key = PhysicalKey::K;
        custom.hotkey.option = false;
        custom.hotkey.control = true;
        custom.hotkey.shift = true;
        store.save(&custom)?;
        let before = fs::read(directory.path().join("settings.json"))?;
        for device in [
            "".to_owned(),
            " trailing ".to_owned(),
            "line\nbreak".to_owned(),
            "x".repeat(513),
        ] {
            let mut invalid = custom.clone();
            invalid.device = Some(device);
            assert!(matches!(store.save(&invalid), Err(SettingsError::Device)));
        }
        let mut invalid = custom.clone();
        invalid.hotkey.control = false;
        assert!(matches!(store.save(&invalid), Err(SettingsError::Hotkey)));
        assert_eq!(fs::read(directory.path().join("settings.json"))?, before);
        assert_eq!(store.load()?, Some(custom));
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }

    #[test]
    fn failed_replacements_clean_temporary_files_and_preserve_the_destination()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        // A directory at the destination forces rename to fail even on reset.
        fs::create_dir(directory.path().join("settings.json"))?;
        assert!(matches!(store.load(), Err(SettingsError::FileType)));
        assert!(store.reset_to_defaults().is_err());
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        assert!(directory.path().join("settings.json").is_dir());
        Ok(())
    }

    #[test]
    fn saved_preferences_are_private_and_symlink_targets_are_never_overwritten()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        let path = directory.path().join("settings.json");
        store.save(&Settings::default())?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        let target = directory.path().join("other.json");
        fs::rename(&path, &target)?;
        let before = fs::read(&target)?;
        symlink(&target, &path)?;
        assert!(matches!(store.load(), Err(SettingsError::FileType)));
        assert!(matches!(
            store.save(&Settings::default()),
            Err(SettingsError::FileType)
        ));
        store.reset_to_defaults()?;
        assert!(fs::symlink_metadata(&path)?.file_type().is_file());
        assert_eq!(fs::read(target)?, before);
        Ok(())
    }
}
