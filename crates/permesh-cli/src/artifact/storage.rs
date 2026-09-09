// SPDX-License-Identifier: MIT
use super::{Artifact, MAX_BYTES};
use crate::error::AppError;
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::{
    fmt,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
// Build a bounded JSON value while rejecting duplicate map keys at every depth.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded unique JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Unique, E> {
                Err(E::custom("floating point unsupported"))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                if v.len() > 16384 {
                    Err(E::custom("string limit"))
                } else {
                    Ok(Unique(v.into()))
                }
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                let mut rows = Vec::new();
                while let Some(Unique(v)) = seq.next_element()? {
                    if rows.len() >= permesh_core::MAX_SNAPSHOT_RECORDS {
                        return Err(de::Error::custom("record limit"));
                    }
                    rows.push(v);
                }
                Ok(Unique(rows.into()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(k) = map.next_key::<String>()? {
                    if k.len() > 256 || values.contains_key(&k) || values.len() >= 1024 {
                        return Err(de::Error::custom("invalid map"));
                    }
                    let Unique(v) = map.next_value()?;
                    values.insert(k, v);
                }
                Ok(Unique(values.into()))
            }
        }
        d.deserialize_any(V)
    }
}
fn invalid_document() -> AppError {
    AppError::input(
        "Invalid review document; check JSON structure, duplicate fields and size limits",
    )
}
fn parse_document<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, AppError> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid_document());
    }
    let Unique(value) = serde_json::from_slice(bytes).map_err(|_| invalid_document())?;
    serde_json::from_value(value).map_err(|_| invalid_document())
}
pub fn parse(bytes: &[u8]) -> Result<Artifact, AppError> {
    let mut artifact: Artifact = parse_document(bytes)?;
    artifact.validate()?;
    artifact.normalize();
    Ok(artifact)
}
fn read_bytes(path: &Path) -> Result<Vec<u8>, AppError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| AppError::input("Cannot read review file"))?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err(invalid_document());
    }
    let file = File::open(path).map_err(|_| AppError::input("Cannot open review file"))?;
    let opened = file
        .metadata()
        .map_err(|_| AppError::input("Cannot inspect opened review"))?;
    if !opened.is_file() || opened.len() > MAX_BYTES as u64 {
        return Err(invalid_document());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != opened.dev() || metadata.ino() != opened.ino() {
            return Err(invalid_document());
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::input("Cannot read review file"))?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid_document());
    }
    Ok(bytes)
}
pub fn load(path: &Path) -> Result<Artifact, AppError> {
    parse(&read_bytes(path)?)
}
/// Bounded structural decoding. The caller must validate document semantics.
pub fn load_document<T: DeserializeOwned>(path: &Path) -> Result<T, AppError> {
    parse_document(&read_bytes(path)?)
}
pub fn check_destination(path: &Path, overwrite: bool) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || !overwrite => {
            return Err(AppError::input(
                "Output exists or is not a regular file; choose a new path or explicitly use --overwrite",
            ));
        }
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(AppError::input("Cannot inspect output destination")),
    }
    Ok(())
}
pub fn write(
    path: &Path,
    artifact: &Artifact,
    overwrite: bool,
    cancel: &crate::cancellation::Cancellation,
) -> Result<(), AppError> {
    artifact.validate()?;
    let bytes = serde_json::to_vec_pretty(artifact)
        .map_err(|_| AppError::new(5, "Cannot format snapshot"))?;
    if bytes.len() > MAX_BYTES {
        return Err(AppError::input("Snapshot exceeds the 64 MiB file limit"));
    }
    parse(&bytes)?;
    persist_bytes(path, &bytes, overwrite, cancel)
}
/// The caller validates its versioned document before exporting it.
pub fn write_document<T: Serialize>(
    path: &Path,
    document: &T,
    cancel: &crate::cancellation::Cancellation,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|_| AppError::new(5, "Cannot format review document"))?;
    let _: Value = parse_document(&bytes)?;
    write_bytes(path, &bytes, cancel)
}
pub fn write_bytes(
    path: &Path,
    bytes: &[u8],
    cancel: &crate::cancellation::Cancellation,
) -> Result<(), AppError> {
    persist_bytes(path, bytes, false, cancel)
}
fn persist_bytes(
    path: &Path,
    bytes: &[u8],
    overwrite: bool,
    cancel: &crate::cancellation::Cancellation,
) -> Result<(), AppError> {
    if bytes.len() > MAX_BYTES {
        return Err(AppError::input("Review file exceeds the 64 MiB limit"));
    }
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    check_destination(path, overwrite)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| AppError::new(5, "Cannot create private review file"))?;
    temp.write_all(bytes)
        .and_then(|()| temp.as_file().sync_all())
        .map_err(|_| AppError::new(5, "Cannot write review file"))?;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    if overwrite {
        if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
            return Err(AppError::input("Snapshot destination changed"));
        }
        temp.persist(path)
            .map_err(|_| AppError::new(5, "Cannot replace snapshot file"))?;
    } else {
        temp.persist_noclobber(path).map_err(|_| {
            AppError::input("Cannot create review file; destination may already exist")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifact::{IdentityContext, ProviderCapture, State},
        cancellation::Cancellation,
    };
    fn sample() -> Artifact {
        let mut data = permesh_core::Snapshot::new("fixture");
        data.accounts.push(permesh_core::Account {
            key: permesh_core::EntityKey::new("fixture", "1"),
            login: "fixture".into(),
            kind: permesh_core::IdentityKind::Human,
            affiliation: permesh_core::Affiliation::Unknown,
            status: permesh_core::IdentityStatus::Unknown,
            verified_emails: vec![],
        });
        Artifact {
            format: "permesh_snapshot".into(),
            format_version: 1,
            producer_version: "test".into(),
            started_at: "2026-09-09T00:00:00Z".into(),
            completed_at: "2026-09-09T00:00:00Z".into(),
            identity: IdentityContext {
                authorities: vec![],
                bindings: vec![],
            },
            providers: vec![ProviderCapture {
                instance: "fixture".into(),
                provider_type: "fixture".into(),
                provider_version: None,
                executable_sha256: None,
                capabilities: vec!["accounts".into()],
                context_sha256: "a".repeat(64),
                configured_scope: Default::default(),
                started_at: "2026-09-09T00:00:00Z".into(),
                completed_at: "2026-09-09T00:00:00Z".into(),
                state: State::Complete,
                limitations: vec![],
                data: Some((&data).into()),
                failure_code: None,
                source_observation: None,
            }],
        }
    }
    #[test]
    fn writer_does_not_create_an_artifact_that_its_reader_rejects()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("snapshot.json");
        let mut artifact = sample();
        artifact.providers[0].data.as_mut().ok_or("data")?.accounts[0].login = "x".repeat(16385);
        assert!(write(&path, &artifact, false, &Cancellation::new()).is_err_and(|e| e.code == 2));
        assert!(!path.exists());
        Ok(())
    }
    #[test]
    fn cancelled_export_preserves_existing_files_and_leaves_no_temporary_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("snapshot.json");
        std::fs::write(&path, "original")?;
        let cancel = Cancellation::new();
        cancel.cancel();
        assert!(write(&path, &sample(), true, &cancel).is_err_and(|e| e.code == 130));
        assert_eq!(std::fs::read_to_string(&path)?, "original");
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
}
