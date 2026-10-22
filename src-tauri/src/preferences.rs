//! Blocking preference transactions. Call only from the supervisor's serialized
//! worker, never a GUI callback. A degraded result requires disabling shortcuts
//! and blocking further edits until the application re-establishes known state.
use crate::settings::{Hotkey, Settings, SettingsError, SettingsLanguage, SettingsStore};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

pub const SUPPORTED_LANGUAGES: &[SettingsLanguage] = &[SettingsLanguage::German];

/// The product exposes only the qualified hybrid. Legacy files migrate at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceModel {
    MlxReferenceEncoder,
}
impl PreferenceModel {
    pub fn from_settings(settings: &Settings) -> Option<Self> {
        match settings.model {
            #[cfg(feature = "mlx")]
            crate::settings::ModelChoice::MlxReferenceEncoder => Some(Self::MlxReferenceEncoder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SettingsDraft {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<PreferenceModel>,
    pub device: Option<String>,
    pub language: SettingsLanguage,
    pub hotkey: Hotkey,
}
impl SettingsDraft {
    pub fn apply_to(self, current: &Settings) -> Result<Settings, PreferencesError> {
        if !SUPPORTED_LANGUAGES.contains(&self.language) {
            return Err(PreferencesError::LanguageUnavailable);
        }
        let model = match self.model {
            None => Settings::default().model,
            #[cfg(feature = "mlx")]
            Some(PreferenceModel::MlxReferenceEncoder) => {
                crate::settings::ModelChoice::MlxReferenceEncoder
            }
            #[cfg(not(feature = "mlx"))]
            Some(PreferenceModel::MlxReferenceEncoder) => {
                return Err(PreferencesError::ModelUnavailable);
            }
        };
        let next = Settings {
            model,
            device: self.device,
            language: self.language,
            hotkey: self.hotkey,
            ..current.clone()
        };
        next.validate().map_err(PreferencesError::Invalid)?;
        Ok(next)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreferencesError {
    #[error("This engine is unavailable in this build")]
    ModelUnavailable,
    #[error("This language has not completed acceptance testing")]
    LanguageUnavailable,
    #[error("Preferences could not be read safely")]
    Unreadable,
    #[error("Preferences changed since they were displayed")]
    Stale,
    #[error("Shortcut registration failed; previous state restored")]
    Registration,
    #[error("Preferences could not be saved; previous state restored")]
    Storage,
    #[error("Preference rollback could not be verified")]
    Degraded {
        binding_restored: bool,
        file_restored: bool,
    },
    #[error("Invalid preference draft")]
    Invalid(#[source] SettingsError),
}

/// The adapter reports the binding it actually owns, not a requested value.
/// A failed rebind may have changed that binding; rollback verifies it explicitly.
pub trait HotkeyRebinder {
    type Error;
    fn binding(&self) -> Option<Hotkey>;
    fn rebind(&mut self, requested: Option<&Hotkey>) -> Result<(), Self::Error>;
}
pub struct PreferencesStore {
    store: SettingsStore,
    directory: PathBuf,
}
impl PreferencesStore {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            store: SettingsStore::new(&directory),
            directory: directory.as_ref().to_owned(),
        }
    }
    pub fn commit(
        &self,
        current: &Settings,
        draft: SettingsDraft,
        binder: &mut impl HotkeyRebinder,
    ) -> Result<Settings, PreferencesError> {
        commit(self, current, draft, binder)
    }
}

// A private file seam permits deterministic failures after rename without
// widening the product API or depending on filesystem permissions under root.
#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    settings: Option<Settings>,
    bytes: Option<Vec<u8>>,
}
trait Storage {
    fn snapshot(&self) -> Result<Snapshot, SettingsError>;
    fn save(&self, next: &Settings) -> Result<(), SettingsError>;
    fn restore(&self, previous: &Snapshot, attempted: &Settings) -> Result<(), SettingsError>;
}
impl Storage for PreferencesStore {
    fn snapshot(&self) -> Result<Snapshot, SettingsError> {
        use std::io::Read;
        let settings = self.store.load()?;
        let path = self.directory.join("settings.json");
        let bytes = match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && settings.is_none() => {
                None
            }
            Err(error) => return Err(error.into()),
            Ok(metadata) => {
                if !metadata.file_type().is_file() {
                    return Err(SettingsError::FileType);
                }
                let mut bytes = Vec::new();
                std::fs::File::open(&path)?
                    .take(16 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16 * 1024 {
                    return Err(SettingsError::TooLarge);
                }
                let parsed: Settings =
                    serde_json::from_slice(&bytes).map_err(|_| SettingsError::Format)?;
                parsed.validate()?;
                if Some(parsed) != settings {
                    return Err(SettingsError::Format);
                }
                Some(bytes)
            }
        };
        Ok(Snapshot { settings, bytes })
    }
    fn save(&self, next: &Settings) -> Result<(), SettingsError> {
        self.store.save(next)
    }
    fn restore(&self, previous: &Snapshot, attempted: &Settings) -> Result<(), SettingsError> {
        let actual = self.snapshot()?;
        if actual == *previous {
            return Ok(());
        }
        // Never overwrite a third-party edit or malformed replacement.
        if actual.settings.as_ref() != Some(attempted) {
            return Err(SettingsError::Format);
        }
        match &previous.bytes {
            Some(bytes) => restore_bytes(&self.directory, bytes),
            None => {
                std::fs::remove_file(self.directory.join("settings.json"))?;
                std::fs::File::open(&self.directory)?.sync_all()?;
                Ok(())
            }
        }
    }
}
fn restore_bytes(directory: &Path, bytes: &[u8]) -> Result<(), SettingsError> {
    use std::{
        io::Write,
        os::unix::fs::OpenOptionsExt,
        sync::atomic::{AtomicU64, Ordering},
    };
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    for _ in 0..16 {
        let path = directory.join(format!(
            ".preferences-rollback-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let outcome = (|| -> Result<(), SettingsError> {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&path, directory.join("settings.json"))?;
            std::fs::File::open(directory)?.sync_all()?;
            Ok(())
        })();
        if let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Preferences rollback temporary cleanup failed");
        }
        return outcome;
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Rollback temporary names occupied",
    )
    .into())
}
fn commit(
    storage: &impl Storage,
    current: &Settings,
    draft: SettingsDraft,
    binder: &mut impl HotkeyRebinder,
) -> Result<Settings, PreferencesError> {
    let next = draft.apply_to(current)?;
    let previous = storage
        .snapshot()
        .map_err(|_| PreferencesError::Unreadable)?;
    if previous.settings.clone().unwrap_or_default() != *current {
        return Err(PreferencesError::Stale);
    }
    let binding = binder.binding();
    if binding.as_ref() != Some(&next.hotkey)
        && (binder.rebind(Some(&next.hotkey)).is_err()
            || binder.binding().as_ref() != Some(&next.hotkey))
    {
        return Err(rollback(
            storage,
            &previous,
            &next,
            binder,
            &binding,
            PreferencesError::Registration,
            false,
        ));
    }
    if storage.save(&next).is_err()
        || !storage
            .snapshot()
            .is_ok_and(|actual| actual.settings.as_ref() == Some(&next))
    {
        return Err(rollback(
            storage,
            &previous,
            &next,
            binder,
            &binding,
            PreferencesError::Storage,
            true,
        ));
    }
    Ok(next)
}
fn rollback(
    storage: &impl Storage,
    previous: &Snapshot,
    attempted: &Settings,
    binder: &mut impl HotkeyRebinder,
    binding: &Option<Hotkey>,
    ordinary: PreferencesError,
    restore_file: bool,
) -> PreferencesError {
    let mut binding_restored = binder.binding() == *binding;
    if !binding_restored {
        binding_restored = binder.rebind(binding.as_ref()).is_ok() && binder.binding() == *binding;
    }
    let mut file_restored = storage.snapshot().is_ok_and(|actual| actual == *previous);
    if !file_restored && restore_file {
        file_restored = storage.restore(previous, attempted).is_ok()
            && storage.snapshot().is_ok_and(|actual| actual == *previous);
    }
    if binding_restored && file_restored {
        ordinary
    } else {
        PreferencesError::Degraded {
            binding_restored,
            file_restored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Binding {
        value: Option<Hotkey>,
        reject: bool,
    }
    impl HotkeyRebinder for Binding {
        type Error = ();
        fn binding(&self) -> Option<Hotkey> {
            self.value.clone()
        }
        fn rebind(&mut self, value: Option<&Hotkey>) -> Result<(), ()> {
            if self.reject {
                return Err(());
            }
            self.value = value.cloned();
            Ok(())
        }
    }
    fn draft(current: &Settings) -> SettingsDraft {
        SettingsDraft {
            model: None,
            device: Some("Studio".into()),
            language: SettingsLanguage::German,
            hotkey: current.hotkey.clone(),
        }
    }
    struct FailingSave {
        real: PreferencesStore,
        after_rename: bool,
        fail_restore: bool,
    }
    impl Storage for FailingSave {
        fn snapshot(&self) -> Result<Snapshot, SettingsError> {
            self.real.snapshot()
        }
        fn save(&self, next: &Settings) -> Result<(), SettingsError> {
            if self.after_rename {
                self.real.save(next)?;
            }
            Err(std::io::Error::other("injected save failure").into())
        }
        fn restore(&self, previous: &Snapshot, attempted: &Settings) -> Result<(), SettingsError> {
            if self.fail_restore {
                return Err(std::io::Error::other("injected rollback failure").into());
            }
            self.real.restore(previous, attempted)
        }
    }
    struct CannotRollback {
        value: Option<Hotkey>,
    }
    impl HotkeyRebinder for CannotRollback {
        type Error = ();
        fn binding(&self) -> Option<Hotkey> {
            self.value.clone()
        }
        fn rebind(&mut self, value: Option<&Hotkey>) -> Result<(), ()> {
            if value.is_none() {
                return Err(());
            }
            self.value = value.cloned();
            Ok(())
        }
    }
    struct PartialFailure {
        value: Option<Hotkey>,
        first: bool,
        lie: bool,
    }
    impl HotkeyRebinder for PartialFailure {
        type Error = ();
        fn binding(&self) -> Option<Hotkey> {
            self.value.clone()
        }
        fn rebind(&mut self, value: Option<&Hotkey>) -> Result<(), ()> {
            if self.lie {
                return Ok(());
            }
            self.value = value.cloned();
            if self.first {
                self.first = false;
                Err(())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn partially_failed_registration_rolls_back_and_false_success_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        for lie in [false, true] {
            let directory = tempfile::tempdir()?;
            let store = PreferencesStore::new(directory.path());
            let current = Settings::default();
            let mut binder = PartialFailure {
                value: None,
                first: true,
                lie,
            };
            assert!(matches!(
                store.commit(&current, draft(&current), &mut binder),
                Err(PreferencesError::Registration)
            ));
            assert_eq!(binder.binding(), None);
            assert_eq!(store.store.load()?, None);
        }
        Ok(())
    }
    #[test]
    fn unsafe_backup_paths_and_oversized_files_are_preserved_without_binding_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        let outside = tempfile::NamedTempFile::new()?;
        std::fs::write(outside.path(), serde_json::to_vec(&Settings::default())?)?;
        let store = PreferencesStore::new(directory.path());
        let current = Settings::default();
        let mut binder = Binding {
            value: None,
            reject: false,
        };
        symlink(outside.path(), &path)?;
        assert!(matches!(
            store.commit(&current, draft(&current), &mut binder),
            Err(PreferencesError::Unreadable)
        ));
        assert!(std::fs::symlink_metadata(&path)?.file_type().is_symlink());
        std::fs::remove_file(&path)?;
        std::fs::write(&path, vec![b' '; 16 * 1024 + 1])?;
        assert!(matches!(
            store.commit(&current, draft(&current), &mut binder),
            Err(PreferencesError::Unreadable)
        ));
        assert_eq!(std::fs::metadata(&path)?.len(), 16 * 1024 + 1);
        assert_eq!(binder.binding(), None);
        Ok(())
    }
    #[test]
    fn failed_native_rollback_is_degraded_even_when_the_file_is_restored()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let current = Settings::default();
        let storage = FailingSave {
            real: PreferencesStore::new(directory.path()),
            after_rename: true,
            fail_restore: false,
        };
        let mut binder = CannotRollback { value: None };
        assert!(matches!(
            commit(&storage, &current, draft(&current), &mut binder),
            Err(PreferencesError::Degraded {
                binding_restored: false,
                file_restored: true
            })
        ));
        assert_eq!(storage.real.store.load()?, None);
        Ok(())
    }
    #[test]
    fn malformed_or_stale_files_and_invalid_drafts_never_touch_bindings()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = PreferencesStore::new(directory.path());
        let current = Settings::default();
        let mut binder = Binding {
            value: None,
            reject: false,
        };
        let mut invalid = draft(&current);
        invalid.device = Some(" ".into());
        assert!(matches!(
            store.commit(&current, invalid, &mut binder),
            Err(PreferencesError::Invalid(_))
        ));
        assert_eq!(binder.binding(), None);
        std::fs::write(directory.path().join("settings.json"), b"private invalid")?;
        assert!(matches!(
            store.commit(&current, draft(&current), &mut binder),
            Err(PreferencesError::Unreadable)
        ));
        assert_eq!(
            std::fs::read(directory.path().join("settings.json"))?,
            b"private invalid"
        );
        assert_eq!(binder.binding(), None);
        let mut changed = current.clone();
        changed.device = Some("Other".into());
        std::fs::write(
            directory.path().join("settings.json"),
            serde_json::to_vec(&changed)?,
        )?;
        assert!(matches!(
            store.commit(&current, draft(&current), &mut binder),
            Err(PreferencesError::Stale)
        ));
        assert_eq!(binder.binding(), None);
        Ok(())
    }
    #[test]
    fn renderer_rejects_unqualified_model_schema_or_unknown_shortcut_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let current = Settings::default();
        let plain = serde_json::to_value(draft(&current))?;
        for field in ["model", "schema_version", "anything"] {
            let mut invalid = plain.clone();
            invalid[field] = "bad".into();
            assert!(serde_json::from_value::<SettingsDraft>(invalid).is_err());
        }
        let mut invalid = plain;
        invalid["hotkey"]["extra"] = true.into();
        assert!(serde_json::from_value::<SettingsDraft>(invalid).is_err());
        Ok(())
    }
    #[test]
    fn registration_failure_leaves_the_exact_file_and_binding_untouched()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let current = Settings::default();
        let store = PreferencesStore::new(directory.path());
        store.store.save(&current)?;
        let before = std::fs::read(directory.path().join("settings.json"))?;
        let mut binder = Binding {
            value: Some(current.hotkey.clone()),
            reject: true,
        };
        let mut requested = draft(&current);
        requested.hotkey.key = crate::settings::PhysicalKey::K;
        assert!(matches!(
            store.commit(&current, requested, &mut binder),
            Err(PreferencesError::Registration)
        ));
        assert_eq!(binder.binding(), Some(current.hotkey));
        assert_eq!(
            std::fs::read(directory.path().join("settings.json"))?,
            before
        );
        Ok(())
    }
    #[test]
    fn save_failures_restore_original_bytes_or_absence_and_actual_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        for existed in [false, true] {
            for after_rename in [false, true] {
                let directory = tempfile::tempdir()?;
                let current = Settings::default();
                let real = PreferencesStore::new(directory.path());
                if existed {
                    std::fs::write(
                        directory.path().join("settings.json"),
                        format!("  {}\n", serde_json::to_string(&current)?),
                    )?;
                }
                let previous = real.snapshot()?;
                let storage = FailingSave {
                    real,
                    after_rename,
                    fail_restore: false,
                };
                let mut binder = Binding {
                    value: None,
                    reject: false,
                };
                assert!(matches!(
                    commit(&storage, &current, draft(&current), &mut binder),
                    Err(PreferencesError::Storage)
                ));
                assert_eq!(binder.binding(), None);
                assert!(storage.snapshot()? == previous);
                assert_eq!(
                    std::fs::read_dir(directory.path())?.count(),
                    usize::from(existed)
                );
            }
        }
        Ok(())
    }
    #[test]
    fn unverifiable_file_rollback_is_explicitly_degraded() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let current = Settings::default();
        let storage = FailingSave {
            real: PreferencesStore::new(directory.path()),
            after_rename: true,
            fail_restore: true,
        };
        let mut binder = Binding {
            value: None,
            reject: false,
        };
        assert!(matches!(
            commit(&storage, &current, draft(&current), &mut binder),
            Err(PreferencesError::Degraded {
                binding_restored: true,
                file_restored: false
            })
        ));
        assert_eq!(binder.binding(), None);
        Ok(())
    }
    #[test]
    fn commit_repairs_missing_binding_and_persists_validated_preferences()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let current = Settings::default();
        let store = PreferencesStore::new(directory.path());
        let mut binder = Binding {
            value: None,
            reject: false,
        };
        let next = store.commit(&current, draft(&current), &mut binder)?;
        assert_eq!(binder.binding(), Some(next.hotkey.clone()));
        assert_eq!(SettingsStore::new(directory.path()).load()?, Some(next));
        Ok(())
    }
    #[test]
    fn draft_rejects_unaccepted_english_and_uses_product_model()
    -> Result<(), Box<dyn std::error::Error>> {
        let current = Settings {
            model: crate::settings::ModelChoice::Small,
            ..Settings::default()
        };
        let mut draft = SettingsDraft {
            model: None,
            device: None,
            language: SettingsLanguage::English,
            hotkey: current.hotkey.clone(),
        };
        assert!(matches!(
            draft.clone().apply_to(&current),
            Err(PreferencesError::LanguageUnavailable)
        ));
        draft.language = SettingsLanguage::German;
        let next = draft.apply_to(&current)?;
        assert_eq!(next.model, Settings::default().model);
        assert_eq!(next.schema_version, current.schema_version);
        Ok(())
    }
    #[cfg(feature = "mlx")]
    #[test]
    fn explicit_model_migration_preserves_preferences_and_persists_only_on_commit()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let current = Settings {
            model: crate::settings::ModelChoice::Reference,
            device: Some("Studio".into()),
            ..Settings::default()
        };
        let store = PreferencesStore::new(directory.path());
        store.store.save(&current)?;
        let mut requested = draft(&current);
        requested.model = Some(PreferenceModel::MlxReferenceEncoder);
        let candidate = requested.clone().apply_to(&current)?;
        assert_eq!(
            candidate.model,
            crate::settings::ModelChoice::MlxReferenceEncoder
        );
        assert_eq!(candidate.device, current.device);
        assert_eq!(candidate.hotkey, current.hotkey);
        assert_eq!(store.store.load()?, Some(current.clone()));
        let mut binder = Binding {
            value: Some(current.hotkey.clone()),
            reject: false,
        };
        let next = store.commit(&current, requested, &mut binder)?;
        assert_eq!(store.store.load()?, Some(next));
        Ok(())
    }
    #[cfg(feature = "mlx")]
    #[test]
    fn failed_model_migration_restores_exact_preferences_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let current = Settings {
            model: crate::settings::ModelChoice::Reference,
            ..Settings::default()
        };
        let real = PreferencesStore::new(directory.path());
        real.store.save(&current)?;
        let before = std::fs::read(directory.path().join("settings.json"))?;
        let storage = FailingSave {
            real,
            after_rename: true,
            fail_restore: false,
        };
        let mut requested = draft(&current);
        requested.model = Some(PreferenceModel::MlxReferenceEncoder);
        let mut binder = Binding {
            value: Some(current.hotkey.clone()),
            reject: false,
        };
        assert!(matches!(
            commit(&storage, &current, requested, &mut binder),
            Err(PreferencesError::Storage)
        ));
        assert_eq!(
            std::fs::read(directory.path().join("settings.json"))?,
            before
        );
        assert_eq!(binder.binding(), Some(current.hotkey));
        Ok(())
    }
}
