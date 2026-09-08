// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    error::AppError,
    external::{failure, storage_root},
    report::Outcome,
};
use permesh_config::{Config, ExternalConfig, ProviderConfig, ProviderKind};
use permesh_provider_external::{
    approvals::{ApprovalStore, fingerprint},
    host::Invocation,
    trust::{Registration, Registry},
};
use permesh_provider_sdk::{Capability, Metadata};
use permesh_secrets::{SecretRef, SecretResolver};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

struct WorkspaceAccess {
    root: PathBuf,
}
fn workspace_path(path: &Path) -> Result<PathBuf, AppError> {
    path.canonicalize()
        .map_err(|_| AppError::input("Cannot locate the workspace configuration file"))
}
fn external_config(provider: &ProviderConfig) -> Result<&ExternalConfig, AppError> {
    if provider.kind != ProviderKind::External {
        return Err(AppError::input(
            "This action requires an external provider instance",
        ));
    }
    provider
        .external
        .as_ref()
        .ok_or_else(|| AppError::input("External provider configuration is missing"))
}
fn selected<'a>(config: &'a Config, id: &str) -> Result<&'a ProviderConfig, AppError> {
    config.validate()?;
    config
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::input("Unknown provider instance; run permesh provider list"))
}
fn approval_required() -> AppError {
    AppError::input(
        "Workspace external provider approval is missing or stale; run permesh provider external review, then approve its fingerprint with --accept-risk",
    )
}
impl WorkspaceAccess {
    fn approvals(&self) -> Result<ApprovalStore, AppError> {
        let parent = self
            .root
            .parent()
            .ok_or_else(|| AppError::input("Cannot locate workspace approval storage"))?;
        ApprovalStore::new(parent.join("workspace-approvals")).map_err(failure)
    }
    fn registration(
        &self,
        provider: &ProviderConfig,
    ) -> Result<(Registry, Registration), AppError> {
        let external = external_config(provider)?;
        let registry = Registry::new(self.root.clone()).map_err(failure)?;
        let registration = registry.load(&external.provider).map_err(failure)?;
        if registration.sha256 != external.sha256 {
            return Err(AppError::input(
                "External provider digest pin does not match its local registration; review and update the explicit pin",
            ));
        }
        Ok((registry, registration))
    }
    fn reviewed(
        &self,
        config: &Config,
        path: &Path,
        id: &str,
    ) -> Result<(PathBuf, Registration, String), AppError> {
        let provider = selected(config, id)?;
        let (registry, registration) = self.registration(provider)?;
        if config
            .identity
            .sources
            .iter()
            .any(|source| source.provider == id)
            && !registration.capabilities.contains(&Capability::Identities)
        {
            return Err(AppError::input(
                "External identity sources require the registered identities capability",
            ));
        }
        let executable = registry.verify(&registration).map_err(failure)?;
        let path = workspace_path(path)?;
        let fingerprint = fingerprint(&path, id, config, &registration).map_err(failure)?;
        Ok((executable, registration, fingerprint))
    }
    fn review(&self, config: &Config, path: &Path, id: &str) -> Result<Outcome, AppError> {
        let (_, registration, fingerprint) = self.reviewed(config, path, id)?;
        let provider = selected(config, id)?;
        let external = external_config(provider)?;
        let path = workspace_path(path)?;
        let approved = self
            .approvals()?
            .get(&path, id)
            .map_err(failure)?
            .is_some_and(|record| record.fingerprint == fingerprint);
        Outcome::new(
            "external_review",
            serde_json::json!({
                "workspace":path, "instance":id, "registration":registration,
                "fingerprint":fingerprint, "approved":approved,
                "configuration":external.configuration, "credential_references":external.credentials,
                "identity":config.identity,
                "message":"Review binds the complete workspace configuration, including aliases and identity authority. No credentials were resolved and no code was executed. Approval allows this trusted native code to execute with your user privileges and receive the named credentials; it is not sandboxed."
            }),
        )
    }
    fn approve(
        &self,
        config: &Config,
        path: &Path,
        id: &str,
        expected_fingerprint: &str,
        accept_risk: bool,
    ) -> Result<Outcome, AppError> {
        if !accept_risk {
            return Err(AppError::input(
                "Workspace approval requires --accept-risk after reviewing the exact fingerprint and credential references",
            ));
        }
        let (_, _, fingerprint) = self.reviewed(config, path, id)?;
        if fingerprint != expected_fingerprint {
            return Err(AppError::input(
                "Workspace review fingerprint changed; review the current configuration before approval",
            ));
        }
        let path = workspace_path(path)?;
        let approval = self
            .approvals()?
            .approve(&path, id, &fingerprint)
            .map_err(failure)?;
        Outcome::new(
            "external_approve",
            serde_json::json!({"approval":approval,"message":"Workspace instance approved for this exact reviewed configuration and registered binary. Future configuration changes require a new approval."}),
        )
    }
    fn revoke(&self, path: &Path, id: &str) -> Result<Outcome, AppError> {
        let path = workspace_path(path)?;
        self.approvals()?.revoke(&path, id).map_err(failure)?;
        Outcome::new(
            "external_revoke",
            serde_json::json!({"workspace":path,"instance":id,"message":"Workspace execution approval revoked."}),
        )
    }
    fn prepare(
        &self,
        config: &Config,
        path: &Path,
        provider: &ProviderConfig,
    ) -> Result<(PathBuf, Registration, Invocation), AppError> {
        let actual = selected(config, &provider.id)?;
        // Bind the selected invocation to the same full configuration being
        // fingerprinted, even if a caller accidentally passes a stale clone.
        if serde_json::to_value(actual)
            .map_err(|_| AppError::input("Cannot validate provider configuration"))?
            != serde_json::to_value(provider)
                .map_err(|_| AppError::input("Cannot validate provider configuration"))?
        {
            return Err(approval_required());
        }
        let (executable, registration, fingerprint) = self.reviewed(config, path, &provider.id)?;
        let path = workspace_path(path)?;
        self.approvals()?
            .verify(&path, &provider.id, &fingerprint)
            .map_err(|_| approval_required())?;
        // This is deliberately the first credential-resolution point. Invalid,
        // cloned, revoked, or edited workspaces cannot touch env/keychain values.
        let external = external_config(actual)?;
        let mut credentials = BTreeMap::new();
        for (name, value) in &external.credentials {
            let reference = SecretRef::parse(value)
                .map_err(|_| AppError::input("Invalid external credential reference"))?;
            let secret = SecretResolver.resolve(&reference).map_err(|_| AppError::new(3,"External credential unavailable; set its configured environment variable or use auth login for the named slot"))?;
            credentials.insert(name.clone(), secret);
        }
        let invocation =
            Invocation::new(external.configuration.clone(), credentials).map_err(failure)?;
        Ok((executable, registration, invocation))
    }
}

