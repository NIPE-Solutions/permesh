// SPDX-License-Identifier: MIT
use permesh_provider_external::trust::{Registry, inspect};
use permesh_provider_sdk::Capability;
use std::{fs, io::Write};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn updates_retain_legacy_pins_and_selection_is_explicit() -> TestResult {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?.join("registry");
    let source = tmp.path().canonicalize()?.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    let registry = Registry::new(root.clone())?;
    let first = registry.trust(
        &source,
        "example",
        &inspect(&source)?.sha256,
        &[Capability::Accounts],
    )?;
    let legacy = registry.verify(&first)?;
    let workspace = tmp.path().canonicalize()?.join("workspace.yaml");
    fs::write(&workspace, b"synthetic workspace")?;
    let configuration = serde_json::json!({"provider":"example","sha256":first.sha256});
    let fingerprint = permesh_provider_external::approvals::fingerprint(
        &workspace,
        "example-main",
        &configuration,
        &first,
    )?;
    let approvals = permesh_provider_external::approvals::ApprovalStore::new(
        tmp.path().canonicalize()?.join("approvals"),
    )?;
    approvals.approve(&workspace, "example-main", &fingerprint)?;

    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"second-version")?;
    let second = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(registry.load("example")?, second);
    assert_eq!(registry.list()?, vec![second.clone()]);
    assert_eq!(registry.load_pinned("example", &first.sha256)?, first);
    assert_eq!(registry.verify(&first)?, legacy);
    let retained = registry.load_pinned("example", &first.sha256)?;
    assert_eq!(
        permesh_provider_external::approvals::fingerprint(
            &workspace,
            "example-main",
            &configuration,
            &retained
        )?,
        fingerprint
    );
    approvals.verify(&workspace, "example-main", &fingerprint)?;

    let installed = registry.verify(&second)?;
    assert_eq!(
        installed,
        root.join("example/versions")
            .join(&second.sha256)
            .join("provider.exe")
    );
    assert!(
        registry
            .trust(&source, "example", &second.sha256, &[Capability::Groups])
            .is_err()
    );
    assert_eq!(registry.load("example")?, second);
    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"third-version")?;
    let third = registry.trust(
        &source,
        "example",
        &inspect(&source)?.sha256,
        &[Capability::Resources],
    )?;
    assert_eq!(registry.load("example")?, third);
    assert!(registry.verify(&first).is_ok());
    assert!(registry.verify(&second).is_ok());
    assert!(registry.verify(&third).is_ok());
    fs::write(&installed, b"changed")?;
    assert!(registry.verify(&second).is_err());
    assert!(registry.verify(&first).is_ok());
    registry.remove("example")?;
    assert!(registry.load_pinned("example", &first.sha256).is_err());
    assert!(registry.list()?.is_empty());
    Ok(())
}

#[test]
fn failed_update_and_stale_lock_preserve_previous_selection() -> TestResult {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?.join("registry");
    let source = tmp.path().canonicalize()?.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    let registry = Registry::new(root.clone())?;
    let first = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"second")?;
    let digest = inspect(&source)?.sha256;
    // A pre-existing selection reservation must never be deleted by this writer.
    let pending = root.join("example/selected.pending");
    // Link an already private file: copying can create an inherited, unprotected
    // Windows DACL, unlike the protected reservations produced by the registry.
    fs::hard_link(root.join("example/manifest.json"), &pending)?;
    assert!(registry.trust(&source, "example", &digest, &[]).is_err());
    assert!(pending.exists());
    assert!(!root.join("example/versions").join(&digest).exists());
    assert_eq!(registry.load("example")?, first);
    assert!(registry.verify(&first).is_ok());
    fs::remove_file(&pending)?;
    let lock = root.join(".registry.lock");
    fs::hard_link(root.join("example/manifest.json"), &lock)?;
    assert!(registry.trust(&source, "example", &digest, &[]).is_err());
    assert!(registry.remove("example").is_err());
    assert!(lock.exists());
    let listed = registry
        .list()
        .map_err(|error| format!("listing with private stale lock: {error}"))?;
    assert_eq!(listed, vec![first.clone()]);
    assert!(registry.verify(&first).is_ok());
    fs::remove_file(lock)?;
    assert!(registry.trust(&source, "example", &digest, &[]).is_ok());
    Ok(())
}
#[test]
fn unknown_retained_entries_prevent_any_removal_and_selected_tampering_fails_closed() -> TestResult
{
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?.join("registry");
    let source = tmp.path().canonicalize()?.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    let registry = Registry::new(root.clone())?;
    let first = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"second")?;
    let second = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    let installed = registry.verify(&second)?;
    let unknown = installed.with_file_name("unrecognized");
    fs::write(&unknown, b"keep")?;
    assert!(registry.remove("example").is_err());
    assert!(registry.verify(&first).is_ok());
    assert!(registry.verify(&second).is_ok());
    fs::remove_file(unknown)?;
    let mut changed = second.clone();
    changed.capabilities.push(Capability::Groups);
    fs::write(
        root.join("example/selected.json"),
        serde_json::to_vec(&changed)?,
    )?;
    assert!(registry.load("example").is_err());
    assert!(registry.verify(&second).is_ok());
    assert!(registry.verify(&changed).is_err());
    registry.remove("example")?;
    Ok(())
}
#[cfg(unix)]
#[test]
fn retained_version_symlinks_never_redirect_reads_or_removal() -> TestResult {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?.join("registry");
    let source = tmp.path().canonicalize()?.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    let registry = Registry::new(root.clone())?;
    let first = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"second")?;
    let second = registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    let version = root.join("example/versions").join(&second.sha256);
    let moved = tmp.path().join("retained");
    fs::rename(&version, &moved)?;
    symlink(&moved, &version)?;
    assert!(registry.load_pinned("example", &second.sha256).is_err());
    assert!(registry.verify(&second).is_err());
    assert!(registry.remove("example").is_err());
    assert!(registry.verify(&first).is_ok());
    assert!(moved.join("provider.exe").exists());
    Ok(())
}

#[test]
fn concurrent_trust_never_replaces_an_existing_digest_or_capabilities() -> TestResult {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?.join("registry");
    let source = tmp.path().canonicalize()?.join("source");
    fs::copy(std::env::current_exe()?, &source)?;
    let registry = Registry::new(root.clone())?;
    registry.trust(&source, "example", &inspect(&source)?.sha256, &[])?;
    fs::OpenOptions::new()
        .append(true)
        .open(&source)?
        .write_all(b"second")?;
    let digest = inspect(&source)?.sha256;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let workers: Vec<_> = [Capability::Groups, Capability::Accounts]
            .into_iter()
            .map(|cap| {
                let barrier = barrier.clone();
                let registry = &registry;
                let source = &source;
                let digest = &digest;
                scope.spawn(move || {
                    barrier.wait();
                    registry.trust(source, "example", digest, &[cap])
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join())
            .collect::<Vec<_>>()
    });
    let mut succeeded = Vec::new();
    for result in results {
        if let Ok(reg) = result.map_err(|_| "worker panicked")? {
            succeeded.push(reg);
        }
    }
    assert_eq!(succeeded.len(), 1);
    assert_eq!(registry.load("example")?, succeeded[0]);
    assert!(registry.verify(&succeeded[0]).is_ok());
    assert!(!root.join(".registry.lock").exists());
    Ok(())
}
