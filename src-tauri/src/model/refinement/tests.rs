use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn tiny() -> Result<RefinementModel, Error> {
    RefinementModel::parse(
        r#"{"repository":"mlx-community/Qwen3-4B-Instruct-2507-4bit","revision":"0000000000000000000000000000000000000000","adapterVersion":1,"adapterSha256":"0000000000000000000000000000000000000000000000000000000000000000","files":[{"name":"one.txt","bytes":3,"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},{"name":"two.txt","bytes":3,"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]}"#,
    )
}
fn fetch(
    directory: &Path,
    asset: Asset<'static>,
    _cancel: &CancellationToken,
    progress: &mut dyn FnMut(u64),
) -> Result<PathBuf, ModelError> {
    let mut output = super::super::storage::VerifiedFile::new(directory, asset.definition())?;
    output.write(b"abc")?;
    progress(3);
    output.finish()
}
#[test]
fn complete_set_becomes_visible_only_after_all_files_are_verified() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let final_path = model.directory(root.path());
    let mut calls = 0;
    let installed = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut |stage, asset, cancel, progress| {
            assert!(!final_path.exists());
            calls += 1;
            fetch(stage, asset, cancel, progress)
        },
    )?;
    assert_eq!(calls, 2);
    assert_eq!(installed, final_path);
    assert_eq!(std::fs::read(installed.join("one.txt"))?, b"abc");
    assert_eq!(std::fs::read(installed.join("two.txt"))?, b"abc");
    Ok(())
}

#[test]
fn failure_after_one_file_preserves_old_revision_and_removes_only_owned_staging() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let mut old = tiny()?;
    old.manifest.revision = "1111111111111111111111111111111111111111";
    let old_path = old.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut fetch,
    )?;
    let mut calls = 0;
    let result = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut |dir, asset, cancel, progress| {
            calls += 1;
            if calls == 2 {
                return Err(ModelError::Network);
            }
            fetch(dir, asset, cancel, progress)
        },
    );
    assert!(matches!(result, Err(Error::Model(ModelError::Network))));
    assert!(!model.directory(root.path()).exists());
    assert_eq!(
        old.inspect(root.path(), &CancellationToken::new())?,
        Some(old_path)
    );
    assert_eq!(fs::read_dir(root.path().join(CACHE_ROOT))?.count(), 2);
    Ok(())
}

#[test]
fn setup_recovers_only_dead_owned_staging_and_preserves_unknown_and_live_entries() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut fetch,
    )?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--exact", "__no_matching_staging_fixture__"])
        .stdout(std::process::Stdio::null())
        .spawn()?;
    let dead_pid = child.id();
    child.wait()?;
    let cache = root.path().join(CACHE_ROOT);
    let dead = tempfile::Builder::new()
        .prefix(&format!(".everlast-text-stage-{dead_pid}-"))
        .tempdir_in(&cache)?
        .keep();
    fs::write(dead.join("one.txt"), b"partial")?;
    let unknown = tempfile::Builder::new()
        .prefix(&format!(".everlast-text-stage-{dead_pid}-"))
        .tempdir_in(&cache)?
        .keep();
    fs::write(unknown.join("personal.txt"), b"keep")?;
    let live = tempfile::Builder::new()
        .prefix(&format!(".everlast-text-stage-{}-", std::process::id()))
        .tempdir_in(&cache)?;
    fs::write(live.path().join("one.txt"), b"partial")?;
    model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Err(ModelError::Closed),
        &mut |_, _, _, _| Err(ModelError::Closed),
    )?;
    assert!(!dead.exists());
    assert_eq!(fs::read(unknown.join("personal.txt"))?, b"keep");
    assert!(live.path().exists());
    Ok(())
}

