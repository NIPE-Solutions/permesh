// SPDX-License-Identifier: MIT OR Apache-2.0
//! Immutable retained versions and an atomic selection for explicit trust updates.
use super::*;
const SELECTED: &str = "selected.json";
const SELECTION_PENDING: &str = "selected.pending";
const VERSIONS: &str = "versions";
const LOCK: &str = ".registry.lock";

fn exists(path: &Path) -> Result<bool, ExternalError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ExternalError::Storage),
    }
}
fn read_registration(path: &Path, id: &str) -> Result<Registration, ExternalError> {
    private(path, false)?;
    let reg: Registration = serde_json::from_slice(&read_bounded(path, MAX_MANIFEST)?)
        .map_err(|_| ExternalError::Trust)?;
    if !valid_registration(&reg) || reg.id != id {
        return Err(ExternalError::Trust);
    }
    Ok(reg)
}
// Validate the entire finite layout before removing anything. Never recurse into
// arbitrary names or follow links, even when recovering an incomplete write.
fn leaf_files(dir: &Path) -> Result<Vec<PathBuf>, ExternalError> {
    private(dir, true)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|_| ExternalError::Storage)? {
        let entry = entry.map_err(|_| ExternalError::Storage)?;
        if ![BINARY, MANIFEST, PENDING]
            .iter()
            .any(|name| entry.file_name() == *name)
        {
            return Err(ExternalError::Trust);
        }
        private(&entry.path(), false)?;
        files.push(entry.path());
    }
    Ok(files)
}
fn delete_files(files: Vec<PathBuf>) -> Result<(), ExternalError> {
    for path in files {
        private(&path, false)?;
        fs::remove_file(path).map_err(|_| ExternalError::Storage)?;
    }
    Ok(())
}
impl Registry {
    pub fn new(root: PathBuf) -> Result<Self, ExternalError> {
        checked_path(&root, true, true)?;
        if exists(&root)? {
            private(&root, true)?;
        }
        Ok(Self { root })
    }
    // A create-only private reservation serializes writers/removers. A crashed
    // writer leaves a visible stale lock and subsequent mutations fail closed.
    fn locked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ExternalError>,
    ) -> Result<T, ExternalError> {
        ensure_root(&self.root)?;
        let lock = self.root.join(LOCK);
        let file = create_private_file(&lock, false)?;
        drop(file);
        let result = operation();
        private(&lock, false)?;
        fs::remove_file(lock).map_err(|_| ExternalError::Storage)?;
        result
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
        self.locked(|| {
            let dir = self.root.join(id);
            if !exists(&dir)? {
                make_dir(&dir)?;
                let result = Self::publish(&dir, &bytes, &manifest);
                if result.is_err() {
                    delete_files(leaf_files(&dir)?)?;
                    fs::remove_dir(&dir).map_err(|_| ExternalError::Storage)?;
                }
                result?;
                return Ok(reg);
            }
            private(&dir, true)?;
            // Require a healthy selected registration before appending a version.
            let selected = self.load(id)?;
            self.verify(&selected)?;
            let legacy = read_registration(&dir.join(MANIFEST), id)?;
            if legacy.sha256 == digest {
                return Err(ExternalError::Trust);
            }
            let versions = dir.join(VERSIONS);
            if !exists(&versions)? {
                make_dir(&versions)?;
            }
            private(&versions, true)?;
            let version = versions.join(digest);
            // Existing digests, including different declared capabilities, conflict.
            make_dir(&version)?;
            let result = (|| {
                Self::publish(&version, &bytes, &manifest)?;
                let pending = dir.join(SELECTION_PENDING);
                // Never delete an existing reservation owned by another writer.
                write_new(&pending, &manifest, false)?;
                let selected_path = dir.join(SELECTED);
                if exists(&selected_path)? {
                    private(&selected_path, false)?;
                }
                fs::rename(&pending, &selected_path).map_err(|_| ExternalError::Storage)?;
                Ok(())
            })();
            if result.is_err() {
                // Keep the previous selection and immutable legacy intact. A
                // pending selection is retained for inspection after failed rename.
                delete_files(leaf_files(&version)?)?;
                fs::remove_dir(&version).map_err(|_| ExternalError::Storage)?;
            }
            result?;
            Ok(reg)
        })
    }
    fn publish(dir: &Path, bytes: &[u8], manifest: &[u8]) -> Result<(), ExternalError> {
        private(dir, true)?;
        write_new(&dir.join(BINARY), bytes, true)?;
        write_new(&dir.join(PENDING), manifest, false)?;
        fs::hard_link(dir.join(PENDING), dir.join(MANIFEST)).map_err(|_| ExternalError::Storage)?;
        fs::remove_file(dir.join(PENDING)).map_err(|_| ExternalError::Storage)
    }
    pub fn load(&self, id: &str) -> Result<Registration, ExternalError> {
        if !valid_id(id) {
            return Err(ExternalError::Input);
        }
        private(&self.root, true)?;
        let dir = self.root.join(id);
        private(&dir, true)?;
        let selected = dir.join(SELECTED);
        let reg = read_registration(
            &if exists(&selected)? {
                selected
            } else {
                dir.join(MANIFEST)
            },
            id,
        )?;
        if self.load_pinned(id, &reg.sha256)? != reg {
            return Err(ExternalError::Trust);
        }
        Ok(reg)
    }
    /// Resolve an immutable registration without following the current selection.
    pub fn load_pinned(&self, id: &str, digest: &str) -> Result<Registration, ExternalError> {
        if !valid_id(id) || !valid_digest(digest) {
            return Err(ExternalError::Input);
        }
        private(&self.root, true)?;
        let dir = self.root.join(id);
        private(&dir, true)?;
        let legacy = read_registration(&dir.join(MANIFEST), id)?;
        if legacy.sha256 == digest {
            return Ok(legacy);
        }
        let versions = dir.join(VERSIONS);
        private(&versions, true)?;
        let version = versions.join(digest);
        private(&version, true)?;
        let reg = read_registration(&version.join(MANIFEST), id)?;
        if reg.sha256 != digest {
            return Err(ExternalError::Trust);
        }
        Ok(reg)
    }
    pub fn verify(&self, reg: &Registration) -> Result<PathBuf, ExternalError> {
        if !valid_registration(reg) || self.load_pinned(&reg.id, &reg.sha256)? != *reg {
            return Err(ExternalError::Trust);
        }
        let dir = self.root.join(&reg.id);
        let legacy = read_registration(&dir.join(MANIFEST), &reg.id)?;
        let executable = if legacy.sha256 == reg.sha256 {
            dir.join(BINARY)
        } else {
            dir.join(VERSIONS).join(&reg.sha256).join(BINARY)
        };
        private(&executable, false)?;
        if inspect(&executable)?.sha256 != reg.sha256 {
            return Err(ExternalError::Trust);
        }
        Ok(executable)
    }
    pub fn list(&self) -> Result<Vec<Registration>, ExternalError> {
        checked_path(&self.root, true, true)?;
        if !exists(&self.root)? {
            return Ok(Vec::new());
        }
        private(&self.root, true)?;
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| ExternalError::Storage)? {
            let entry = entry.map_err(|_| ExternalError::Storage)?;
            if entry.file_name() == LOCK {
                private(&entry.path(), false)?;
                continue;
            }
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
        self.locked(|| {
            let dir = self.root.join(id);
            private(&dir, true)?;
            let mut files = Vec::new();
            let mut directories = Vec::new();
            for entry in fs::read_dir(&dir).map_err(|_| ExternalError::Storage)? {
                let entry = entry.map_err(|_| ExternalError::Storage)?;
                if entry.file_name() == VERSIONS {
                    private(&entry.path(), true)?;
                    for version in fs::read_dir(entry.path()).map_err(|_| ExternalError::Storage)? {
                        let version = version.map_err(|_| ExternalError::Storage)?;
                        if !version.file_name().to_str().is_some_and(valid_digest) {
                            return Err(ExternalError::Trust);
                        }
                        files.extend(leaf_files(&version.path())?);
                        directories.push(version.path());
                    }
                    directories.push(entry.path());
                } else {
                    if ![BINARY, MANIFEST, PENDING, SELECTED, SELECTION_PENDING]
                        .iter()
                        .any(|name| entry.file_name() == *name)
                    {
                        return Err(ExternalError::Trust);
                    }
                    private(&entry.path(), false)?;
                    files.push(entry.path());
                }
            }
            delete_files(files)?;
            for path in directories {
                private(&path, true)?;
                fs::remove_dir(path).map_err(|_| ExternalError::Storage)?;
            }
            fs::remove_dir(dir).map_err(|_| ExternalError::Storage)
        })
    }
}
