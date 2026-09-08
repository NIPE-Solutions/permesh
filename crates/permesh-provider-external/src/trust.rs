// SPDX-License-Identifier: MIT OR Apache-2.0
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
fn make_dir(path: &Path) -> Result<(), ExternalError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| ExternalError::Storage)
}
#[cfg(windows)]
fn make_dir(path: &Path) -> Result<(), ExternalError> {
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
pub fn inspect(path: &Path) -> Result<Inspection, ExternalError> {
    inspection(&read_bounded(path, MAX_BINARY)?)
}
impl Registry {
    pub fn new(root: PathBuf) -> Result<Self, ExternalError> {
        checked_path(&root, true, true)?;
        if root.exists() {
            private(&root, true)?;
        }
        Ok(Self { root })
    }
    pub fn trust(
        &self,
        source: &Path,
        id: &str,
        digest: &str,
        capabilities: &[Capability],
    ) -> Result<Registration, ExternalError> {
        if !valid_id(id) || !valid_digest(digest) || !valid_caps(capabilities) {
            return Err(ExternalError::Input);
        }
        let bytes = read_bounded(source, MAX_BINARY)?;
        if inspection(&bytes)?.sha256 != digest {
            return Err(ExternalError::Trust);
        }
        let reg = Registration {
            schema: 1,
            id: id.into(),
            sha256: digest.into(),
            capabilities: capabilities.into(),
        };
        let manifest = serde_json::to_vec(&reg).map_err(|_| ExternalError::Storage)?;
        ensure_root(&self.root)?;
        let dir = self.root.join(id);
        // Exclusive reservation: an existing or incomplete registration is never replaced.
        make_dir(&dir)?;
        let result = (|| {
            private(&dir, true)?;
            write_new(&dir.join(BINARY), &bytes, true)?;
            write_new(&dir.join(PENDING), &manifest, false)?;
            // Link publishes a complete manifest atomically without replacing anything.
            fs::hard_link(dir.join(PENDING), dir.join(MANIFEST))
                .map_err(|_| ExternalError::Storage)?;
            fs::remove_file(dir.join(PENDING)).map_err(|_| ExternalError::Storage)?;
            Ok(reg)
        })();
        if result.is_err() && self.remove(id).is_err() {
            return Err(ExternalError::Storage);
        }
        result
    }
    pub fn load(&self, id: &str) -> Result<Registration, ExternalError> {
        if !valid_id(id) {
            return Err(ExternalError::Input);
        }
        private(&self.root, true)?;
        let dir = self.root.join(id);
        private(&dir, true)?;
        let path = dir.join(MANIFEST);
        private(&path, false)?;
        let reg: Registration = serde_json::from_slice(&read_bounded(&path, MAX_MANIFEST)?)
            .map_err(|_| ExternalError::Trust)?;
        if !valid_registration(&reg) || reg.id != id {
            return Err(ExternalError::Trust);
        }
        Ok(reg)
    }
    pub fn verify(&self, reg: &Registration) -> Result<PathBuf, ExternalError> {
        if !valid_registration(reg) || self.load(&reg.id)? != *reg {
            return Err(ExternalError::Trust);
        }
        let executable = self.root.join(&reg.id).join(BINARY);
        private(&executable, false)?;
        if inspect(&executable)?.sha256 != reg.sha256 {
            return Err(ExternalError::Trust);
        }
        Ok(executable)
    }
    pub fn list(&self) -> Result<Vec<Registration>, ExternalError> {
        checked_path(&self.root, true, true)?;
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        private(&self.root, true)?;
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| ExternalError::Storage)? {
            let entry = entry.map_err(|_| ExternalError::Storage)?;
            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| ExternalError::Trust)?;
            result.push(self.load(&id)?);
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    }
    pub fn remove(&self, id: &str) -> Result<(), ExternalError> {
        if !valid_id(id) {
            return Err(ExternalError::Input);
        }
        private(&self.root, true)?;
        let dir = self.root.join(id);
        private(&dir, true)?;
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|_| ExternalError::Storage)? {
            let entry = entry.map_err(|_| ExternalError::Storage)?;
            let name = entry.file_name();
            if ![BINARY, MANIFEST, PENDING]
                .iter()
                .any(|allowed| name == *allowed)
            {
                return Err(ExternalError::Trust);
            }
            private(&entry.path(), false)?;
            files.push(entry.path());
        }
        for path in files {
            checked_path(&path, false, true)?;
            fs::remove_file(path).map_err(|_| ExternalError::Storage)?;
        }
        fs::remove_dir(dir).map_err(|_| ExternalError::Storage)
    }
}

#[cfg(windows)]
pub(crate) fn private_working_directory() -> Result<windows::WorkingDirectory, ExternalError> {
    windows::working_directory()
}