#[test]
fn bundled_manifest_sizes_and_identity_match_the_worker_asset_set() -> TestResult {
    use sha2::{Digest, Sha256};
    let model = RefinementModel::bundled()?;
    assert_eq!(model.installation_bytes(), 2_278_970_666);
    assert_eq!(model.staging_and_snapshot_bytes()?, 4_625_050_196);
    assert_eq!(model.manifest.files.len(), 12);
    assert_eq!(
        super::super::hex_digest(Sha256::digest(MANIFEST.as_bytes())),
        "fea7cc2a764e60d910da77c44549b7f123e681edb0c84f03658011eb86519352"
    );
    let root = tempfile::tempdir()?;
    assert_eq!(model.inspect(root.path(), &CancellationToken::new())?, None);
    assert!(!root.path().join(CACHE_ROOT).exists());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        model.install(root.path(), &cancel, |_, _| {}),
        Err(Error::Model(ModelError::Cancelled))
    ));
    assert!(!root.path().join(CACHE_ROOT).exists());
    Ok(())
}

#[test]
fn insufficient_space_prevents_transfer_and_cache_hit_needs_no_transfer_or_space_query()
-> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let result = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(0),
        &mut |_, _, _, _| Err(ModelError::Closed),
    );
    assert!(matches!(
        result,
        Err(Error::InsufficientSpace { available: 0, .. })
    ));
    assert!(!model.directory(root.path()).exists());
    let installed = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut fetch,
    )?;
    let hit = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Err(ModelError::Closed),
        &mut |_, _, _, _| Err(ModelError::Closed),
    )?;
    assert_eq!(hit, installed);
    Ok(())
}

#[test]
fn cancellation_after_the_last_file_withholds_publication_and_cleans_staging() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let cancel = CancellationToken::new();
    let mut calls = 0;
    let result = model.install_with(
        root.path(),
        &cancel,
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut |directory, asset, token, progress| {
            let path = fetch(directory, asset, token, progress)?;
            calls += 1;
            if calls == 2 {
                cancel.cancel();
            }
            Ok(path)
        },
    );
    assert!(matches!(result, Err(Error::Model(ModelError::Cancelled))));
    assert!(!model.directory(root.path()).exists());
    assert_eq!(fs::read_dir(root.path().join(CACHE_ROOT))?.count(), 1);
    Ok(())
}

#[test]
fn corrupt_known_cache_is_replaced_only_after_success_and_preserved_as_backup() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let installed = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut fetch,
    )?;
    fs::write(installed.join("two.txt"), b"bad")?;
    assert!(matches!(
        model.inspect(root.path(), &CancellationToken::new()),
        Err(Error::Model(ModelError::Hash))
    ));
    let replaced = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut fetch,
    )?;
    assert_eq!(replaced, installed);
    let backup = fs::read_dir(root.path().join(CACHE_ROOT))?
        .filter_map(Result::ok)
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".everlast-text-replaced-")
        })
        .ok_or("missing preserved corrupt set")?;
    assert_eq!(fs::read(backup.path().join("two.txt"))?, b"bad");
    assert_eq!(
        model.inspect(root.path(), &CancellationToken::new())?,
        Some(installed)
    );
    Ok(())
}

#[test]
fn unsupported_cache_entries_and_symlinks_are_not_repaired_or_followed() -> TestResult {
    use std::os::unix::fs::symlink;
    for redirected in [false, true] {
        let root = tempfile::tempdir()?;
        let model = tiny()?;
        let installed = model.install_with(
            root.path(),
            &CancellationToken::new(),
            &mut |_, _| {},
            |_| Ok(u64::MAX),
            &mut fetch,
        )?;
        let external = root.path().join("external");
        fs::write(&external, b"keep")?;
        if redirected {
            fs::remove_file(installed.join("one.txt"))?;
            symlink(&external, installed.join("one.txt"))?;
        } else {
            fs::write(installed.join("personal.txt"), b"keep")?;
        }
        let result = model.install_with(
            root.path(),
            &CancellationToken::new(),
            &mut |_, _| {},
            |_| Err(ModelError::Closed),
            &mut |_, _, _, _| Err(ModelError::Closed),
        );
        assert!(matches!(result, Err(Error::InvalidCache)));
        assert_eq!(fs::read(external)?, b"keep");
        assert!(installed.exists());
    }
    Ok(())
}

