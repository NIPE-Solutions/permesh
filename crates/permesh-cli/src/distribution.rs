// SPDX-License-Identifier: MIT
use crate::{blocking::BlockingPool, cancellation::Cancellation, error::AppError, report::Outcome};
use clap::Args;
use permesh_provider_external::{
    DistributionError,
    catalog::{self, Catalog, Release},
    download,
    packages::{InstalledPackage, PackageStore},
};
use std::{collections::BTreeMap, time::Duration};

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

pub(crate) enum AcquisitionMode {
    Ordinary,
    Setup { portable: bool },
}
pub(crate) struct OfficialSelection {
    pub native_package: InstalledPackage,
    pub target_pins: Option<BTreeMap<String, String>>,
}
impl From<InstalledPackage> for OfficialSelection {
    fn from(native_package: InstalledPackage) -> Self {
        Self {
            native_package,
            target_pins: None,
        }
    }
}
pub(crate) struct Acquired {
    pub outcome: Outcome,
    pub package: Option<InstalledPackage>,
    pub target_pins: Option<BTreeMap<String, String>>,
}
pub async fn run(
    provider: &str,
    version: Option<&str>,
    update: bool,
    check: bool,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    Ok(acquire(
        provider,
        version,
        update,
        check,
        AcquisitionMode::Ordinary,
        pool,
        cancellation,
    )
    .await?
    .outcome)
}

pub(crate) async fn acquire(
    provider: &str,
    version: Option<&str>,
    update: bool,
    check: bool,
    mode: AcquisitionMode,
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
    let installed = tokio::time::timeout(
        Duration::from_secs(120),
        pool.run(move || reader.list_provider(&id)),
    )
    .await
    .map_err(|_| AppError::new(5, "Local package inspection timed out"))??
    .map_err(failure)?;
    if update
        && !installed
            .iter()
            .any(|package| package.release.target == target)
    {
        return Err(AppError::input(
            "This provider has no installed package for this platform; use permesh provider install PROVIDER --version VERSION first",
        ));
    }
    let catalog = download::fetch_catalog().await.map_err(failure)?;
    let installed = native_inventory(&catalog, installed, target)?;
    let setup = matches!(mode, AcquisitionMode::Setup { .. });
    let available = select_release(&catalog, provider, target, version, setup).map_err(failure)?;
    let target_pins = if matches!(mode, AcquisitionMode::Setup { portable: true }) {
        Some(portable_pins(&catalog, available)?)
    } else {
        None
    };
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
    Ok(Acquired {
        outcome,
        package,
        target_pins,
    })
}
fn portable_pins(
    catalog: &Catalog,
    native: &Release,
) -> Result<BTreeMap<String, String>, AppError> {
    catalog.validate().map_err(failure)?;
    if !catalog.releases.contains(native) {
        return Err(AppError::input(
            "Portable setup requires the native release in the selected catalog version",
        ));
    }
    let mut pins = BTreeMap::new();
    for release in catalog
        .releases
        .iter()
        .filter(|release| release.provider == native.provider && release.version == native.version)
    {
        if release.discovery_protocol != native.discovery_protocol
            || release.protocols.len() != native.protocols.len()
            || !release
                .protocols
                .iter()
                .all(|p| native.protocols.contains(p))
            || release.capabilities.len() != native.capabilities.len()
            || !release
                .capabilities
                .iter()
                .all(|c| native.capabilities.contains(c))
        {
            return Err(AppError::input(
                "Portable setup requires matching discovery protocol, setup protocols and capabilities across every target of the exact selected version",
            ));
        }
        pins.insert(release.target.clone(), release.executable_sha256.clone());
    }
    if pins.len() < 2 {
        return Err(AppError::input(
            "Portable setup is unavailable for this version: at least two supported target releases are required. Choose another exact version or omit --portable",
        ));
    }
    Ok(pins)
}
fn native_inventory(
    catalog: &Catalog,
    mut installed: Vec<InstalledPackage>,
    target: &str,
) -> Result<Vec<InstalledPackage>, AppError> {
    // Foreign targets still carry immutable metadata that the catalog must respect.
    validate_known_versions(catalog, &installed)?;
    installed.retain(|package| package.release.target == target);
    installed.sort_by(|a, b| a.release.version.cmp(&b.release.version));
    Ok(installed)
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
        catalog.releases[0].discovery_protocol = catalog::DiscoveryProtocol::NegotiatedV1;
        catalog.releases[0].protocols = vec![3];
        assert!(validate_known_versions(&catalog, &installed).is_err_and(|e| e.code == 3));
        catalog.releases[0] = installed[0].release.clone();
        catalog.releases[0].executable_sha256 = "c".repeat(64);
        assert!(validate_known_versions(&catalog, &installed).is_err_and(|e| e.code == 3));
        Ok(())
    }
}

