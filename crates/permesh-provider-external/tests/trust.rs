// SPDX-License-Identifier: MIT
use permesh_provider_external::trust::{Registry, inspect};
use permesh_provider_sdk::Capability;
use std::{fs, path::PathBuf};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn setup() -> TestResult<(tempfile::TempDir, PathBuf, PathBuf)> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    let source = root.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    Ok((tmp, root.join("registry"), source))
}
#[test]
fn explicit_copy_is_immutable_and_create_only() -> TestResult {
    let (_tmp, root, source) = setup()?;
    let registry = Registry::new(root.clone())?;
    assert!(!root.exists());
    assert!(registry.list()?.is_empty());
    let digest = inspect(&source)?.sha256;
    let reg = registry.trust(&source, "example", &digest, &[Capability::Accounts])?;
    let installed = registry.verify(&reg)?;
    assert_ne!(installed, source);
    fs::write(source, b"replacement")?;
    assert!(registry.verify(&reg).is_ok());
    assert_eq!(registry.load("example")?.sha256, digest);
    assert_eq!(registry.list()?.len(), 1);
    assert!(
        registry
            .trust(&installed, "example", &digest, &[Capability::Accounts])
            .is_err()
    );
    registry.remove("example")?;
    assert!(registry.list()?.is_empty());
    Ok(())
}
#[test]
fn invalid_input_and_changed_installed_bytes_are_rejected() -> TestResult {
    let (_tmp, root, source) = setup()?;
    assert!(Registry::new(PathBuf::from("relative")).is_err());
    let registry = Registry::new(root)?;
    let digest = inspect(&source)?.sha256;
    for id in ["", "../escape", "1example", "foo/bar", "a b"] {
        assert!(registry.trust(&source, id, &digest, &[]).is_err());
    }
    assert!(
        registry
            .trust(&source, "example", &"0".repeat(64), &[])
            .is_err()
    );
    assert!(
        registry
            .trust(
                &source,
                "example",
                &digest,
                &[Capability::Accounts, Capability::Accounts]
            )
            .is_err()
    );
    let reg = registry.trust(&source, "example", &digest, &[])?;
    fs::write(registry.verify(&reg)?, b"changed")?;
    assert!(registry.verify(&reg).is_err());
    Ok(())
}
#[test]
fn native_only_and_bounded_input() -> TestResult {
    let (_tmp, _root, source) = setup()?;
    fs::write(&source, b"#!/bin/sh\nexit 0\n")?;
    assert!(inspect(&source).is_err());
    fs::OpenOptions::new()
        .write(true)
        .open(&source)?
        .set_len(128 * 1024 * 1024 + 1)?;
    assert!(inspect(&source).is_err());
    Ok(())
}
#[test]
fn malformed_and_oversized_manifests_and_changed_capabilities_are_rejected() -> TestResult {
    let (_tmp, root, source) = setup()?;
    let registry = Registry::new(root.clone())?;
    let digest = inspect(&source)?.sha256;
    let mut reg = registry.trust(&source, "example", &digest, &[])?;
    reg.capabilities.push(Capability::Accounts);
    assert!(registry.verify(&reg).is_err());
    let manifest = root.join("example/manifest.json");
    for bytes in [b"{}".to_vec(), vec![b' '; 16385], format!("{{\"schema\":1,\"id\":\"example\",\"sha256\":\"{digest}\",\"capabilities\":[],\"path\":\"/evil\"}}").into_bytes()] {
        fs::write(&manifest, bytes)?;
        assert!(registry.load("example").is_err());
    }
    Ok(())
}
#[cfg(unix)]
#[test]
fn symlinks_and_unknown_removal_entries_are_not_followed() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (_tmp, root, source) = setup()?;
    let alias = source.with_file_name("alias");
    symlink(&source, &alias)?;
    assert!(inspect(&alias).is_err());
    let registry = Registry::new(root.clone())?;
    let digest = inspect(&source)?.sha256;
    let reg = registry.trust(&source, "example", &digest, &[])?;
    assert_eq!(fs::metadata(&root)?.permissions().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(root.join("example/manifest.json"))?
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::write(root.join("example/unknown"), b"keep")?;
    assert!(registry.remove("example").is_err());
    assert!(root.join("example/unknown").exists());
    let binary = registry.verify(&reg)?;
    fs::remove_file(&binary)?;
    symlink(&source, &binary)?;
    assert!(registry.verify(&reg).is_err());
    assert!(registry.remove("example").is_err());
    assert!(source.exists());
    Ok(())
}

#[test]
fn manifest_schema_digest_ids_and_duplicate_capabilities_are_checked() -> TestResult {
    let (_tmp, root, source) = setup()?;
    let registry = Registry::new(root.clone())?;
    let digest = inspect(&source)?.sha256;
    let reg = registry.trust(&source, "example", &digest, &[])?;
    let valid = serde_json::to_value(&reg)?;
    for (key, value) in [
        ("schema", serde_json::json!(2)),
        ("sha256", serde_json::json!("invalid")),
        ("id", serde_json::json!("different")),
        ("capabilities", serde_json::json!(["accounts", "accounts"])),
        ("capabilities", serde_json::json!(["unknown"])),
    ] {
        let mut changed = valid.clone();
        changed[key] = value;
        fs::write(
            root.join("example/manifest.json"),
            serde_json::to_vec(&changed)?,
        )?;
        assert!(registry.load("example").is_err());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn insecure_root_and_symlinked_storage_components_are_rejected() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (_tmp, root, source) = setup()?;
    let registry = Registry::new(root.clone())?;
    let digest = inspect(&source)?.sha256;
    let reg = registry.trust(&source, "example", &digest, &[])?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755))?;
    assert!(Registry::new(root.clone()).is_err());
    assert!(registry.verify(&reg).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let moved = root.with_file_name("moved");
    fs::rename(&root, &moved)?;
    symlink(&moved, &root)?;
    assert!(registry.load("example").is_err());
    assert!(registry.list().is_err());
    assert!(registry.remove("example").is_err());
    assert!(Registry::new(root).is_err());
    assert!(moved.join("example/manifest.json").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn writable_nonsticky_ancestor_is_rejected_and_sticky_owner_is_supported() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let (_tmp, root, source) = setup()?;
    let shared = root.with_file_name("shared");
    fs::create_dir(&shared)?;
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777))?;
    assert!(Registry::new(shared.join("registry")).is_err());
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777))?;
    let registry = Registry::new(shared.join("registry"))?;
    let digest = inspect(&source)?.sha256;
    let reg = registry.trust(&source, "example", &digest, &[])?;
    assert!(registry.verify(&reg).is_ok());
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777))?;
    assert!(registry.verify(&reg).is_err());
    assert!(registry.remove("example").is_err());
    Ok(())
}
