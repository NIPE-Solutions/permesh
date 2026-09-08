// SPDX-License-Identifier: MIT
use permesh_provider_external::trust::{Registry, inspect};
use permesh_provider_sdk::Capability;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const CAPS: [Capability; 5] = [
    Capability::Accounts,
    Capability::Resources,
    Capability::Groups,
    Capability::Memberships,
    Capability::Grants,
];
fn run(root: &Path, id: &str, digest: &str) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env_remove("PERMESH_MIGRATION_UNSET")
        .args(["provider", "migrate", id, "--sha256", digest, "--json"])
        .output()
}
fn config(root: &Path, id: &str, reference: &str) -> std::io::Result<()> {
    fs::write(
        root.join("permesh.yaml"),
        format!(
            "version: 1\norganization: {{name: Acme}}\nproviders:\n  - id: {id}\n    type: github\n    organizations: [Acme, second-org]\n    auth: {{token: '{reference}'}}\n  - id: demo\n    type: demo\nidentity:\n  sources: [{{provider: demo, authoritative: true}}]\n  aliases:\n    alice@example.com: {{{id}: ['123']}}\n"
        ),
    )
}
fn registration(
    root: &Path,
    caps: &[Capability],
) -> Result<(Registry, String), Box<dyn std::error::Error>> {
    let source = root.join("source");
    // Inspection accepts a native-shaped fixture; it cannot execute successfully.
    // Successful migration therefore cannot have handshaken with a provider.
    fs::write(&source, b"\x7fELFthis fixture must never execute")?;
    let digest = inspect(&source)?.sha256;
    let registry = Registry::new(root.join("state/providers"))?;
    registry.trust(&source, "github", &digest, caps)?;
    Ok((registry, digest))
}
#[test]
fn migration_preserves_identity_and_exact_references_without_execution_or_resolution() -> TestResult
{
    for reference in [
        "env://PERMESH_MIGRATION_UNSET",
        "keychain://github-work/token",
    ] {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().canonicalize()?;
        config(&root, "github-work", reference)?;
        let before = permesh_config::Config::load(&root.join("permesh.yaml"))?;
        let (registry, digest) = registration(&root, &CAPS)?;
        let out = run(&root, "github-work", &digest)?;
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(out.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        assert_eq!(report["command"], "provider_migrate");
        let after = permesh_config::Config::load(&root.join("permesh.yaml"))?;
        let migrated = &after.providers[0];
        assert_eq!(migrated.id, "github-work");
        assert_eq!(migrated.kind, permesh_config::ProviderKind::External);
        assert!(migrated.auth.is_none() && migrated.organizations.is_empty());
        let external = migrated.external.as_ref().ok_or("missing external")?;
        assert_eq!(external.provider, "github");
        assert_eq!(external.sha256, digest);
        assert_eq!(
            external.configuration["organizations"],
            serde_json::json!(["Acme", "second-org"])
        );
        assert_eq!(external.credentials["token"], reference);
        assert_eq!(
            serde_json::to_value(&after.identity)?,
            serde_json::to_value(&before.identity)?
        );
        assert_eq!(
            serde_json::to_value(&after.providers[1])?,
            serde_json::to_value(&before.providers[1])?
        );
        assert_eq!(registry.list()?.len(), 1);
        assert!(!root.join("state/workspace-approvals").exists());
        let bytes = fs::read(root.join("permesh.yaml"))?;
        assert_eq!(run(&root, "github-work", &digest)?.status.code(), Some(2));
        assert_eq!(fs::read(root.join("permesh.yaml"))?, bytes);
    }
    Ok(())
}

#[test]
fn rejected_migrations_leave_original_bytes_and_trust_unchanged() -> TestResult {
    for (id, caps) in [
        ("github-work", Vec::new()),
        ("1legacy", CAPS.to_vec()),
        ("_legacy", CAPS.to_vec()),
        (&"a".repeat(65), CAPS.to_vec()),
    ] {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().canonicalize()?;
        config(&root, id, "env://PERMESH_MIGRATION_UNSET")?;
        let (registry, digest) = registration(&root, &caps)?;
        let before = fs::read(root.join("permesh.yaml"))?;
        let selected = registry.load("github")?;
        let output = run(&root, id, &digest)?;
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(fs::read(root.join("permesh.yaml"))?, before);
        assert_eq!(registry.load("github")?, selected);
        assert!(!root.join("state/workspace-approvals").exists());
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    config(&root, "github-work", "env://PERMESH_MIGRATION_UNSET")?;
    let before = fs::read(root.join("permesh.yaml"))?;
    for (id, digest) in [
        ("missing", "a".repeat(64)),
        ("github-work", "a".repeat(64)),
        ("github-work", "invalid".into()),
        ("demo", "a".repeat(64)),
    ] {
        assert_eq!(run(&root, id, &digest)?.status.code(), Some(2));
        assert_eq!(fs::read(root.join("permesh.yaml"))?, before);
        assert!(!root.join("state").exists());
    }
    let (registry, digest) = registration(&root, &CAPS)?;
    fs::write(registry.verify(&registry.load("github")?)?, b"tampered")?;
    assert_eq!(run(&root, "github-work", &digest)?.status.code(), Some(2));
    assert_eq!(fs::read(root.join("permesh.yaml"))?, before);
    Ok(())
}
#[test]
fn retained_pin_is_used_and_all_workspace_approvals_require_review() -> TestResult {
    use permesh_provider_external::approvals::{ApprovalStore, fingerprint};
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    config(&root, "github-work", "env://PERMESH_MIGRATION_UNSET")?;
    let (registry, digest) = registration(&root, &CAPS)?;
    let old = registry.load("github")?;
    let path = root.join("permesh.yaml");
    let mut before = permesh_config::Config::load(&path)?;
    let external: permesh_config::ProviderConfig = serde_json::from_value(
        serde_json::json!({"id":"existing","type":"external","external":{"provider":"github","sha256":digest,"configuration":{"organizations":["other"]},"credentials":{"token":"env://PERMESH_MIGRATION_UNSET"}}}),
    )?;
    before.providers.push(external);
    fs::write(&path, permesh_config::to_yaml(&before)?)?;
    let approvals = ApprovalStore::new(root.join("state/workspace-approvals"))?;
    let original = fingerprint(&path, "existing", &before, &old)?;
    approvals.approve(&path, "existing", &original)?;
    let records: Vec<_> = fs::read_dir(root.join("state/workspace-approvals"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    let record = records.first().ok_or("no approval")?;
    let approval_bytes = fs::read(record)?;
    fs::write(root.join("source"), b"\x7fELFsecond inert native fixture")?;
    let latest = registry.trust(
        &root.join("source"),
        "github",
        &inspect(&root.join("source"))?.sha256,
        &CAPS,
    )?;
    let output = run(&root, "github-work", &digest)?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(registry.load("github")?, latest);
    let after = permesh_config::Config::load(&path)?;
    assert_eq!(
        after.providers[0]
            .external
            .as_ref()
            .ok_or("missing external")?
            .sha256,
        digest
    );
    assert_eq!(
        serde_json::to_value(&before.providers[2])?,
        serde_json::to_value(&after.providers[2])?
    );
    assert_eq!(fs::read(record)?, approval_bytes);
    assert!(
        approvals
            .verify(
                &path,
                "existing",
                &fingerprint(&path, "existing", &after, &old)?
            )
            .is_err()
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("all external instances"));
    Ok(())
}
#[test]
fn legacy_organization_bounds_and_unsafe_workspace_fail_before_writing() -> TestResult {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    let (_, digest) = registration(&root, &CAPS)?;
    for organizations in [
        vec!["a".repeat(101)],
        (0..101).map(|i| format!("org-{i}")).collect(),
    ] {
        config(&root, "github-work", "env://PERMESH_MIGRATION_UNSET")?;
        let path = root.join("permesh.yaml");
        let mut value = permesh_config::Config::load(&path)?;
        value.providers[0].organizations = organizations;
        fs::write(&path, permesh_config::to_yaml(&value)?)?;
        let before = fs::read(&path)?;
        assert_eq!(run(&root, "github-work", &digest)?.status.code(), Some(2));
        assert_eq!(fs::read(&path)?, before);
    }
    fs::write(
        root.join("permesh.yaml"),
        vec![b' '; permesh_config::MAX_CONFIG_BYTES + 1],
    )?;
    assert_eq!(run(&root, "github-work", &digest)?.status.code(), Some(2));
    #[cfg(unix)]
    {
        let target = root.join("real.yaml");
        fs::rename(root.join("permesh.yaml"), &target)?;
        std::os::unix::fs::symlink(&target, root.join("permesh.yaml"))?;
        assert_eq!(run(&root, "github-work", &digest)?.status.code(), Some(2));
        assert!(
            fs::symlink_metadata(root.join("permesh.yaml"))?
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::metadata(target)?.len(),
            permesh_config::MAX_CONFIG_BYTES as u64 + 1
        );
    }
    Ok(())
}

#[test]
fn google_migration_preserves_authority_customer_and_immutable_aliases() -> TestResult {
    for reference in [
        "env://PERMESH_MIGRATION_UNSET",
        "keychain://google-work/token",
    ] {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().canonicalize()?;
        let path = root.join("permesh.yaml");
        let original = serde_json::json!({
            "version":1,"organization":{"name":"Example"},
            "providers":[{"id":"google-work","type":"google","customer_id":"C01234567","auth":{"token":reference}}],
            "identity":{"sources":[{"provider":"google-work","authoritative":true}],
            "aliases":{"google:C01234567:123":{"google-work":["123"]}}}
        });
        fs::write(&path, serde_json::to_vec(&original)?)?;
        let registry = Registry::new(root.join("state/providers"))?;
        let fixture = root.join("inert-google");
        fs::write(&fixture, b"\x7fELFmust never execute")?;
        let digest = inspect(&fixture)?.sha256;
        registry.trust(
            &fixture,
            "google",
            &digest,
            &[Capability::Accounts, Capability::Identities],
        )?;
        let output = run(&root, "google-work", &digest)?;
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["result"]["provider"], "google");
        assert_eq!(report["result"]["credentials_resolved"], false);
        let after = permesh_config::Config::load(&path)?;
        assert_eq!(serde_json::to_value(&after.identity)?, original["identity"]);
        let provider = &after.providers[0];
        assert_eq!(provider.id, "google-work");
        assert!(provider.auth.is_none() && provider.customer_id.is_none());
        let external = provider.external.as_ref().ok_or("missing external")?;
        assert_eq!(external.provider, "google");
        assert_eq!(
            external.configuration,
            std::collections::BTreeMap::from([
                ("customer_id".into(), serde_json::json!("C01234567")),
                ("auth_mode".into(), serde_json::json!("access_token")),
            ])
        );
        assert_eq!(external.credentials["token"], reference);
        assert!(!root.join("state/workspace-approvals").exists());
    }
    Ok(())
}

#[test]
fn google_migration_rejects_wrong_provider_or_missing_identity_capability() -> TestResult {
    for (kind, capabilities) in [
        ("github", CAPS.to_vec()),
        ("google", vec![Capability::Accounts]),
    ] {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().canonicalize()?;
        let path = root.join("permesh.yaml");
        fs::write(&path, b"version: 1\norganization: {name: Example}\nproviders:\n- id: google-work\n  type: google\n  customer_id: C123\n  auth: {token: env://PERMESH_MIGRATION_UNSET}\n")?;
        let before = fs::read(&path)?;
        let fixture = root.join("inert");
        fs::write(&fixture, b"\x7fELFinert")?;
        let digest = inspect(&fixture)?.sha256;
        Registry::new(root.join("state/providers"))?.trust(
            &fixture,
            kind,
            &digest,
            &capabilities,
        )?;
        assert_eq!(run(&root, "google-work", &digest)?.status.code(), Some(2));
        assert_eq!(fs::read(&path)?, before);
    }
    Ok(())
}
