// SPDX-License-Identifier: MIT
use crate::{blocking::BlockingPool, cancellation::Cancellation, error::AppError, report::Outcome};
use clap::Args;
use permesh_provider_external::{
    DistributionError,
    catalog::{self, Catalog, Release},
    download,
    packages::{InstalledPackage, PackageStore},
};
use std::time::Duration;

#[derive(Clone, Args)]
pub struct InstallArgs {
    /// Official provider ID (for example, github).
    pub provider: String,
    /// Exact stable provider version to download; does not execute or trust code.
    #[arg(long)]
    pub version: String,
}
#[derive(Clone, Args)]
pub struct UpdateArgs {
    /// Previously installed official provider ID.
    pub provider: String,
    /// Inspect available versions without downloading a package or changing local state.
    #[arg(long)]
    pub check: bool,
    /// Download an exact stable version, including an intentional rollback.
    #[arg(long)]
    pub version: Option<String>,
}
fn failure(error: DistributionError) -> AppError {
    let code = match error {
        DistributionError::Input | DistributionError::Compatibility => 2,
        DistributionError::Storage => 5,
        _ => 3,
    };
    AppError::new(code, error.to_string())
}
fn store() -> Result<PackageStore, AppError> {
    let providers = crate::external::storage_root()?;
    let root = providers
        .parent()
        .ok_or_else(|| AppError::input("Cannot locate local package storage"))?
        .join("packages");
    PackageStore::new(root).map_err(failure)
}

pub(crate) struct Acquired {
    pub outcome: Outcome,
    pub package: Option<InstalledPackage>,
}
pub async fn run(
    provider: &str,
    version: Option<&str>,
    update: bool,
    check: bool,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    Ok(
        acquire(provider, version, update, check, false, pool, cancellation)
            .await?
            .outcome,
    )
}

