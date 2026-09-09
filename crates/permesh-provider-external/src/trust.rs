// SPDX-License-Identifier: MIT
//! Explicit, user-local trust. Path checks cannot prevent a malicious process
//! running as the same user from racing filesystem operations or launch.
use crate::ExternalError;
use permesh_provider_sdk::Capability;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "trust_windows.rs"]
mod windows;

const MAX_BINARY: u64 = 128 * 1024 * 1024;
const MAX_MANIFEST: u64 = 16 * 1024;
const BINARY: &str = "provider.exe";
const MANIFEST: &str = "manifest.json";
const PENDING: &str = "manifest.pending";

#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub schema: u32,
    pub id: String,
    pub sha256: String,
    pub capabilities: Vec<Capability>,
}
#[derive(Clone, Debug)]
pub struct Registry {
    root: PathBuf,
}

pub(crate) fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.as_bytes()[0].is_ascii_alphabetic()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}
pub(crate) fn valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn valid_caps(caps: &[Capability]) -> bool {
    caps.len() <= 6 && caps.iter().enumerate().all(|(i, c)| !caps[..i].contains(c))
}
pub(crate) fn valid_registration(reg: &Registration) -> bool {
    reg.schema == 1
        && valid_id(&reg.id)
        && valid_digest(&reg.sha256)
        && valid_caps(&reg.capabilities)
}
// Reject symlinks in every existing component, including directory aliases.
pub(crate) fn checked_path(
    path: &Path,
    allow_missing: bool,
    trusted_ancestry: bool,
) -> Result<(), ExternalError> {
    if !path.is_absolute() {
        return Err(ExternalError::Input);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(ExternalError::Input);
        }
        current.push(component);
        // A Windows drive prefix alone (C:) is relative until RootDir arrives.
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(ExternalError::Trust),
            Ok(meta) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if trusted_ancestry && meta.is_dir() {
                        let uid = meta.uid();
                        let mode = meta.mode();
                        // Root and this user are trusted directory owners. The sticky
                        // bit protects their children in shared temporary directories.
                        if (uid != 0 && uid != rustix::process::geteuid().as_raw())
                            || (mode & 0o022 != 0 && mode & 0o1000 == 0)
                        {
                            return Err(ExternalError::Trust);
                        }
                    }
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    // Junctions and other reparse points can redirect traversal too.
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(ExternalError::Trust);
                    }
                    if trusted_ancestry && meta.is_dir() {
                        windows::validate(&current, true, false)?;
                    }
                }
            }
            Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(ExternalError::Trust),
        }
    }
    Ok(())
}
pub(crate) fn private(path: &Path, directory: bool) -> Result<(), ExternalError> {
    checked_path(path, false, true)?;
    let meta = fs::symlink_metadata(path).map_err(|_| ExternalError::Trust)?;
    if (directory && !meta.is_dir()) || (!directory && !meta.is_file()) {
        return Err(ExternalError::Trust);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.uid() != rustix::process::geteuid().as_raw()
            || meta.permissions().mode() & 0o077 != 0
        {
            return Err(ExternalError::Trust);
        }
    }
    #[cfg(windows)]
    windows::validate(path, directory, true)?;
    Ok(())
}
#[cfg(not(windows))]
pub(crate) fn make_dir(path: &Path) -> Result<(), ExternalError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| ExternalError::Storage)
}
#[cfg(windows)]
pub(crate) fn make_dir(path: &Path) -> Result<(), ExternalError> {
    windows::make_dir(path)
}
pub(crate) fn ensure_root(path: &Path) -> Result<(), ExternalError> {
    checked_path(path, true, true)?;
    match fs::symlink_metadata(path) {
        Ok(_) => private(path, true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(ExternalError::Input)?;
            if !parent.exists() {
                ensure_root(parent)?;
            }
            make_dir(path)?;
            private(path, true)
        }
        Err(_) => Err(ExternalError::Storage),
    }
}
pub(crate) fn create_private_file(path: &Path, executable: bool) -> Result<File, ExternalError> {
    #[cfg(not(windows))]
    let mut options = OpenOptions::new();
    #[cfg(not(windows))]
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if executable { 0o700 } else { 0o600 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    #[cfg(not(windows))]
    let file = options.open(path).map_err(|_| ExternalError::Storage)?;
    #[cfg(windows)]
    let file = windows::create_file(path)?;
    Ok(file)
}
pub(crate) fn write_new(path: &Path, bytes: &[u8], executable: bool) -> Result<(), ExternalError> {
    let mut file = create_private_file(path, executable)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ExternalError::Storage)
}
pub(crate) fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ExternalError> {
    checked_path(path, false, false)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| ExternalError::Trust)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(ExternalError::Trust);
    }
    let file = File::open(path).map_err(|_| ExternalError::Storage)?;
    let opened = file.metadata().map_err(|_| ExternalError::Storage)?;
    if !opened.is_file() || opened.len() > limit {
        return Err(ExternalError::Trust);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err(ExternalError::Trust);
        }
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ExternalError::Storage)?;
    if bytes.len() as u64 > limit {
        return Err(ExternalError::Trust);
    }
    Ok(bytes)
}
fn native(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"\x7fELF") {
        return true;
    }
    if bytes.len() >= 4
        && matches!(
            &bytes[..4],
            b"\xfe\xed\xfa\xce"
                | b"\xce\xfa\xed\xfe"
                | b"\xfe\xed\xfa\xcf"
                | b"\xcf\xfa\xed\xfe"
                | b"\xca\xfe\xba\xbe"
                | b"\xbe\xba\xfe\xca"
                | b"\xca\xfe\xba\xbf"
                | b"\xbf\xba\xfe\xca"
        )
    {
        return true;
    }
    if bytes.starts_with(b"MZ") && bytes.len() >= 64 {
        let offset = u32::from_le_bytes([bytes[60], bytes[61], bytes[62], bytes[63]]) as usize;
        return offset.checked_add(4).and_then(|end| bytes.get(offset..end))
            == Some(b"PE\0\0".as_slice());
    }
    false
}
fn inspection(bytes: &[u8]) -> Result<Inspection, ExternalError> {
    if !native(bytes) {
        return Err(ExternalError::Trust);
    }
    Ok(Inspection {
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        size: bytes.len() as u64,
    })
}
/// Narrow the verification-to-launch gap for managed executables. This cannot
/// atomically bind a later path-based spawn against hostile same-user races.
pub(crate) fn verify_executable_pin(path: &Path, expected: &str) -> Result<(), ExternalError> {
    if !valid_digest(expected) {
        return Err(ExternalError::Trust);
    }
    checked_path(path, false, true).map_err(|_| ExternalError::Trust)?;
    private(path, false).map_err(|_| ExternalError::Trust)?;
    if inspect(path).map_err(|_| ExternalError::Trust)?.sha256 != expected {
        return Err(ExternalError::Trust);
    }
    Ok(())
}
pub fn inspect(path: &Path) -> Result<Inspection, ExternalError> {
    inspection(&read_bounded(path, MAX_BINARY)?)
}
#[path = "trust_registry.rs"]
mod registry;

#[cfg(windows)]
pub(crate) fn private_working_directory() -> Result<windows::WorkingDirectory, ExternalError> {
    windows::working_directory()
}
