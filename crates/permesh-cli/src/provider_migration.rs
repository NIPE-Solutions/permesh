// SPDX-License-Identifier: MIT
//! Explicit local migration; this module never invokes a provider or resolves credentials.
use crate::{
    args::Cli, blocking::BlockingPool, cancellation::Cancellation, error::AppError, report::Outcome,
};
use clap::Args;
use permesh_config::{Config, ExternalConfig, ProviderKind};
use permesh_provider_external::trust::Registry;
use permesh_provider_sdk::Capability;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Args)]
pub struct MigrationArgs {
    /// Existing legacy GitHub instance to migrate, preserving its ID and references.
    #[arg(value_name = "INSTANCE")]
    pub id: String,
    /// Exact SHA-256 of an already trusted external github binary.
    #[arg(long, value_name = "DIGEST")]
    pub sha256: String,
}
const CAPABILITIES: [Capability; 5] = [
    Capability::Accounts,
    Capability::Resources,
    Capability::Groups,
    Capability::Memberships,
    Capability::Grants,
];
fn cancelled() -> AppError {
    AppError::new(130, "Cancelled")
}
pub async fn run(
    cli: &Cli,
    args: &MigrationArgs,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    let draft = Draft::load(&crate::workspace::path(cli)?, args)?;
    let registry =
        Registry::new(crate::external::storage_root()?).map_err(crate::external::failure)?;
    let digest = args.sha256.clone();
    tokio::select! {
        biased;
        ()=cancellation.cancelled()=>return Err(cancelled()),
        result=tokio::time::timeout(Duration::from_secs(120),pool.run(move|| {
            let registration=registry.load_pinned("github",&digest).map_err(crate::external::failure)?;
            if registration.capabilities.len()!=CAPABILITIES.len() || !CAPABILITIES.iter().all(|cap|registration.capabilities.contains(cap)) {
                return Err(AppError::input("Migration requires the trusted GitHub capabilities: accounts, resources, groups, memberships and grants"));
            }
            registry.verify(&registration).map_err(crate::external::failure)?;
            Ok::<_,AppError>(())
        }))=>result.map_err(|_|AppError::new(3,"Provider registration verification timed out"))???,
    }
    // The only worker performed read-only verification. The final local commit
    // has no suspension point and cannot publish after a cancelled caller returns.
    draft.commit(args, cancellation)
}
struct Draft {
    path: PathBuf,
    original: Vec<u8>,
    config: Config,
}
fn source(path: &Path) -> Result<Vec<u8>, AppError> {
    if !std::fs::symlink_metadata(path)
        .map_err(|_| AppError::input("Cannot inspect workspace configuration"))?
        .is_file()
    {
        return Err(AppError::input(
            "Migration requires a regular workspace file; symlink replacement is not supported",
        ));
    }
    let file =
        File::open(path).map_err(|_| AppError::input("Cannot read workspace configuration"))?;
    if !file
        .metadata()
        .map_err(|_| AppError::input("Cannot inspect workspace configuration"))?
        .is_file()
    {
        return Err(AppError::input(
            "Migration requires a regular workspace file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(permesh_config::MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::input("Cannot read workspace configuration"))?;
    if bytes.len() > permesh_config::MAX_CONFIG_BYTES {
        return Err(AppError::input("Workspace configuration exceeds 1 MiB"));
    }
    Ok(bytes)
}
impl Draft {
    fn load(path: &Path, args: &MigrationArgs) -> Result<Self, AppError> {
        let original = source(path)?;
        let mut config = Config::from_bytes(&original)?;
        let provider = config
            .providers
            .iter_mut()
            .find(|provider| provider.id == args.id)
            .ok_or_else(|| {
                AppError::input("Unknown provider instance; run permesh provider list")
            })?;
        if provider.kind != ProviderKind::Github {
            return Err(AppError::input(
                "Migration requires an existing legacy type: github instance; external instances are already migrated",
            ));
        }
        if args.id.len() > 64
            || !args
                .id
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
        {
            return Err(AppError::input(
                "External instance IDs must start with an ASCII letter and contain at most 64 bytes. Rename this legacy ID and its aliases/keychain references explicitly before migration",
            ));
        }
        if provider.organizations.len() > 100
            || provider.organizations.iter().any(|org| org.len() > 100)
        {
            return Err(AppError::input(
                "External GitHub supports at most 100 organizations with names at most 100 bytes; edit the legacy configuration explicitly before migration",
            ));
        }
        let token = provider
            .auth
            .take()
            .ok_or_else(|| AppError::input("Legacy GitHub token reference is missing"))?
            .token;
        let organizations = std::mem::take(&mut provider.organizations);
        provider.kind = ProviderKind::External;
        provider.customer_id = None;
        provider.external = Some(ExternalConfig {
            provider: "github".into(),
            sha256: args.sha256.clone(),
            configuration: BTreeMap::from([(
                "organizations".into(),
                serde_json::json!(organizations),
            )]),
            credentials: BTreeMap::from([("token".into(), token)]),
        });
        config.validate()?;
        let path = path
            .canonicalize()
            .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
        Ok(Self {
            path,
            original,
            config,
        })
    }
    fn commit(
        self,
        args: &MigrationArgs,
        cancellation: &Cancellation,
    ) -> Result<Outcome, AppError> {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let yaml = permesh_config::to_yaml(&self.config)?;
        Config::from_bytes(yaml.as_bytes())?;
        let outcome = Outcome::new(
            "provider_migrate",
            serde_json::json!({
                "id":args.id,"provider":"github","sha256":args.sha256,"file":self.path,"changed":true,
                "trust_changed":false,"approval_records_changed":false,"execution_approvals_require_review":true,"credentials_resolved":false,
                "message":"Migrated GitHub instance; ID, organizations, aliases and token reference were preserved. Configuration formatting was normalized. Review the diff. This workspace change invalidates execution approvals for all external instances; review and approve each before querying. No provider was executed and no credentials were resolved or stored.",
                "next":format!("permesh provider external review {}",args.id)
            }),
        )?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::input("Cannot locate workspace directory"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| AppError::input("Cannot prepare workspace update"))?;
        temporary
            .write_all(yaml.as_bytes())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| AppError::new(5, "Cannot write workspace update"))?;
        if source(&self.path)? != self.original {
            return Err(AppError::input(
                "Workspace changed during migration; no configuration was replaced. Run migration again using the current configuration",
            ));
        }
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        temporary
            .persist(&self.path)
            .map_err(|_| AppError::new(5, "Cannot replace workspace configuration"))?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn changed_workspace_and_cancellation_never_publish_prepared_migration() -> TestResult {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("permesh.yaml");
        let original=b"version: 1\norganization: {name: Acme}\nproviders:\n- id: github-main\n  type: github\n  organizations: [acme]\n  auth: {token: env://MISSING}\n";
        let args = MigrationArgs {
            id: "github-main".into(),
            sha256: "a".repeat(64),
        };
        std::fs::write(&path, original)?;
        let draft = Draft::load(&path, &args).map_err(|error| error.message)?;
        let changed = b"# newer configuration\n";
        std::fs::write(&path, changed)?;
        assert!(
            draft
                .commit(&args, &Cancellation::new())
                .is_err_and(|e| e.message.contains("changed"))
        );
        assert_eq!(std::fs::read(&path)?, changed);
        std::fs::write(&path, original)?;
        let draft = Draft::load(&path, &args).map_err(|error| error.message)?;
        let cancellation = Cancellation::new();
        cancellation.cancel();
        assert!(
            draft
                .commit(&args, &cancellation)
                .is_err_and(|e| e.code == 130)
        );
        assert_eq!(std::fs::read(&path)?, original);
        assert_eq!(std::fs::read_dir(tmp.path())?.count(), 1);
        Ok(())
    }
}