pub(crate) async fn acquire(
    provider: &str,
    version: Option<&str>,
    update: bool,
    check: bool,
    setup: bool,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Acquired, AppError> {
    let started_at = crate::report::now()?;
    catalog::validate_request(provider, version).map_err(failure)?;
    let target = catalog::native_target().ok_or_else(|| {
        AppError::input("Official provider packages are not supported on this platform")
    })?;
    let store = store()?;
    let reader = self::store()?;
    let id = provider.to_owned();
    let mut installed = tokio::time::timeout(
        Duration::from_secs(120),
        pool.run(move || reader.list_provider(&id)),
    )
    .await
    .map_err(|_| AppError::new(5, "Local package inspection timed out"))??
    .map_err(failure)?;
    installed.retain(|package| package.release.target == target);
    installed.sort_by(|a, b| a.release.version.cmp(&b.release.version));
    if update && installed.is_empty() {
        return Err(AppError::input(
            "This provider has no installed package for this platform; use permesh provider install PROVIDER --version VERSION first",
        ));
    }
    let catalog = download::fetch_catalog().await.map_err(failure)?;
    let available = select_release(&catalog, provider, target, version, setup).map_err(failure)?;
    validate_known_versions(&catalog, &installed)?;
    let current = installed.last();
    let selected = if setup {
        available
    } else {
        selection(available, current, version.is_some())
    };
    let existing = installed.iter().find(|p| p.release == *selected);
    let changed = existing.is_none();
    let package = if check {
        None
    } else if let Some(package) = existing {
        Some(package.clone())
    } else {
        let bytes = download::fetch_archive(selected).await.map_err(failure)?;
        if cancellation.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        // Commit is bounded and synchronous: cancellation cannot leave a detached
        // worker publishing files after the command has returned.
        Some(store.install(selected, &bytes).map_err(failure)?)
    };
    let mut versions: Vec<_> = installed
        .iter()
        .map(|p| p.release.version.clone())
        .collect();
    if !check && changed {
        versions.push(selected.version.clone());
        versions.sort();
    }
    let result = serde_json::json!({
        "provider":provider,
        "target":target,
        "installed_versions":versions.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "available_version":available.version.to_string(),
        "selected":crate::schema1_control::Release::from(selected),
        "update_available":versions.last().is_some_and(|version| available.version > *version),
        "check_only":check,
        "changed":!check && changed,
        "package":package.as_ref().map(crate::schema1_control::InstalledPackage::from),
        "trust_changed":false,
        "workspace_changed":false,
        "message":if check {"Catalog checked. No package downloaded, provider executed or local state changed."} else {"Package verified and available locally. Installation does not trust or execute it. Review the source, executable digest and capabilities before explicitly trusting this version; workspace pins and approvals are unchanged."},
    });
    let mut outcome = Outcome::new(
        if update {
            "provider_update"
        } else {
            "provider_install"
        },
        result,
    )?;
    outcome.report.started_at = started_at;
    Ok(Acquired { outcome, package })
}
fn validate_known_versions(
    catalog: &Catalog,
    installed: &[InstalledPackage],
) -> Result<(), AppError> {
    for package in installed {
        if catalog.releases.iter().any(|release| {
            release.provider == package.release.provider
                && release.version == package.release.version
                && release.target == package.release.target
                && release != &package.release
        }) {
            return Err(failure(DistributionError::Integrity));
        }
    }
    Ok(())
}
fn select_release<'a>(
    catalog: &'a Catalog,
    provider: &str,
    target: &str,
    version: Option<&str>,
    setup: bool,
) -> Result<&'a Release, DistributionError> {
    // Validate the whole catalog and request before applying operation compatibility.
    let available = catalog.select(provider, target, version)?;
    if !setup {
        return Ok(available);
    }
    catalog
        .releases
        .iter()
        .filter(|release| {
            release.provider == provider
                && release.target == target
                && release.protocols.contains(&3)
                && version.is_none_or(|version| release.version.to_string() == version)
        })
        .max_by(|a, b| a.version.cmp(&b.version))
        .ok_or(DistributionError::Compatibility)
}
fn selection<'a>(
    available: &'a Release,
    current: Option<&'a InstalledPackage>,
    explicit: bool,
) -> &'a Release {
    match current {
        Some(current) if !explicit && available.version < current.release.version => {
            &current.release
        }
        _ => available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Result<Release, serde_json::Error> {
        serde_json::from_value(serde_json::json!({
            "provider":"fixture", "version":version,
            "target":"x86_64-unknown-linux-gnu", "capabilities":["accounts"],
            "protocols":[2], "archive_sha256":"a".repeat(64),
            "executable_sha256":"b".repeat(64), "archive_size":100
        }))
    }
    #[test]
    fn guided_selection_requires_setup_and_honors_exact_or_catalog_latest()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut compatible = release("1.0.0")?;
        compatible.protocols.push(3);
        let newer = release("2.0.0")?;
        let catalog = Catalog {
            schema_version: 1,
            releases: vec![compatible.clone(), newer.clone()],
        };
        let target = "x86_64-unknown-linux-gnu";
        assert_eq!(
            select_release(&catalog, "fixture", target, None, true)?.version,
            compatible.version
        );
        assert!(select_release(&catalog, "fixture", target, Some("2.0.0"), true).is_err());
        assert_eq!(
            select_release(&catalog, "fixture", target, Some("1.0.0"), true)?.version,
            compatible.version
        );
        assert_eq!(
            select_release(&catalog, "fixture", target, None, false)?.version,
            newer.version
        );
        Ok(())
    }
    #[test]
    fn ordinary_update_never_downgrades_but_exact_version_allows_rollback()
    -> Result<(), Box<dyn std::error::Error>> {
        let old = release("1.0.0")?;
        let current = InstalledPackage {
            release: release("2.0.0")?,
            executable: "/unused/provider".into(),
        };
        assert_eq!(
            selection(&old, Some(&current), false).version,
            current.release.version
        );
        assert_eq!(selection(&old, Some(&current), true).version, old.version);
        assert_eq!(selection(&old, None, false).version, old.version);
        Ok(())
    }
    #[test]
    fn catalog_cannot_redefine_an_installed_version() -> Result<(), Box<dyn std::error::Error>> {
        let original = release("1.0.0")?;
        let installed = vec![InstalledPackage {
            release: original.clone(),
            executable: "/unused/provider".into(),
        }];
        let mut catalog = Catalog {
            schema_version: 1,
            releases: vec![original],
        };
        assert!(validate_known_versions(&catalog, &installed).is_ok());
        catalog.releases[0].executable_sha256 = "c".repeat(64);
        assert!(validate_known_versions(&catalog, &installed).is_err_and(|e| e.code == 3));
        Ok(())
    }
}
