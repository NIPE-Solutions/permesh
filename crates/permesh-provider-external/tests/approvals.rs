// SPDX-License-Identifier: MIT
use permesh_provider_external::{
    approvals::{ApprovalStore, fingerprint},
    trust::Registration,
};
use permesh_provider_sdk::Capability;
use serde_json::json;
use std::{fs, path::PathBuf};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn setup() -> TestResult<(tempfile::TempDir, PathBuf, PathBuf, Registration)> {
    let temp = tempfile::tempdir()?;
    let base = temp.path().canonicalize()?;
    let workspace = base.join("permesh.yaml");
    fs::write(&workspace, "schema: 1\n")?;
    let registration = Registration {
        schema: 1,
        id: "provider".into(),
        sha256: "a".repeat(64),
        capabilities: vec![Capability::Accounts],
    };
    Ok((temp, base.join("approvals"), workspace, registration))
}
#[test]
fn reads_are_inert_and_explicit_approval_can_be_replaced_and_revoked() -> TestResult {
    let (_temp, root, workspace, registration) = setup()?;
    let store = ApprovalStore::new(root.clone())?;
    assert!(store.get(&workspace, "instance")?.is_none());
    assert!(!root.exists());
    let first = fingerprint(
        &workspace,
        "instance",
        &json!({"aliases": []}),
        &registration,
    )?;
    assert!(store.verify(&workspace, "instance", &first).is_err());
    let approved = store.approve(&workspace, "instance", &first)?;
    assert_eq!(approved.fingerprint, first);
    store.verify(&workspace, "instance", &first)?;
    let next = fingerprint(
        &workspace,
        "instance",
        &json!({"aliases": ["changed"]}),
        &registration,
    )?;
    store.approve(&workspace, "instance", &next)?;
    assert!(store.verify(&workspace, "instance", &first).is_err());
    store.verify(&workspace, "instance", &next)?;
    store.revoke(&workspace, "instance")?;
    assert!(store.get(&workspace, "instance")?.is_none());
    store.revoke(&workspace, "instance")?;
    Ok(())
}
#[test]
fn fingerprint_binds_full_configuration_registration_instance_and_clone_path() -> TestResult {
    let (_temp, _root, workspace, registration) = setup()?;
    let config = json!({"aliases": [], "authority": "demo", "configuration": {"endpoint":"one"}, "credentials":{"token":"env://FIRST"}});
    let first = fingerprint(&workspace, "instance", &config, &registration)?;
    for (field, value) in [
        ("aliases", json!(["different"])),
        ("authority", json!("external")),
        ("configuration", json!({"endpoint":"two"})),
        ("credentials", json!({"token":"env://SECOND"})),
    ] {
        let mut changed = config.clone();
        changed[field] = value;
        assert_ne!(
            first,
            fingerprint(&workspace, "instance", &changed, &registration)?
        );
    }
    let clone = workspace.with_file_name("clone.yaml");
    fs::copy(&workspace, &clone)?;
    assert_ne!(
        first,
        fingerprint(&clone, "instance", &config, &registration)?
    );
    assert_ne!(
        first,
        fingerprint(&workspace, "other", &config, &registration)?
    );
    let mut changed = registration.clone();
    changed.sha256 = "b".repeat(64);
    assert_ne!(
        first,
        fingerprint(&workspace, "instance", &config, &changed)?
    );
    changed = registration.clone();
    changed.capabilities.push(Capability::Identities);
    assert_ne!(
        first,
        fingerprint(&workspace, "instance", &config, &changed)?
    );
    Ok(())
}
#[test]
fn invalid_or_oversized_inputs_fail_and_records_store_only_fingerprint() -> TestResult {
    let (_temp, root, workspace, registration) = setup()?;
    assert!(ApprovalStore::new(PathBuf::from("relative")).is_err());
    assert!(fingerprint(&workspace, "../escape", &json!({}), &registration).is_err());
    assert!(
        fingerprint(
            &workspace,
            "instance",
            &"x".repeat(1024 * 1024 + 1),
            &registration
        )
        .is_err()
    );
    let store = ApprovalStore::new(root.clone())?;
    assert!(store.approve(&workspace, "instance", "invalid").is_err());
    let digest = fingerprint(
        &workspace,
        "instance",
        &json!({"credentials":{"token":"env://NEVER_STORE_REFERENCE_OR_VALUE"}}),
        &registration,
    )?;
    store.approve(&workspace, "instance", &digest)?;
    let entry = fs::read_dir(&root)?.next().ok_or("missing approval")??;
    let bytes = fs::read(entry.path())?;
    assert!(!String::from_utf8_lossy(&bytes).contains("NEVER_STORE_REFERENCE_OR_VALUE"));
    for bytes in [
        b"{}".to_vec(),
        vec![b' '; 16385],
        br#"{"schema":1,"unknown":true}"#.to_vec(),
    ] {
        fs::write(entry.path(), bytes)?;
        assert!(store.get(&workspace, "instance").is_err());
    }
    store.revoke(&workspace, "instance")?;
    Ok(())
}
#[cfg(unix)]
#[test]
fn insecure_storage_and_symlink_records_are_rejected_without_following() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (_temp, root, workspace, registration) = setup()?;
    let store = ApprovalStore::new(root.clone())?;
    let digest = fingerprint(&workspace, "instance", &json!({}), &registration)?;
    store.approve(&workspace, "instance", &digest)?;
    let entry = fs::read_dir(&root)?.next().ok_or("missing approval")??;
    fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o644))?;
    assert!(store.verify(&workspace, "instance", &digest).is_err());
    fs::remove_file(entry.path())?;
    symlink(&workspace, entry.path())?;
    assert!(store.get(&workspace, "instance").is_err());
    assert!(store.revoke(&workspace, "instance").is_err());
    assert!(workspace.exists());
    Ok(())
}

