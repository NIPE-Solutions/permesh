// SPDX-License-Identifier: MIT
//! Versioned package bytes. Installation never grants trust or executes code.
//! Same-user filesystem interference remains outside the protected-store boundary.
use crate::{DistributionError, catalog::Release, trust};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[path = "packages_zip.rs"]
mod archive;
const MANIFEST: &str = "manifest.json";
const LICENSE: &str = "LICENSE";
const MAX_MANIFEST: u64 = 16 * 1024;
const MAX_LICENSE: u64 = 1024 * 1024;
const MAX_PACKAGES: usize = 1024;
#[derive(Clone, Debug, Serialize)]
pub struct InstalledPackage {
    pub release: Release,
    pub executable: PathBuf,
}
#[derive(Clone, Debug)]
pub struct PackageStore {
    root: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    release: Release,
    license_sha256: String,
}
fn storage(_: crate::ExternalError) -> DistributionError {
    DistributionError::Storage
}
fn integrity(_: crate::ExternalError) -> DistributionError {
    DistributionError::Integrity
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn filename(target: &str) -> &'static str {
    if target.ends_with("windows-msvc") {
        "provider.exe"
    } else {
        "provider"
    }
}
fn provider_id(provider: &str) -> bool {
    !provider.is_empty()
        && provider.len() <= 32
        && provider.as_bytes()[0].is_ascii_lowercase()
        && provider
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}
fn exists(path: &Path) -> Result<bool, DistributionError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(DistributionError::Storage),
    }
}
// Concurrent creation of shared ancestors may lose create_new; revalidate the
// resulting directory rather than interpreting an arbitrary existing path as safe.
fn ensure(path: &Path) -> Result<(), DistributionError> {
    if trust::ensure_root(path).is_err() {
        trust::private(path, true).map_err(storage)?;
    }
    Ok(())
}
impl PackageStore {
    pub fn new(root: PathBuf) -> Result<Self, DistributionError> {
        if !root.is_absolute() {
            return Err(DistributionError::Input);
        }
        trust::checked_path(&root, true, true).map_err(storage)?;
        if exists(&root)? {
            trust::private(&root, true).map_err(storage)?;
        }
        Ok(Self { root })
    }
    pub fn install(
        &self,
        release: &Release,
        bytes: &[u8],
    ) -> Result<InstalledPackage, DistributionError> {
        release.validate()?;
        // Complete byte validation precedes creation of any package state.
        let contents = archive::decode(release, bytes)?;
        ensure(&self.root)?;
        let provider = self.root.join(&release.provider);
        ensure(&provider)?;
        let version = provider.join(release.version.to_string());
        ensure(&version)?;
        let destination = version.join(&release.target);
        let lock_path = version.join(format!(".install-{}", release.target));
        let lock = trust::create_private_file(&lock_path, false).map_err(storage)?;
        // Only the successful exclusive creator may remove this reservation.
        let result = (|| {
            if exists(&destination)? {
                let installed = self.load(&destination)?;
                if installed.release != *release {
                    return Err(DistributionError::Integrity);
                }
                return Ok(installed);
            }
            let stage = version.join(format!(".pending-{}", release.target));
            trust::make_dir(&stage).map_err(storage)?;
            let result = (|| {
                trust::private(&stage, true).map_err(storage)?;
                trust::write_new(
                    &stage.join(filename(&release.target)),
                    &contents.executable,
                    true,
                )
                .map_err(storage)?;
                trust::write_new(&stage.join(LICENSE), &contents.license, false)
                    .map_err(storage)?;
                let manifest = Manifest {
                    schema: 1,
                    release: release.clone(),
                    license_sha256: hash(&contents.license),
                };
                let encoded =
                    serde_json::to_vec(&manifest).map_err(|_| DistributionError::Storage)?;
                if encoded.len() as u64 > MAX_MANIFEST {
                    return Err(DistributionError::Integrity);
                }
                trust::write_new(&stage.join(MANIFEST), &encoded, false).map_err(storage)?;
                let staged = self.load(&stage)?;
                if staged.release != *release {
                    return Err(DistributionError::Integrity);
                }
                // The private reservation serializes cooperating installers. Never
                // replace even an incomplete existing destination. Same-user races
                // are not prevented by these path-based filesystem APIs.
                if exists(&destination)? {
                    return Err(DistributionError::Storage);
                }
                fs::rename(&stage, &destination).map_err(|_| DistributionError::Storage)?;
                Ok(InstalledPackage {
                    release: release.clone(),
                    executable: destination.join(filename(&release.target)),
                })
            })();
            if exists(&stage)? {
                cleanup(&stage, filename(&release.target))?;
            }
            result
        })();
        drop(lock);
        trust::private(&lock_path, false).map_err(storage)?;
        fs::remove_file(&lock_path).map_err(|_| DistributionError::Storage)?;
        result
    }
    /// Verified inventory of every installed target/version for one provider.
    /// Missing storage is empty and never created by this read-only operation.
    pub fn list_provider(
        &self,
        provider: &str,
    ) -> Result<Vec<InstalledPackage>, DistributionError> {
        if !provider_id(provider) {
            return Err(DistributionError::Input);
        }
        trust::checked_path(&self.root, true, true).map_err(storage)?;
        if !exists(&self.root)? {
            return Ok(Vec::new());
        }
        trust::private(&self.root, true).map_err(storage)?;
        let root = self.root.join(provider);
        if !exists(&root)? {
            return Ok(Vec::new());
        }
        trust::private(&root, true).map_err(storage)?;
        let mut packages = Vec::new();
        for version in entries(&root, MAX_PACKAGES)? {
            let version_text = version
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or(DistributionError::Integrity)?;
            let parsed =
                semver::Version::parse(version_text).map_err(|_| DistributionError::Integrity)?;
            if !parsed.pre.is_empty()
                || !parsed.build.is_empty()
                || parsed.to_string() != version_text
            {
                return Err(DistributionError::Integrity);
            }
            trust::private(&version, true).map_err(storage)?;
            for target in entries(&version, 15)? {
                let name = target
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or(DistributionError::Integrity)?;
                if let Some(target_name) = name.strip_prefix(".install-") {
                    if !crate::catalog::TARGETS.contains(&target_name) {
                        return Err(DistributionError::Integrity);
                    }
                    trust::private(&target, false).map_err(storage)?;
                    continue;
                }
                if let Some(target_name) = name.strip_prefix(".pending-") {
                    if !crate::catalog::TARGETS.contains(&target_name) {
                        return Err(DistributionError::Integrity);
                    }
                    trust::private(&target, true).map_err(storage)?;
                    continue;
                }
                if !crate::catalog::TARGETS.contains(&name) {
                    return Err(DistributionError::Integrity);
                }
                let package = self.load(&target)?;
                if package.release.provider != provider
                    || package.release.version != parsed
                    || package.release.target != name
                {
                    return Err(DistributionError::Integrity);
                }
                if packages.len() == MAX_PACKAGES {
                    return Err(DistributionError::Storage);
                }
                packages.push(package);
            }
        }
        packages.sort_by(|a, b| {
            a.release
                .version
                .cmp(&b.release.version)
                .then_with(|| a.release.target.cmp(&b.release.target))
        });
        Ok(packages)
    }
    fn load(&self, directory: &Path) -> Result<InstalledPackage, DistributionError> {
        trust::private(directory, true).map_err(storage)?;
        let manifest_path = directory.join(MANIFEST);
        trust::private(&manifest_path, false).map_err(storage)?;
        let bytes = trust::read_bounded(&manifest_path, MAX_MANIFEST).map_err(integrity)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| DistributionError::Integrity)?;
        if manifest.schema != 1 || !trust::valid_digest(&manifest.license_sha256) {
            return Err(DistributionError::Integrity);
        }
        manifest
            .release
            .validate()
            .map_err(|_| DistributionError::Integrity)?;
        let binary = filename(&manifest.release.target);
        let files = entries(directory, 3)?;
        if files.len() != 3
            || files.iter().any(|p| {
                ![binary, LICENSE, MANIFEST]
                    .iter()
                    .any(|n| p.file_name().is_some_and(|name| name == *n))
            })
        {
            return Err(DistributionError::Integrity);
        }
        let executable = directory.join(binary);
        trust::private(&executable, false).map_err(storage)?;
        let inspected = trust::inspect(&executable).map_err(integrity)?;
        if inspected.sha256 != manifest.release.executable_sha256 {
            return Err(DistributionError::Integrity);
        }
        let license = directory.join(LICENSE);
        trust::private(&license, false).map_err(storage)?;
        let license = trust::read_bounded(&license, MAX_LICENSE).map_err(integrity)?;
        if hash(&license) != manifest.license_sha256 {
            return Err(DistributionError::Integrity);
        }
        Ok(InstalledPackage {
            release: manifest.release,
            executable,
        })
    }
}
fn entries(path: &Path, limit: usize) -> Result<Vec<PathBuf>, DistributionError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| DistributionError::Storage)? {
        if paths.len() == limit {
            return Err(DistributionError::Integrity);
        }
        paths.push(entry.map_err(|_| DistributionError::Storage)?.path());
    }
    Ok(paths)
}
fn cleanup(stage: &Path, binary: &str) -> Result<(), DistributionError> {
    trust::private(stage, true).map_err(storage)?;
    let files = entries(stage, 3)?;
    // Validate the complete set before deleting anything; never recurse.
    for file in &files {
        if ![binary, LICENSE, MANIFEST]
            .iter()
            .any(|n| file.file_name().is_some_and(|name| name == *n))
        {
            return Err(DistributionError::Storage);
        }
        trust::private(file, false).map_err(storage)?;
    }
    for file in files {
        fs::remove_file(file).map_err(|_| DistributionError::Storage)?;
    }
    fs::remove_dir(stage).map_err(|_| DistributionError::Storage)
}
#[cfg(test)]
#[path = "packages_tests.rs"]
mod tests;
