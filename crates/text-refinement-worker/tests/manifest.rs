use text_refinement_worker::manifest::{ADAPTER_VERSION, adapted_config, pinned_manifest};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn adapter_adds_only_the_exact_template_and_canonicalizes_keys() -> TestResult {
    let original = br#"{"z":3,"a":{"b":true,"a":false}}"#;
    let adapted = adapted_config(original, "{{messages}}").map_err(|_| "adapter failed")?;
    assert_eq!(
        String::from_utf8(adapted)?,
        r#"{"a":{"a":false,"b":true},"chat_template":"{{messages}}","z":3}"#
    );
    assert_eq!(original, br#"{"z":3,"a":{"b":true,"a":false}}"#);
    Ok(())
}

#[test]
fn adapter_rejects_nonobjects_and_oversized_inputs() {
    assert!(adapted_config(b"[]", "template").is_err());
    assert!(adapted_config(&vec![b' '; 65537], "template").is_err());
    assert!(adapted_config(b"{}", &"x".repeat(65537)).is_err());
}

#[test]
fn manifest_pins_every_required_asset_and_has_no_path_components() -> TestResult {
    let manifest = pinned_manifest().map_err(|_| "manifest failed")?;
    assert_eq!(manifest.adapter_version, ADAPTER_VERSION);
    assert_eq!(
        manifest.revision,
        "50d427756c6b1b2fe0c0a10f67fbda1fc8e82c1b"
    );
    for asset in &manifest.files {
        assert!(!asset.name.contains(['/', '\\']));
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.bytes > 0);
    }
    for required in [
        "model.safetensors",
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "chat_template.jinja",
        "generation_config.json",
    ] {
        assert_eq!(
            manifest
                .files
                .iter()
                .filter(|asset| asset.name == required)
                .count(),
            1
        );
    }
    Ok(())
}

#[test]
fn cancelled_snapshot_never_loads_and_wrong_hash_is_rejected() -> TestResult {
    use std::sync::atomic::AtomicBool;
    use text_refinement_worker::manifest::snapshot;
    let source = tempfile::tempdir()?;
    assert!(matches!(
        snapshot(source.path(), source.path(), &AtomicBool::new(true)),
        Err(text_refinement_protocol::FailureKind::Incomplete)
    ));
    let first = pinned_manifest().map_err(|_| "manifest")?.files.remove(0);
    std::fs::write(
        source.path().join(first.name),
        vec![0u8; first.bytes as usize],
    )?;
    assert!(matches!(
        snapshot(source.path(), source.path(), &AtomicBool::new(false)),
        Err(text_refinement_protocol::FailureKind::ModelLoad)
    ));
    Ok(())
}

#[test]
fn template_reads_are_bounded_and_reject_symbolic_links() -> TestResult {
    use text_refinement_worker::manifest::read_template;
    let source = tempfile::tempdir()?;
    let path = source.path().join("chat_template.jinja");
    std::fs::write(&path, vec![b'x'; 65537])?;
    assert!(read_template(source.path()).is_err());
    std::fs::remove_file(&path)?;
    #[cfg(unix)]
    {
        let other = source.path().join("other");
        std::fs::write(&other, "template")?;
        std::os::unix::fs::symlink(&other, &path)?;
        assert!(read_template(source.path()).is_err());
    }
    Ok(())
}