#[test]
fn corruption_during_staging_is_detected_before_directory_publication() -> TestResult {
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let result = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut |directory, asset, token, progress| {
            let path = fetch(directory, asset, token, progress)?;
            fs::write(&path, b"bad")?;
            Ok(path)
        },
    );
    assert!(matches!(result, Err(Error::Model(ModelError::Hash))));
    assert!(!model.directory(root.path()).exists());
    Ok(())
}

#[test]
fn publication_failure_restores_old_set_or_reports_failed_recovery_without_deleting_it()
-> TestResult {
    for fail_recovery in [false, true] {
        let root = tempfile::tempdir()?;
        let model = tiny()?;
        let target = model.install_with(
            root.path(),
            &CancellationToken::new(),
            &mut |_, _| {},
            |_| Ok(u64::MAX),
            &mut fetch,
        )?;
        fs::write(target.join("two.txt"), b"old")?;
        let cache = root.path().join(CACHE_ROOT);
        let stage = tempfile::tempdir_in(&cache)?;
        fs::write(stage.path().join("one.txt"), b"abc")?;
        fs::write(stage.path().join("two.txt"), b"abc")?;
        let mut calls = 0;
        let result = publish_set(stage, target.clone(), true, &mut |from, to| {
            calls += 1;
            if calls == 2 || (fail_recovery && calls == 3) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            fs::rename(from, to)
        });
        if fail_recovery {
            assert!(matches!(result, Err(Error::PublicationRecovery { .. })));
            assert!(!target.exists());
            let backup = fs::read_dir(&cache)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".everlast-text-replaced-")
                })
                .ok_or("missing retained backup")?;
            assert_eq!(fs::read(backup.path().join("two.txt"))?, b"old");
        } else {
            assert!(matches!(result, Err(Error::Io(_))));
            assert_eq!(fs::read(target.join("two.txt"))?, b"old");
        }
    }
    Ok(())
}

#[test]
fn local_http_files_pass_through_the_real_bounded_transfer_before_publication() -> TestResult {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    listener.set_nonblocking(true)?;
    let server = std::thread::spawn(move || -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        for _ in 0..2 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => return Err(e),
                }
            };
            // Darwin inherits the listener's nonblocking flag. Request bytes
            // may arrive after accept, so use bounded blocking fixture I/O.
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(Duration::from_secs(1)))?;
            stream.set_write_timeout(Some(Duration::from_secs(1)))?;
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte)?;
                header.push(byte[0]);
                if header.len() > 4096 {
                    return Err(io::ErrorKind::InvalidData.into());
                }
            }
            stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc",
            )?;
        }
        Ok(())
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let client = super::super::download_client_builder()?
        .https_only(false)
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    let result = model.install_with(
        root.path(),
        &CancellationToken::new(),
        &mut |_, _| {},
        |_| Ok(u64::MAX),
        &mut |dir, asset, cancel, progress| {
            runtime.block_on(super::super::transfer(
                dir,
                asset.definition(),
                cancel,
                &mut |bytes| progress(bytes),
                &client,
                &format!("{url}/{}", asset.name),
            ))
        },
    );
    server.join().map_err(|_| "source fixture panicked")??;
    let installed = result?;
    assert_eq!(fs::read(installed.join("two.txt"))?, b"abc");
    Ok(())
}

#[test]
fn malformed_manifests_and_redirected_cache_roots_fail_without_modifying_targets() -> TestResult {
    for invalid in ["{}", r#"{"repository":"other/repo"}"#] {
        assert!(RefinementModel::parse(invalid).is_err());
    }
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir()?;
    let model = tiny()?;
    let external = tempfile::tempdir()?;
    fs::write(external.path().join("personal.txt"), b"keep")?;
    symlink(external.path(), root.path().join(CACHE_ROOT))?;
    assert!(matches!(
        model.inspect(root.path(), &CancellationToken::new()),
        Err(Error::InvalidCache)
    ));
    assert!(matches!(
        model.install_with(
            root.path(),
            &CancellationToken::new(),
            &mut |_, _| {},
            |_| Err(ModelError::Closed),
            &mut |_, _, _, _| Err(ModelError::Closed)
        ),
        Err(Error::InvalidCache)
    ));
    assert_eq!(fs::read(external.path().join("personal.txt"))?, b"keep");
    Ok(())
}