#[test]
fn canonical_object_order_is_stable_and_record_tampering_is_rejected() -> TestResult {
    use std::collections::HashMap;
    let (_temp, root, workspace, registration) = setup()?;
    let mut first = HashMap::new();
    first.insert("a", 1);
    first.insert("b", 2);
    let mut second = HashMap::new();
    second.insert("b", 2);
    second.insert("a", 1);
    let digest = fingerprint(&workspace, "instance", &first, &registration)?;
    assert_eq!(
        digest,
        fingerprint(&workspace, "instance", &second, &registration)?
    );
    let store = ApprovalStore::new(root.clone())?;
    let approved = store.approve(&workspace, "instance", &digest)?;
    let entry = fs::read_dir(root)?.next().ok_or("missing approval")??;
    for (field, value) in [
        ("schema", json!(2)),
        ("instance", json!("other")),
        ("workspace", json!("/different/workspace")),
        ("fingerprint", json!("0".repeat(64))),
    ] {
        let mut record = serde_json::to_value(&approved)?;
        record[field] = value;
        fs::write(entry.path(), serde_json::to_vec(&record)?)?;
        assert!(store.verify(&workspace, "instance", &digest).is_err());
    }
    Ok(())
}

#[test]
fn concurrent_approvals_never_remove_another_writers_reservation() -> TestResult {
    use std::sync::{Arc, Barrier};
    let (_temp, root, workspace, _) = setup()?;
    let store = ApprovalStore::new(root)?;
    for _ in 0..16 {
        store.approve(&workspace, "instance", &"a".repeat(64))?;
        let ready = Arc::new(Barrier::new(16));
        let attempts: Vec<_> = (0..16)
            .map(|_| {
                let store = store.clone();
                let workspace = workspace.clone();
                let ready = ready.clone();
                std::thread::spawn(move || {
                    ready.wait();
                    store.approve(&workspace, "instance", &"b".repeat(64))
                })
            })
            .collect();
        let mut succeeded = false;
        for attempt in attempts {
            succeeded |= attempt
                .join()
                .map_err(|_| "approval thread panicked")?
                .is_ok();
        }
        assert!(succeeded, "one exclusive approval writer must finish");
        store.verify(&workspace, "instance", &"b".repeat(64))?;
    }
    Ok(())
}