pub(crate) fn review(config: &Config, path: &Path, id: &str) -> Result<Outcome, AppError> {
    WorkspaceAccess {
        root: storage_root()?,
    }
    .review(config, path, id)
}
pub(crate) fn approve(
    config: &Config,
    path: &Path,
    id: &str,
    fingerprint: &str,
    accept_risk: bool,
) -> Result<Outcome, AppError> {
    WorkspaceAccess {
        root: storage_root()?,
    }
    .approve(config, path, id, fingerprint, accept_risk)
}
pub(crate) fn revoke(path: &Path, id: &str) -> Result<Outcome, AppError> {
    WorkspaceAccess {
        root: storage_root()?,
    }
    .revoke(path, id)
}
pub(crate) fn prepare(
    config: &Config,
    path: &Path,
    provider: &ProviderConfig,
) -> Result<(PathBuf, Registration, Invocation), AppError> {
    WorkspaceAccess {
        root: storage_root()?,
    }
    .prepare(config, path, provider)
}
pub(crate) fn metadata(provider: &ProviderConfig) -> Result<Metadata, AppError> {
    let (_, registration) = WorkspaceAccess {
        root: storage_root()?,
    }
    .registration(provider)?;
    Ok(Metadata {
        kind: "external".into(),
        capabilities: registration.capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use permesh_provider_external::trust::inspect;
    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
    fn setup(
        capabilities: &[Capability],
    ) -> TestResult<(tempfile::TempDir, WorkspaceAccess, Config, PathBuf)> {
        let temporary = tempfile::tempdir()?;
        let base = temporary.path().canonicalize()?;
        let path = base.join("permesh.yaml");
        std::fs::write(&path, "workspace")?;
        let source = base.join("native");
        std::fs::copy(std::env::current_exe()?, &source)?;
        let sha256 = inspect(&source)?.sha256;
        let root = base.join("data/providers");
        Registry::new(root.clone())?.trust(&source, "fixture", &sha256, capabilities)?;
        let config: Config = serde_json::from_value(serde_json::json!({
            "version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":sha256,"configuration":{"endpoint":"example"},"credentials":{"token":"env://PERMESH_APPROVAL_TEST_UNAVAILABLE_CREDENTIAL_193756"}}}]
        }))?;
        Ok((temporary, WorkspaceAccess { root }, config, path))
    }
    #[test]
    fn missing_approval_blocks_before_credential_resolution_and_review_is_reference_only()
    -> TestResult {
        let (_temporary, access, config, path) = setup(&[])?;
        let failure = access
            .prepare(&config, &path, &config.providers[0])
            .err()
            .ok_or("unexpected preparation")?;
        assert_eq!(failure.code, 2);
        assert!(failure.message.contains("approval"));
        let reviewed = access
            .review(&config, &path, "instance")
            .map_err(|e| e.message)?;
        assert_eq!(reviewed.report.result["approved"], false);
        assert_eq!(
            reviewed.report.result["credential_references"]["token"],
            "env://PERMESH_APPROVAL_TEST_UNAVAILABLE_CREDENTIAL_193756"
        );
        assert!(
            !access
                .root
                .parent()
                .ok_or("parent")?
                .join("workspace-approvals")
                .exists()
        );
        Ok(())
    }
    #[test]
    fn approval_requires_exact_review_and_risk_acceptance_then_config_edits_fail() -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[])?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .credentials
            .clear();
        let reviewed = access
            .review(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let fingerprint = reviewed.report.result["fingerprint"]
            .as_str()
            .ok_or("fingerprint")?;
        assert!(
            access
                .approve(&config, &path, "instance", fingerprint, false)
                .is_err()
        );
        assert!(
            access
                .approve(&config, &path, "instance", &"0".repeat(64), true)
                .is_err()
        );
        access
            .approve(&config, &path, "instance", fingerprint, true)
            .map_err(|e| e.message)?;
        access
            .prepare(&config, &path, &config.providers[0])
            .map_err(|e| e.message)?;
        config.organization.name = "changed".into();
        assert!(
            access
                .prepare(&config, &path, &config.providers[0])
                .is_err()
        );
        access.revoke(&path, "instance").map_err(|e| e.message)?;
        Ok(())
    }
    #[test]
    fn digest_pin_and_identity_source_capability_are_required() -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[])?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .sha256 = "b".repeat(64);
        assert!(access.review(&config, &path, "instance").is_err());
        let reg = Registry::new(access.root.clone())?.load("fixture")?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .sha256 = reg.sha256;
        config
            .identity
            .sources
            .push(permesh_config::IdentitySource {
                provider: "instance".into(),
                authoritative: true,
            });
        assert!(access.review(&config, &path, "instance").is_err());
        Ok(())
    }
}