#[cfg(test)]
mod portable_tests {
    use super::*;
    fn release(target: &str, version: &str, digest: &str) -> Release {
        serde_json::from_value(serde_json::json!({"provider":"github","version":version,"target":target,"capabilities":["accounts","resources"],"protocols":[2,3],"archive_sha256":"a".repeat(64),"executable_sha256":digest.repeat(64),"archive_size":100})).unwrap_or_else(|_| panic!("fixture"))
    }
    #[test]
    fn portable_pins_use_one_exact_version_and_allow_supported_partial_coverage() {
        let native = release("x86_64-unknown-linux-gnu", "1.0.0", "b");
        let foreign = release("aarch64-apple-darwin", "1.0.0", "c");
        let mut catalog = Catalog {
            schema_version: 1,
            releases: vec![
                native.clone(),
                foreign.clone(),
                release("aarch64-apple-darwin", "2.0.0", "d"),
            ],
        };
        let pins = portable_pins(&catalog, &native).unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[&foreign.target], foreign.executable_sha256);
        assert_eq!(pins[&native.target], native.executable_sha256);
        catalog.releases[1].protocols.reverse();
        catalog.releases[1].capabilities.reverse();
        assert!(portable_pins(&catalog, &native).is_ok());
    }
    #[test]
    fn foreign_installed_metadata_conflict_is_rejected_before_native_filtering() {
        let native = release("x86_64-unknown-linux-gnu", "1.0.0", "b");
        let foreign = release("aarch64-apple-darwin", "1.0.0", "c");
        let installed = vec![InstalledPackage {
            release: foreign.clone(),
            executable: "/unused/foreign".into(),
        }];
        let mut catalog = Catalog {
            schema_version: 1,
            releases: vec![native.clone(), foreign],
        };
        assert!(
            native_inventory(&catalog, installed.clone(), &native.target)
                .unwrap_or_else(|_| panic!("valid inventory"))
                .is_empty()
        );
        catalog.releases[1].executable_sha256 = "d".repeat(64);
        assert!(native_inventory(&catalog, installed, &native.target).is_err_and(|e| e.code == 3));
    }
    #[test]
    fn portable_group_rejects_contract_disagreement_missing_native_and_single_target() {
        let native = release("x86_64-unknown-linux-gnu", "1.0.0", "b");
        let foreign = release("aarch64-apple-darwin", "1.0.0", "c");
        for mutation in 0..5 {
            let mut catalog = Catalog {
                schema_version: 1,
                releases: vec![native.clone(), foreign.clone()],
            };
            match mutation {
                0 => {
                    catalog.releases.remove(0);
                }
                1 => {
                    catalog.releases.pop();
                }
                2 => {
                    catalog.releases[1].capabilities.pop();
                }
                3 => {
                    catalog.releases[1].protocols = vec![2];
                }
                _ => {
                    catalog.releases[1].protocols = vec![3];
                    catalog.releases[1].discovery_protocol =
                        catalog::DiscoveryProtocol::NegotiatedV1;
                }
            }
            assert!(portable_pins(&catalog, &native).is_err());
        }
    }
}
