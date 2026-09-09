// SPDX-License-Identifier: MIT
use crate::{
    error::AppError,
    external::{failure, storage_root},
    provider_diagnostics::Code,
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

#[path = "external_network.rs"]
mod network;

pub(crate) struct WorkspaceAccess {
    pub root: PathBuf,
}
fn workspace_path(path: &Path) -> Result<PathBuf, AppError> {
    path.canonicalize()
        .map_err(|_| AppError::input("Cannot locate the workspace configuration file"))
}
fn external_config(provider: &ProviderConfig) -> Result<&ExternalConfig, AppError> {
    if provider.kind != ProviderKind::External {
        return Err(
            AppError::input("This action requires an external provider instance")
                .diagnostic(Code::InvalidConfiguration),
        );
    }
    provider.external.as_ref().ok_or_else(|| {
        AppError::input("External provider configuration is missing")
            .diagnostic(Code::InvalidConfiguration)
    })
}
fn selected<'a>(config: &'a Config, id: &str) -> Result<&'a ProviderConfig, AppError> {
    config
        .validate()
        .map_err(|error| AppError::from(error).diagnostic(Code::InvalidConfiguration))?;
    config
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::input("Unknown provider instance; run permesh provider list"))
}
fn approval_required() -> AppError {
    AppError::input(
        "Workspace external provider approval is missing or stale; run permesh provider external review, then approve its fingerprint with --accept-risk",
    ).diagnostic(Code::ApprovalMissingOrStale)
}
fn trust_failure(error: permesh_provider_external::ExternalError) -> AppError {
    let code = if matches!(error, permesh_provider_external::ExternalError::Storage) {
        Code::StorageUnavailable
    } else {
        Code::BinaryUntrustedOrChanged
    };
    failure(error).diagnostic(code)
}

