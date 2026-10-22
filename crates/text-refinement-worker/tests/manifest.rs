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