#[test]
fn owner_lease_protects_directory_until_the_worker_releases_it() -> TestResult {
    use text_refinement_worker::manifest::owner_lease;
    let root = tempfile::tempdir()?;
    let path = root.path().join(".owner.lock");
    std::fs::write(&path, b"everlast-text-owner-v1\n")?;
    let held = owner_lease(root.path()).map_err(|_| "valid lease rejected")?;
    let other = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    assert!(matches!(
        other.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(held);
    other.try_lock()?;
    other.unlock()?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn owner_lease_rejects_missing_unknown_or_symlinked_markers() -> TestResult {
    use text_refinement_worker::manifest::owner_lease;
    let root = tempfile::tempdir()?;
    assert!(owner_lease(root.path()).is_err());
    let path = root.path().join(".owner.lock");
    std::fs::write(&path, b"foreign file")?;
    assert!(owner_lease(root.path()).is_err());
    std::fs::remove_file(&path)?;
    let target = root.path().join("target");
    std::fs::write(&target, b"everlast-text-owner-v1\n")?;
    std::os::unix::fs::symlink(&target, &path)?;
    assert!(owner_lease(root.path()).is_err());
    Ok(())
}

#[test]
fn corrupt_later_asset_cannot_publish_a_partial_snapshot() -> TestResult {
    use std::sync::atomic::AtomicBool;
    use text_refinement_protocol::FailureKind;
    use text_refinement_worker::manifest::{snapshot, verify};
    let source = tempfile::tempdir()?;
    let mut owner_builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        owner_builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let owner = owner_builder.tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(owner.path().metadata()?.permissions().mode() & 0o777, 0o700);
    }
    let manifest = pinned_manifest().map_err(|_| "manifest")?;
    for asset in &manifest.files {
        std::fs::write(source.path().join(&asset.name), [])?;
    }
    // This existing attribution file is byte-identical to the first pinned
    // model asset. Exercise real hash/copy work before a later asset fails.
    let first = include_bytes!("../../../docs/licenses/qwen3-4b-mlx-model-card.txt");
    assert_eq!(manifest.files[0].name, "README.md");
    assert_eq!(u64::try_from(first.len())?, manifest.files[0].bytes);
    use sha2::{Digest, Sha256};
    let digest: String = Sha256::digest(first)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(digest, manifest.files[0].sha256);
    std::fs::write(source.path().join(&manifest.files[0].name), first)?;
    std::fs::write(
        source.path().join(&manifest.files[1].name),
        vec![0; usize::try_from(manifest.files[1].bytes)?],
    )?;
    let cancel = AtomicBool::new(false);
    assert!(matches!(
        verify(source.path(), &cancel),
        Err(FailureKind::ModelLoad)
    ));
    assert!(matches!(
        snapshot(source.path(), owner.path(), &cancel),
        Err(FailureKind::ModelLoad)
    ));
    assert_eq!(std::fs::read_dir(owner.path())?.count(), 0);
    assert_eq!(
        std::fs::read(source.path().join(&manifest.files[0].name))?,
        first
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn verification_rejects_foreign_entries_and_nonprivate_snapshot_parents() -> TestResult {
    use std::{os::unix::fs::PermissionsExt, sync::atomic::AtomicBool};
    use text_refinement_protocol::FailureKind;
    use text_refinement_worker::manifest::{snapshot, verify};
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    std::fs::create_dir(&source)?;
    let cancel = AtomicBool::new(false);
    assert!(matches!(
        verify(&source, &cancel),
        Err(FailureKind::ModelLoad)
    ));
    std::fs::write(source.join("foreign-file"), b"keep")?;
    assert!(matches!(
        verify(&source, &cancel),
        Err(FailureKind::ModelLoad)
    ));
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(&source, &alias)?;
    assert!(matches!(
        verify(&alias, &cancel),
        Err(FailureKind::ModelLoad)
    ));
    assert!(matches!(
        verify(std::path::Path::new("."), &cancel),
        Err(FailureKind::ModelLoad)
    ));
    let owner = root.path().join("owner");
    std::fs::create_dir(&owner)?;
    std::fs::set_permissions(&owner, std::fs::Permissions::from_mode(0o755))?;
    assert!(matches!(
        snapshot(&source, &owner, &cancel),
        Err(FailureKind::ModelLoad)
    ));
    assert_eq!(std::fs::read(source.join("foreign-file"))?, b"keep");
    assert_eq!(std::fs::read_dir(owner)?.count(), 0);
    Ok(())
}
