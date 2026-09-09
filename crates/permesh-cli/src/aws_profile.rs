// SPDX-License-Identifier: MIT
//! Read one explicitly selected temporary shared-credentials profile. No SDK chain.
use crate::error::AppError;
use permesh_secrets::Secret;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
};
use zeroize::Zeroizing;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_LINE: usize = 16_384;
const MAX_LINES: usize = 10_000;
pub(crate) struct Session {
    access_key_id: Secret,
    secret_access_key: Secret,
    session_token: Secret,
}
impl Session {
    pub(crate) fn credentials(self) -> BTreeMap<String, Secret> {
        BTreeMap::from([
            ("access_key_id".into(), self.access_key_id),
            ("secret_access_key".into(), self.secret_access_key),
            ("session_token".into(), self.session_token),
        ])
    }
}
fn invalid() -> AppError {
    AppError::new(
        3,
        "Cannot read the selected temporary AWS profile. Use an explicit private regular credentials file and a unique named section containing only aws_access_key_id, aws_secret_access_key and aws_session_token. SDK fallback, static keys, role chains, SSO cache and credential_process are unsupported.",
    ).diagnostic(crate::provider_diagnostics::Code::CredentialUnavailable)
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 8192
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'\'' | b'"'))
}
fn parse(text: &str, profile: &str) -> Result<Session, AppError> {
    if text.len() > MAX_BYTES || !valid_profile(profile) {
        return Err(invalid());
    }
    let mut active = false;
    let mut found = false;
    let mut values = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if index >= MAX_LINES
            || line.len() > MAX_LINE
            || line.chars().any(|c| c.is_control() && c != '\t')
        {
            return Err(invalid());
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            let section = line
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .filter(|s| !s.is_empty())
                .ok_or_else(invalid)?;
            active = section == profile;
            if active && found {
                return Err(invalid());
            }
            found |= active;
            continue;
        }
        if !active {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(invalid)?;
        let key = key.trim();
        let value = value.trim();
        if !matches!(
            key,
            "aws_access_key_id" | "aws_secret_access_key" | "aws_session_token"
        ) || !token(value)
            || values.insert(key, value).is_some()
        {
            return Err(invalid());
        }
    }
    let access = values.remove("aws_access_key_id").ok_or_else(invalid)?;
    if !access.starts_with("ASIA")
        || !(16..=128).contains(&access.len())
        || !access.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err(invalid());
    }
    Ok(Session {
        access_key_id: Secret::new(access.into()),
        secret_access_key: Secret::new(
            values
                .remove("aws_secret_access_key")
                .ok_or_else(invalid)?
                .into(),
        ),
        session_token: Secret::new(
            values
                .remove("aws_session_token")
                .ok_or_else(invalid)?
                .into(),
        ),
    })
}
fn valid_profile(profile: &str) -> bool {
    !profile.is_empty()
        && profile.len() <= 128
        && profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
pub(crate) fn load(path: &Path, profile: &str) -> Result<Session, AppError> {
    if !path.is_absolute() || !valid_profile(profile) {
        return Err(invalid());
    }
    let mut prefix = std::path::PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(invalid());
        }
        prefix.push(component);
        // A Windows drive/UNC prefix is not a rooted path on its own. Inspect
        // it after RootDir is appended, then inspect every remaining component.
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| invalid())?;
        if metadata.file_type().is_symlink() {
            return Err(invalid());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid());
            }
        }
    }
    let before = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !before.is_file() || before.len() > MAX_BYTES as u64 {
        return Err(invalid());
    }
    let file = fs::File::open(path).map_err(|_| invalid())?;
    let opened = file.metadata().map_err(|_| invalid())?;
    if !opened.is_file() || opened.len() > MAX_BYTES as u64 {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev()
            || before.ino() != opened.ino()
            || opened.mode() & 0o077 != 0
        {
            return Err(invalid());
        }
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_BYTES + 1));
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    parse(text, profile)
}
#[cfg(test)]
#[path = "aws_profile_tests.rs"]
mod tests;
/// CLI-owned review contract; resolved values are never part of this representation.
#[derive(serde::Serialize)]
pub(crate) struct Review<'a> {
    schema_version: u32,
    backend: &'static str,
    credentials_file: &'a str,
    profile: &'a str,
    credential_slots: [&'static str; 3],
}
impl<'a> From<&'a permesh_config::AwsProfile> for Review<'a> {
    fn from(source: &'a permesh_config::AwsProfile) -> Self {
        Self {
            schema_version: 1,
            backend: "aws_temporary_profile",
            credentials_file: &source.credentials_file,
            profile: &source.profile,
            credential_slots: ["access_key_id", "secret_access_key", "session_token"],
        }
    }
}