struct PreparedContext {
    executable: PathBuf,
    registration: Registration,
    external: ExternalConfig,
    network: Option<permesh_provider_sdk::network::NetworkContext>,
    resolvers: BTreeMap<String, crate::remote_secrets::Prepared>,
}
impl PreparedContext {
    fn finish(
        self,
        credentials: BTreeMap<String, permesh_secrets::Secret>,
    ) -> Result<(PathBuf, Registration, Invocation), AppError> {
        let invocation = Invocation::new(self.external.configuration, credentials)
            .map_err(failure)?
            .with_executable_pin(&self.registration.sha256)
            .map_err(failure)?;
        let invocation = if let Some(network) = self.network {
            invocation.with_network(network).map_err(failure)?
        } else {
            invocation
        };
        Ok((self.executable, self.registration, invocation))
    }
}
fn resolver_contexts(
    external: &ExternalConfig,
    path: &Path,
    instance: &str,
) -> Result<BTreeMap<String, crate::remote_secrets::Prepared>, AppError> {
    external
        .credential_resolvers
        .iter()
        .map(|(name, resolver)| {
            let network = resolver
                .network()
                .map(|settings| network::load(settings, path))
                .transpose()?;
            let prepared =
                crate::remote_secrets::Prepared::new(resolver, instance, network.as_ref())?;
            Ok((name.clone(), prepared))
        })
        .collect()
}
impl WorkspaceAccess {
    fn approvals(&self) -> Result<ApprovalStore, AppError> {
        let parent = self
            .root
            .parent()
            .ok_or_else(|| AppError::input("Cannot locate workspace approval storage"))?;
        ApprovalStore::new(parent.join("workspace-approvals"))
            .map_err(|error| failure(error).diagnostic(Code::StorageUnavailable))
    }
    fn registration(
        &self,
        provider: &ProviderConfig,
    ) -> Result<(Registry, Registration), AppError> {
        let external = external_config(provider)?;
        let digest = external
            .digest_for_target(permesh_provider_sdk::target::native_target())
            .map_err(|error| AppError::from(error).diagnostic(Code::TargetPinUnavailable))?;
        let registry = Registry::new(self.root.clone()).map_err(trust_failure)?;
        let registration = registry
            .load_pinned(&external.provider, digest)
            .map_err(trust_failure)?;
        if registration.sha256 != digest {
            return Err(AppError::input(
                "External provider digest pin does not match its local registration; review and update the explicit pin",
            ).diagnostic(Code::BinaryUntrustedOrChanged));
        }
        Ok((registry, registration))
    }
    pub(crate) fn reviewed(
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
            )
            .diagnostic(Code::InvalidConfiguration));
        }
        let executable = registry.verify(&registration).map_err(trust_failure)?;
        let path = workspace_path(path)?;
        if let Some(settings) = &external_config(provider)?.network {
            network::load(settings, &path)
                .map_err(|error| error.diagnostic(Code::NetworkContextInvalid))?;
        }
        // Validate every approved resolver transport before any bootstrap can be touched.
        let _ = resolver_contexts(external_config(provider)?, &path, id)?;
        // Only reviewed inputs that affect this instance belong in its approval.
        // Keep relevant identity mappings/authority because they change how its
        // observations are interpreted, even though they are not sent to the child.
        let aliases: BTreeMap<_, _> = config
            .identity
            .aliases
            .iter()
            .filter_map(|(identity, providers)| {
                providers.get(id).map(|accounts| (identity, accounts))
            })
            .collect();
        let sources: Vec<_> = config
            .identity
            .sources
            .iter()
            .filter(|source| source.provider == id)
            .collect();
        let mut context = serde_json::json!({
            "approval_context": "permesh-provider-context-v2",
            "workspace_schema": config.version,
            "provider": provider,
            "identity_sources": sources,
            "identity_aliases": aliases,
        });
        if external_config(provider)?.sha256_by_target.is_some() {
            // Resolve only from the running CLI target. Never derive it from a
            // mutable selected registration or from a foreign map entry.
            context["resolved_target"] =
                serde_json::json!(permesh_provider_sdk::target::native_target());
            context["resolved_sha256"] = serde_json::json!(registration.sha256);
        }
        let fingerprint = fingerprint(&path, id, &context, &registration).map_err(failure)?;
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
        let mut result = serde_json::json!({
            "workspace":path, "instance":id, "registration":crate::schema1_control::Registration::from(&registration),
            "fingerprint":fingerprint, "approved":approved,
            "configuration":external.configuration, "credential_references":external.credentials,
            "identity":crate::schema1_control::IdentityConfig::from(&config.identity),
            "message":"Review binds this provider instance, its credential references, relevant identity aliases and authority, and its registered binary. Unrelated provider and organization edits do not invalidate approval. No credentials were resolved and no code was executed. Approval allows this trusted native code to execute with your user privileges and receive the named credentials; it is not sandboxed."
        });
        // Omit the default to preserve the existing control-output contract.
        // A negotiated context must be visible before the user approves it.
        if external.discovery_protocol == permesh_config::DiscoveryProtocol::NegotiatedV1 {
            result["discovery_protocol"] = serde_json::json!("negotiated_v1");
        }
        if let Some(profile) = &external.aws_profile {
            result["aws_profile"] = serde_json::to_value(crate::aws_profile::Review::from(profile))
                .map_err(|_| AppError::input("Cannot format AWS profile review"))?;
        }
        if !external.credential_resolvers.is_empty() {
            result["credential_resolvers"] = serde_json::to_value(
                external
                    .credential_resolvers
                    .iter()
                    .map(|(name, resolver)| (name, ResolverReview::from(resolver)))
                    .collect::<BTreeMap<_, _>>(),
            )
            .map_err(|_| AppError::input("Cannot format credential resolver review"))?;
        }
        if let Some(network) = &external.network {
            result["network"] = serde_json::to_value(network::NetworkReview::from(network))
                .map_err(|_| AppError::input("Cannot format external network settings"))?;
        }
        if let Some(pins) = &external.sha256_by_target {
            let target = permesh_provider_sdk::target::native_target().ok_or_else(|| {
                AppError::input("No supported native target for portable provider pins")
            })?;
            result["target_pins"] =
                serde_json::to_value(crate::external_output::TargetPinsReview {
                    sha256_by_target: pins,
                    resolved_target: target,
                    resolved_sha256: &registration.sha256,
                })
                .map_err(|_| AppError::input("Cannot format portable provider pins"))?;
        }
        Outcome::new("external_review", result)
    }
    pub(crate) fn approve(
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
            serde_json::json!({"approval":crate::schema1_control::Approval::from(&approval),"message":"Workspace instance approved for this exact reviewed configuration and registered binary. Changes to this provider context require a new approval."}),
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
    fn authorize(
        &self,
        config: &Config,
        path: &Path,
        id: &str,
    ) -> Result<(PathBuf, Registration), AppError> {
        let (executable, registration, fingerprint) = self.reviewed(config, path, id)?;
        self.approvals()?
            .verify(&workspace_path(path)?, id, &fingerprint)
            .map_err(|_| approval_required())?;
        Ok((executable, registration))
    }
    fn prepare_context(
        &self,
        config: &Config,
        path: &Path,
        provider: &ProviderConfig,
    ) -> Result<PreparedContext, AppError> {
        let actual = selected(config, &provider.id)?;
        if serde_json::to_value(actual)
            .map_err(|_| AppError::input("Cannot validate provider configuration"))?
            != serde_json::to_value(provider)
                .map_err(|_| AppError::input("Cannot validate provider configuration"))?
        {
            return Err(approval_required());
        }
        let (executable, registration) = self.authorize(config, path, &provider.id)?;
        let external = external_config(actual)?;
        let path = workspace_path(path)?;
        let network = external
            .network
            .as_ref()
            .map(|settings| {
                network::load(settings, &path)
                    .map_err(|e| e.diagnostic(Code::NetworkContextInvalid))
            })
            .transpose()?;
        let resolvers = resolver_contexts(external, &path, &provider.id)?;
        Ok(PreparedContext {
            executable,
            registration,
            external: external.clone(),
            network,
            resolvers,
        })
    }
    #[cfg(test)]
    fn prepare(
        &self,
        config: &Config,
        path: &Path,
        provider: &ProviderConfig,
    ) -> Result<(PathBuf, Registration, Invocation), AppError> {
        let context = self.prepare_context(config, path, provider)?;
        let mut credentials = if let Some(profile) = &context.external.aws_profile {
            crate::aws_profile::load(Path::new(&profile.credentials_file), &profile.profile)?
                .credentials()
        } else {
            BTreeMap::new()
        };
        for (name, value) in &context.external.credentials {
            let reference = SecretRef::parse(value)
                .map_err(|_| AppError::input("Invalid external credential reference"))?;
            let secret=SecretResolver.resolve(&reference).map_err(|_|AppError::new(3,"External credential unavailable; set its configured environment variable or use auth login for the named slot").diagnostic(Code::CredentialUnavailable))?;
            credentials.insert(name.clone(), secret);
        }
        context.finish(credentials)
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
pub(crate) async fn prepare_async(
    config: &Config,
    path: &Path,
    provider: &ProviderConfig,
    pool: &crate::blocking::BlockingPool,
    cancel: &crate::cancellation::Cancellation,
) -> Result<(PathBuf, Registration, Invocation), AppError> {
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    let config = config.clone();
    let path = path.to_owned();
    let provider = provider.clone();
    let context = pool
        .run(move || {
            WorkspaceAccess {
                root: storage_root()?,
            }
            .prepare_context(&config, &path, &provider)
        })
        .await??;
    let mut credentials = if let Some(profile) = context.external.aws_profile.clone() {
        if cancel.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        pool.run(move || {
            crate::aws_profile::load(Path::new(&profile.credentials_file), &profile.profile)
                .map(crate::aws_profile::Session::credentials)
        })
        .await??
    } else {
        BTreeMap::new()
    };
    let mut expiries = Vec::new();
    for (name, value) in &context.external.credentials {
        if cancel.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        let reference = SecretRef::parse(value)
            .map_err(|_| AppError::input("Invalid external credential reference"))?;
        let secret = if let SecretRef::Remote { name, .. } = &reference {
            let resolver = context
                .resolvers
                .get(name)
                .ok_or_else(|| AppError::input("Missing approved credential resolver"))?
                .clone();
            let resolved = resolver.resolve(pool, cancel).await?;
            if resolved.version == 0 {
                return Err(AppError::new(
                    3,
                    "Remote credential version metadata is invalid",
                ));
            }
            if let Some(expiry) = resolved.expires_at {
                expiries.push(expiry);
            }
            resolved.secret
        } else {
            pool.run(move||SecretResolver.resolve(&reference)).await?.map_err(|_|AppError::new(3,"External credential unavailable; set its explicit environment or keychain reference").diagnostic(Code::CredentialUnavailable))?
        };
        credentials.insert(name.clone(), secret);
    }
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    if expiries
        .iter()
        .any(|expiry| *expiry <= time::OffsetDateTime::now_utc())
    {
        return Err(
            AppError::new(3, "Remote credential expired before provider invocation")
                .diagnostic(Code::CredentialUnavailable),
        );
    }
    context.finish(credentials)
}
/// Verify binary and the provider-scoped workspace approval without resolving credentials.
pub(crate) fn authorize(
    config: &Config,
    path: &Path,
    id: &str,
) -> Result<(PathBuf, Registration), AppError> {
    WorkspaceAccess {
        root: storage_root()?,
    }
    .authorize(config, path, id)
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
        // These tests inspect/copy bytes and prepare invocations; they never
        // execute this file. A tiny native-magic fixture keeps approval tests
        // independent of debug test executables exceeding the 128 MiB limit.
        std::fs::write(&source, b"\x7fELFpermesh approval fixture")?;
        let sha256 = inspect(&source)?.sha256;
        let root = base.join("data/providers");
        Registry::new(root.clone())?.trust(&source, "fixture", &sha256, capabilities)?;
        let config: Config = serde_json::from_value(serde_json::json!({
            "version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":sha256,"configuration":{"endpoint":"example"},"credentials":{"token":"env://PERMESH_APPROVAL_TEST_UNAVAILABLE_CREDENTIAL_193756"}}}]
        }))?;
        Ok((temporary, WorkspaceAccess { root }, config, path))
    }
    #[test]
    fn remote_resolvers_are_reviewed_and_approved_before_bootstrap_and_changes_invalidate()
    -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[])?;
        let external = config.providers[0].external.as_mut().ok_or("external")?;
        external
            .credentials
            .insert("token".into(), "vault://named".into());
        external.credential_resolvers.insert("named".into(),serde_json::from_value(serde_json::json!({"version":1,"type":"vault_kv2","origin":"https://unreachable.invalid","mount":"secret","path":"providers/api","field":"token","bootstrap":"env://PERMESH_ABSENT_BOOTSTRAP_937812"}))?);
        let missing = access
            .prepare_context(&config, &path, &config.providers[0])
            .err()
            .ok_or("approval should be required")?;
        assert_eq!(missing.diagnostic, Some(Code::ApprovalMissingOrStale));
        let review = access
            .review(&config, &path, "instance")
            .map_err(|e| e.message)?;
        assert_eq!(
            review.report.result["credential_resolvers"]["named"]["origin"],
            "https://unreachable.invalid"
        );
        assert_eq!(
            review.report.result["credential_resolvers"]["named"]["bootstrap_reference"],
            "env://PERMESH_ABSENT_BOOTSTRAP_937812"
        );
        let fingerprint = review.report.result["fingerprint"]
            .as_str()
            .ok_or("fingerprint")?;
        access
            .approve(&config, &path, "instance", fingerprint, true)
            .map_err(|e| e.message)?;
        // Preparing the reviewed transport does not read the deliberately absent bootstrap.
        let ready = access
            .prepare_context(&config, &path, &config.providers[0])
            .map_err(|e| e.message)?;
        assert_eq!(ready.resolvers.len(), 1);
        for field in ["origin", "path", "field", "bootstrap", "secret_version"] {
            let mut changed = config.clone();
            let ext = changed.providers[0].external.as_mut().ok_or("external")?;
            let mut declaration = serde_json::to_value(&ext.credential_resolvers["named"])?;
            declaration[field] = match field {
                "origin" => serde_json::json!("https://another.invalid"),
                "secret_version" => serde_json::json!(2),
                "bootstrap" => serde_json::json!("env://OTHER_BOOTSTRAP"),
                _ => serde_json::json!("different"),
            };
            ext.credential_resolvers
                .insert("named".into(), serde_json::from_value(declaration)?);
            let stale = access
                .prepare_context(&changed, &path, &changed.providers[0])
                .err()
                .ok_or("changed resolver needs approval")?;
            assert_eq!(stale.diagnostic, Some(Code::ApprovalMissingOrStale));
        }
        Ok(())
    }
    #[test]
    fn network_ca_pin_is_reviewed_and_rechecked_before_secret_resolution() -> TestResult {
        use sha2::{Digest, Sha256};
        let (_temporary, access, mut config, path) = setup(&[])?;
        let pem = "-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n";
        let ca_path = path.with_file_name("ca.pem");
        std::fs::write(&ca_path, pem)?;
        let external = config.providers[0].external.as_mut().ok_or("external")?;
        external.discovery_protocol = permesh_config::DiscoveryProtocol::NegotiatedV1;
        external.network = Some(permesh_config::NetworkConfig {
            https_proxy: Some("http://proxy.example:8080".into()),
            no_proxy: vec!["example.test".into()],
            ca_bundle: Some(permesh_config::CaBundle {
                path: "ca.pem".into(),
                sha256: Sha256::digest(pem.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            }),
        });
        let error = access
            .prepare(&config, &path, &config.providers[0])
            .err()
            .ok_or("expected no approval")?;
        assert!(error.message.contains("approval"));
        assert_eq!(error.diagnostic, Some(Code::ApprovalMissingOrStale));
        let review = access
            .review(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let output = serde_json::to_string(&review.report.result)?;
        assert!(output.contains("ca.pem"));
        assert!(!output.contains("BEGIN CERTIFICATE"));
        let fingerprint = review.report.result["fingerprint"]
            .as_str()
            .ok_or("fingerprint")?;
        access
            .approve(&config, &path, "instance", fingerprint, true)
            .map_err(|e| e.message)?;
        let credential_error = access
            .prepare(&config, &path, &config.providers[0])
            .err()
            .ok_or("expected missing credential")?;
        assert!(credential_error.message.contains("credential unavailable"));
        assert_eq!(
            credential_error.diagnostic,
            Some(Code::CredentialUnavailable)
        );
        std::fs::write(&ca_path, pem.replace("YWJj", "ZGVm"))?;
        let changed = access
            .prepare(&config, &path, &config.providers[0])
            .err()
            .ok_or("expected pin rejection")?;
        assert!(changed.message.contains("CA bundle"));
        assert_eq!(changed.diagnostic, Some(Code::NetworkContextInvalid));
        assert!(!changed.message.contains("credential unavailable"));
        assert!(access.review(&config, &path, "instance").is_err());
        std::fs::write(&ca_path, pem)?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .network
            .as_mut()
            .ok_or("network")?
            .https_proxy = Some("http://other.example:8080".into());
        let changed = access
            .prepare(&config, &path, &config.providers[0])
            .err()
            .ok_or("expected stale approval")?;
        assert!(changed.message.contains("approval"));
        Ok(())
    }
    #[test]
    fn browser_authorization_requires_approval_but_never_resolves_credentials() -> TestResult {
        let (_temporary, access, config, path) = setup(&[])?;
        assert!(access.authorize(&config, &path, "instance").is_err());
        let review = access
            .review(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let fingerprint = review.report.result["fingerprint"]
            .as_str()
            .ok_or("fingerprint")?;
        access
            .approve(&config, &path, "instance", fingerprint, true)
            .map_err(|e| e.message)?;
        access
            .authorize(&config, &path, "instance")
            .map_err(|e| e.message)?;
        // The existing environment reference is deliberately unavailable.
        assert!(
            access
                .prepare(&config, &path, &config.providers[0])
                .is_err()
        );
        Ok(())
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
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .configuration
            .insert("endpoint".into(), serde_json::json!("changed"));
        assert!(
            access
                .prepare(&config, &path, &config.providers[0])
                .is_err()
        );
        access.revoke(&path, "instance").map_err(|e| e.message)?;
        Ok(())
    }
    #[test]
    fn approval_ignores_unrelated_providers_but_binds_instance_security_context() -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[Capability::Identities])?;
        let (_, _, original) = access
            .reviewed(&config, &path, "instance")
            .map_err(|e| e.message)?;
        access
            .approve(&config, &path, "instance", &original, true)
            .map_err(|e| e.message)?;
        config.organization.name = "Renamed".into();
        config.providers.push(ProviderConfig {
            inventory: None,
            id: "demo".into(),
            kind: ProviderKind::Demo,
            organizations: vec![],
            customer_id: None,
            auth: None,
            external: None,
        });
        config.identity.aliases.insert(
            "unrelated@example.com".into(),
            BTreeMap::from([("demo".into(), vec!["other".into()])]),
        );
        access
            .authorize(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let unchanged = config.clone();
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .credentials
            .insert("token".into(), "env://CHANGED_REFERENCE".into());
        assert!(access.authorize(&config, &path, "instance").is_err());
        config = unchanged.clone();
        config
            .identity
            .sources
            .push(permesh_config::IdentitySource {
                provider: "instance".into(),
                authoritative: true,
            });
        assert!(access.authorize(&config, &path, "instance").is_err());
        config = unchanged;
        config.identity.aliases.insert(
            "mapped@example.com".into(),
            BTreeMap::from([("instance".into(), vec!["native-id".into()])]),
        );
        assert!(access.authorize(&config, &path, "instance").is_err());
        Ok(())
    }
    #[test]
    fn stored_legacy_approval_cannot_authorize_or_resolve_credentials() -> TestResult {
        let (_temporary, access, config, path) = setup(&[])?;
        let (_, registration, _) = access
            .reviewed(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let legacy = fingerprint(&path, "instance", &config, &registration)?;
        access
            .approvals()
            .map_err(|e| e.message)?
            .approve(&path, "instance", &legacy)?;
        assert!(
            access
                .authorize(&config, &path, "instance")
                .is_err_and(|e| e.code == 2 && e.message.contains("approval"))
        );
        // The named environment credential is absent; approval must fail before
        // resolution could instead report credential-unavailable (code 3).
        assert!(
            access
                .prepare(&config, &path, &config.providers[0])
                .is_err_and(|e| e.code == 2 && e.message.contains("approval"))
        );
        assert_eq!(
            access
                .review(&config, &path, "instance")
                .map_err(|e| e.message)?
                .report
                .result["approved"],
            false
        );
        Ok(())
    }

    #[test]
    fn mixed_aliases_and_unrelated_authority_do_not_invalidate_selected_approval() -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[Capability::Identities])?;
        config.providers.push(ProviderConfig {
            inventory: None,
            id: "demo".into(),
            kind: ProviderKind::Demo,
            organizations: vec![],
            customer_id: None,
            auth: None,
            external: None,
        });
        config.identity.aliases.insert(
            "person@example.com".into(),
            BTreeMap::from([
                ("instance".into(), vec!["native-id".into()]),
                ("demo".into(), vec!["other-id".into()]),
            ]),
        );
        let (_, _, original) = access
            .reviewed(&config, &path, "instance")
            .map_err(|e| e.message)?;
        access
            .approve(&config, &path, "instance", &original, true)
            .map_err(|e| e.message)?;
        config
            .identity
            .sources
            .push(permesh_config::IdentitySource {
                provider: "demo".into(),
                authoritative: true,
            });
        config
            .identity
            .aliases
            .get_mut("person@example.com")
            .ok_or("alias")?
            .insert("demo".into(), vec!["changed-other-id".into()]);
        access
            .authorize(&config, &path, "instance")
            .map_err(|e| e.message)?;
        let unchanged = config.clone();
        config
            .identity
            .aliases
            .get_mut("person@example.com")
            .ok_or("alias")?
            .insert("instance".into(), vec!["changed-native-id".into()]);
        assert!(
            access
                .authorize(&config, &path, "instance")
                .is_err_and(|e| e.code == 2)
        );
        config = unchanged;
        config
            .identity
            .aliases
            .get_mut("person@example.com")
            .ok_or("alias")?
            .remove("instance");
        assert!(
            access
                .authorize(&config, &path, "instance")
                .is_err_and(|e| e.code == 2)
        );
        Ok(())
    }

    #[test]
    fn digest_pin_and_identity_source_capability_are_required() -> TestResult {
        let (_temporary, access, mut config, path) = setup(&[])?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .sha256 = Some("b".repeat(64));
        assert!(access.review(&config, &path, "instance").is_err());
        let reg = Registry::new(access.root.clone())?.load("fixture")?;
        config.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .sha256 = Some(reg.sha256);
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
    mod portable {
        include!("external_portable_tests.rs");
    }
}

/// Versioned CLI review fields independent of configuration serialization.
#[derive(serde::Serialize)]
struct ResolverReview<'a> {
    schema_version: u32,
    backend: &'static str,
    origin: &'a str,
    bootstrap_reference: &'a str,
    field: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    vault: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mount: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<network::NetworkReview<'a>>,
}
impl<'a> From<&'a permesh_config::CredentialResolver> for ResolverReview<'a> {
    fn from(value: &'a permesh_config::CredentialResolver) -> Self {
        use permesh_config::CredentialResolver::*;
        let (backend, vault, item, mount, path, secret_version) = match value {
            OnePasswordConnect { vault, item, .. } => (
                "1password_connect",
                Some(vault.as_str()),
                Some(item.as_str()),
                None,
                None,
                None,
            ),
            VaultKv2 {
                mount,
                path,
                secret_version,
                ..
            } => (
                "vault_kv2",
                None,
                None,
                Some(mount.as_str()),
                Some(path.as_str()),
                *secret_version,
            ),
            OpenBaoKv2 {
                mount,
                path,
                secret_version,
                ..
            } => (
                "openbao_kv2",
                None,
                None,
                Some(mount.as_str()),
                Some(path.as_str()),
                *secret_version,
            ),
        };
        Self {
            schema_version: 1,
            backend,
            origin: value.origin(),
            bootstrap_reference: value.bootstrap(),
            field: value.field(),
            vault,
            item,
            mount,
            path,
            secret_version,
            network: value.network().map(network::NetworkReview::from),
        }
    }
}
