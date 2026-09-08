// SPDX-License-Identifier: MIT
//! Protected local approvals bind a complete reviewed configuration to one
//! canonical workspace file and registered provider. They contain no settings or
//! credential references, only a versioned fingerprint of those reviewed inputs.
use crate::{
    ExternalError,
    trust::{self, Registration},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
const MAX_INPUT: usize = 1024 * 1024;
const MAX_RECORD: usize = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub schema: u32,
    pub workspace: PathBuf,
    pub instance: String,
    pub fingerprint: String,
}
#[derive(Clone, Debug)]
pub struct ApprovalStore {
    root: PathBuf,
}
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("serialization exceeds limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn serialize(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, ExternalError> {
    let mut output = BoundedBytes {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut output, value).map_err(|_| ExternalError::Input)?;
    Ok(output.bytes)
}
fn canonical_workspace(workspace: &Path, instance: &str) -> Result<PathBuf, ExternalError> {
    if !workspace.is_absolute() || !trust::valid_id(instance) {
        return Err(ExternalError::Input);
    }
    let canonical = workspace.canonicalize().map_err(|_| ExternalError::Input)?;
    if !canonical.is_file() || canonical.to_str().is_none() {
        return Err(ExternalError::Input);
    }
    Ok(canonical)
}
fn canonicalize_json(value: &mut Value, depth: usize) -> Result<(), ExternalError> {
    if depth > 64 {
        return Err(ExternalError::Input);
    }
    match value {
        Value::Object(object) => {
            object.sort_keys();
            for child in object.values_mut() {
                canonicalize_json(child, depth + 1)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                canonicalize_json(child, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
/// The caller supplies the complete normalized config, including aliases,
/// authority, settings, and credential references, before resolving any secret.
pub fn fingerprint(
    workspace: &Path,
    instance: &str,
    configuration: &impl Serialize,
    registration: &Registration,
) -> Result<String, ExternalError> {
    let workspace = canonical_workspace(workspace, instance)?;
    if !trust::valid_registration(registration) {
        return Err(ExternalError::Input);
    }
    let mut configuration: Value = serde_json::from_slice(&serialize(configuration, MAX_INPUT)?)
        .map_err(|_| ExternalError::Input)?;
    canonicalize_json(&mut configuration, 0)?;
    Ok(digest(&serialize(
        &(
            "permesh-workspace-approval-v1",
            workspace,
            instance,
            configuration,
            registration,
        ),
        MAX_INPUT,
    )?))
}
impl ApprovalStore {
    pub fn new(root: PathBuf) -> Result<Self, ExternalError> {
        trust::checked_path(&root, true, true)?;
        if root.exists() {
            trust::private(&root, true)?;
        }
        Ok(Self { root })
    }
    fn record_path(&self, workspace: &Path, instance: &str) -> Result<PathBuf, ExternalError> {
        let key = digest(&serialize(
            &("permesh-workspace-approval-key-v1", workspace, instance),
            MAX_RECORD,
        )?);
        Ok(self.root.join(format!("{key}.json")))
    }
    fn root_exists(&self) -> Result<bool, ExternalError> {
        trust::checked_path(&self.root, true, true)?;
        if !self.root.exists() {
            return Ok(false);
        }
        trust::private(&self.root, true)?;
        Ok(true)
    }
    pub fn get(&self, workspace: &Path, instance: &str) -> Result<Option<Approval>, ExternalError> {
        let workspace = canonical_workspace(workspace, instance)?;
        if !self.root_exists()? {
            return Ok(None);
        }
        let path = self.record_path(&workspace, instance)?;
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ExternalError::Storage),
            Ok(_) => (),
        }
        trust::private(&path, false)?;
        let record: Approval =
            serde_json::from_slice(&trust::read_bounded(&path, MAX_RECORD as u64)?)
                .map_err(|_| ExternalError::Trust)?;
        if record.schema != 1
            || record.workspace != workspace
            || record.instance != instance
            || !trust::valid_digest(&record.fingerprint)
        {
            return Err(ExternalError::Trust);
        }
        Ok(Some(record))
    }
    pub fn verify(
        &self,
        workspace: &Path,
        instance: &str,
        fingerprint: &str,
    ) -> Result<(), ExternalError> {
        if !trust::valid_digest(fingerprint) {
            return Err(ExternalError::Input);
        }
        match self.get(workspace, instance)? {
            Some(record) if record.fingerprint == fingerprint => Ok(()),
            _ => Err(ExternalError::Trust),
        }
    }
    pub fn approve(
        &self,
        workspace: &Path,
        instance: &str,
        fingerprint: &str,
    ) -> Result<Approval, ExternalError> {
        let workspace = canonical_workspace(workspace, instance)?;
        if !trust::valid_digest(fingerprint) {
            return Err(ExternalError::Input);
        }
        // A malformed existing approval requires explicit revoke before approval.
        let existing = self.get(&workspace, instance)?.is_some();
        let record = Approval {
            schema: 1,
            workspace,
            instance: instance.into(),
            fingerprint: fingerprint.into(),
        };
        let bytes = serialize(&record, MAX_RECORD)?;
        trust::ensure_root(&self.root)?;
        let path = self.record_path(&record.workspace, instance)?;
        let pending = path.with_extension("pending");
        // Reserve the pending name exclusively; do not clean up someone else's
        // pre-existing pending record if this reservation fails.
        let mut file = trust::create_private_file(&pending, false)?;
        // Cleanup below is reached only after this call owns the reservation.
        let written = file
            .write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| ExternalError::Storage);
        drop(file); // Close the exclusive Windows handle before validation/linking.
        let result = (|| {
            written?;
            trust::private(&pending, false)?;
            if existing {
                trust::private(&path, false)?;
                fs::remove_file(&path).map_err(|_| ExternalError::Storage)?;
            }
            // Complete records publish atomically, create-only. Replacement has
            // a short absent state, which fails closed to every reader.
            fs::hard_link(&pending, &path).map_err(|_| ExternalError::Storage)?;
            Ok(record)
        })();
        match fs::symlink_metadata(&pending) {
            Ok(_) => {
                trust::private(&pending, false)?;
                fs::remove_file(&pending).map_err(|_| ExternalError::Storage)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(_) => return Err(ExternalError::Storage),
        }
        result
    }
    pub fn revoke(&self, workspace: &Path, instance: &str) -> Result<(), ExternalError> {
        let workspace = canonical_workspace(workspace, instance)?;
        if !self.root_exists()? {
            return Ok(());
        }
        let path = self.record_path(&workspace, instance)?;
        for known in [&path, &path.with_extension("pending")] {
            match fs::symlink_metadata(known) {
                Ok(_) => {
                    trust::private(known, false)?;
                    fs::remove_file(known).map_err(|_| ExternalError::Storage)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(_) => return Err(ExternalError::Storage),
            }
        }
        Ok(())
    }
}
