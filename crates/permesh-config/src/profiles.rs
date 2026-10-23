// SPDX-License-Identifier: MIT
//! Access profiles: declared expected access (`profiles/*.yaml`), people
//! assignments (`people/*.yaml`) and directory group mappings (ADR 0033).
//! Stable format 1 (ADR 0036). Entries reference immutable provider IDs, never labels.
use crate::{Config, Error, MAX_CONFIG_BYTES, Result, data, invalid, valid_id, valid_text};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const MAX_PROFILES: usize = 256;
pub const MAX_PROFILE_ENTRIES: usize = 512;
pub const MAX_PEOPLE: usize = 10_000;
pub const MAX_DIRECTORY_GROUPS: usize = 256;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesConfig {
    /// Count profile findings in `permesh check` (exit 6). Report-only when false.
    #[serde(default)]
    pub enforce: bool,
    #[serde(default)]
    pub directory_groups: Vec<DirectoryGroup>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryGroup {
    pub instance: String,
    pub group: String,
    pub profiles: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "RawEntry", into = "RawEntry")]
pub enum ProfileEntry {
    Group {
        instance: String,
        group: String,
    },
    Role {
        instance: String,
        role: String,
        resource: String,
    },
}
impl ProfileEntry {
    pub fn instance(&self) -> &str {
        match self {
            Self::Group { instance, .. } | Self::Role { instance, .. } => instance,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    instance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resource: Option<String>,
}
impl TryFrom<RawEntry> for ProfileEntry {
    type Error = &'static str;
    fn try_from(raw: RawEntry) -> std::result::Result<Self, Self::Error> {
        match (raw.group, raw.role, raw.resource) {
            (Some(group), None, None) => Ok(Self::Group {
                instance: raw.instance,
                group,
            }),
            (None, Some(role), Some(resource)) => Ok(Self::Role {
                instance: raw.instance,
                role,
                resource,
            }),
            _ => Err("a profile entry has either `group` or both `role` and `resource`"),
        }
    }
}
impl From<ProfileEntry> for RawEntry {
    fn from(entry: ProfileEntry) -> Self {
        match entry {
            ProfileEntry::Group { instance, group } => Self {
                instance,
                group: Some(group),
                role: None,
                resource: None,
            },
            ProfileEntry::Role {
                instance,
                role,
                resource,
            } => Self {
                instance,
                group: None,
                role: Some(role),
                resource: Some(resource),
            },
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Optional format marker; absent means [`PROFILE_FORMAT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<String>,
    pub grants: Vec<ProfileEntry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    /// Optional format marker; absent means [`PROFILE_FORMAT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    pub identity: String,
    #[serde(default)]
    pub status: PersonStatus,
    pub profiles: Vec<String>,
}
/// `left` marks an explicit departure; `permesh check` counts remaining access.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonStatus {
    #[default]
    Active,
    Left,
}
/// Validated profiles with includes expanded.
#[derive(Clone, Debug)]
pub struct ProfileSet {
    pub enforce: bool,
    pub profiles: BTreeMap<String, BTreeSet<ProfileEntry>>,
    pub people: BTreeMap<String, BTreeSet<String>>,
    /// Identities whose people file says `status: left`.
    pub left: BTreeSet<String>,
    pub directory_groups: Vec<DirectoryGroup>,
}

/// Whether `name` is usable as a profile name.
pub fn valid_name(name: &str) -> bool {
    valid_id(name)
}

pub(crate) fn validate(config: &ProfilesConfig, instances: &BTreeSet<&str>) -> Result<()> {
    if config.directory_groups.len() > MAX_DIRECTORY_GROUPS {
        return Err(invalid("too many profile directory groups"));
    }
    let mut seen = BTreeSet::new();
    for mapping in &config.directory_groups {
        if !instances.contains(mapping.instance.as_str())
            || !valid_text(&mapping.group)
            || mapping.profiles.is_empty()
            || mapping.profiles.iter().any(|p| !valid_id(p))
            || !seen.insert((&mapping.instance, &mapping.group))
        {
            return Err(invalid(
                "profile directory groups need a configured instance, a group ID and known profile names, once per group",
            ));
        }
    }
    Ok(())
}

/// The people and profile file format this build reads.
pub const PROFILE_FORMAT: u64 = 1;

/// Read first, so a newer file reports its version instead of an unknown field.
#[derive(Deserialize)]
struct FormatPeek {
    #[serde(default)]
    version: Option<u64>,
}

fn in_file(file: &str, error: Error) -> Error {
    Error::File {
        file: file.to_owned(),
        source: Box::new(error),
    }
}
/// Read `*.yaml` files of one directory in name order; symlinks are refused.
fn read_dir<T: serde::de::DeserializeOwned>(
    root: &Path,
    dir: &str,
    limit: usize,
) -> Result<Option<Vec<(String, T)>>> {
    let path = root.join(dir);
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Ok(None);
    };
    if !meta.is_dir() {
        return Err(in_file(dir, Error::Read));
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|_| in_file(dir, Error::Read))? {
        let entry = entry.map_err(|_| in_file(dir, Error::Read))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".yaml") {
            names.push(name);
        }
    }
    if names.len() > limit {
        return Err(in_file(dir, invalid("too many files")));
    }
    names.sort();
    let mut files = Vec::new();
    for name in names {
        let label = format!("{dir}/{name}");
        let file = path.join(&name);
        let meta = std::fs::symlink_metadata(&file).map_err(|_| in_file(&label, Error::Read))?;
        if !meta.is_file() {
            return Err(in_file(&label, Error::Read));
        }
        let bytes = data::read_file(&file, MAX_CONFIG_BYTES).map_err(|e| in_file(&label, e))?;
        // A file that is not even a mapping falls through to the full parse and its error.
        if let Ok(FormatPeek {
            version: Some(version),
        }) = data::parse::<FormatPeek>(&bytes, MAX_CONFIG_BYTES)
            && version != PROFILE_FORMAT
        {
            return Err(in_file(&label, Error::UnsupportedVersion(version)));
        }
        files.push((
            label.clone(),
            data::parse(&bytes, MAX_CONFIG_BYTES).map_err(|e| in_file(&label, e))?,
        ));
    }
    Ok(Some(files))
}

fn check_profile(profile: &Profile, instances: &BTreeSet<&str>) -> Result<()> {
    if !valid_id(&profile.profile)
        || profile
            .description
            .as_deref()
            .is_some_and(|d| !valid_text(d))
        || profile.includes.iter().any(|i| !valid_id(i))
        || profile.grants.len() > MAX_PROFILE_ENTRIES
    {
        return Err(invalid(
            "profiles need a name of letters, digits, '-' or '_', bounded text and at most 512 entries",
        ));
    }
    for entry in &profile.grants {
        let ids: Vec<&str> = match entry {
            ProfileEntry::Group { group, .. } => vec![group],
            ProfileEntry::Role { role, resource, .. } => vec![role, resource],
        };
        if ids.iter().any(|v| !valid_text(v)) {
            return Err(invalid("profile entries need non-empty IDs and roles"));
        }
        if !instances.contains(entry.instance()) {
            return Err(invalid(
                "profile entry names an instance that is not configured in permesh.yaml",
            ));
        }
    }
    Ok(())
}

fn expand(
    name: &str,
    profiles: &BTreeMap<String, Profile>,
    done: &mut BTreeMap<String, BTreeSet<ProfileEntry>>,
    visiting: &mut BTreeSet<String>,
) -> Result<BTreeSet<ProfileEntry>> {
    if let Some(entries) = done.get(name) {
        return Ok(entries.clone());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(invalid("profile includes form a cycle"));
    }
    let profile = profiles
        .get(name)
        .ok_or(invalid("a profile includes an unknown profile"))?;
    let mut entries: BTreeSet<_> = profile.grants.iter().cloned().collect();
    for include in &profile.includes {
        entries.extend(expand(include, profiles, done, visiting)?);
    }
    visiting.remove(name);
    done.insert(name.to_owned(), entries.clone());
    Ok(entries)
}

/// Load profiles next to `permesh.yaml`. `None` when nothing is configured.
pub fn load_profiles(root: &Path, config: &Config) -> Result<Option<ProfileSet>> {
    let instances: BTreeSet<&str> = config.providers.iter().map(|p| p.id.as_str()).collect();
    let profile_files = read_dir::<Profile>(root, "profiles", MAX_PROFILES)?;
    let people_files = read_dir::<Person>(root, "people", MAX_PEOPLE)?;
    // Directories holding only READMEs (as in the starter) are not a configuration.
    if profile_files.as_ref().is_none_or(Vec::is_empty)
        && people_files.as_ref().is_none_or(Vec::is_empty)
        && config.profiles.is_none()
    {
        return Ok(None);
    }
    let mut profiles = BTreeMap::new();
    for (file, profile) in profile_files.unwrap_or_default() {
        check_profile(&profile, &instances).map_err(|e| in_file(&file, e))?;
        if profiles.contains_key(&profile.profile) {
            return Err(in_file(&file, invalid("duplicate profile name")));
        }
        profiles.insert(profile.profile.clone(), profile);
    }
    let mut expanded = BTreeMap::new();
    for name in profiles.keys() {
        let entries = expand(name, &profiles, &mut expanded, &mut BTreeSet::new())
            .map_err(|e| in_file(&format!("profiles ({name})"), e))?;
        expanded.insert(name.clone(), entries);
    }
    let mut people = BTreeMap::new();
    let mut left = BTreeSet::new();
    for (file, person) in people_files.unwrap_or_default() {
        if !valid_text(&person.identity)
            || person.profiles.is_empty()
            || person.profiles.iter().any(|p| !expanded.contains_key(p))
        {
            return Err(in_file(
                &file,
                invalid("people files need an identity and known profile names"),
            ));
        }
        if person.status == PersonStatus::Left {
            left.insert(person.identity.clone());
        }
        if people
            .insert(
                person.identity.clone(),
                person.profiles.into_iter().collect(),
            )
            .is_some()
        {
            return Err(in_file(
                &file,
                invalid("identity is assigned in two people files"),
            ));
        }
    }
    let section = config.profiles.clone().unwrap_or_default();
    for mapping in &section.directory_groups {
        if mapping.profiles.iter().any(|p| !expanded.contains_key(p)) {
            return Err(in_file(
                "permesh.yaml",
                invalid("a profile directory group names an unknown profile"),
            ));
        }
    }
    Ok(Some(ProfileSet {
        enforce: section.enforce,
        profiles: expanded,
        people,
        left,
        directory_groups: section.directory_groups,
    }))
}
