// SPDX-License-Identifier: MIT
//! Independent wire5 DTOs. Use the decoder and EOF validation to obtain domain records.
use serde::{Deserialize, Serialize};

/// Closed wire5 record capability set, independent of SDK growth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Accounts,
    Identities,
    Resources,
    Groups,
    Memberships,
    Grants,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Human,
    Service,
    Bot,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityStatus {
    Active,
    Inactive,
    Suspended,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    Standard,
    Elevated,
    Admin,
    Owner,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    Observed,
    Derived,
    Inferred,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Affiliation {
    Internal,
    External,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Permission,
    Assignment,
    PolicyAttachment,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityKey {
    pub provider: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub id: String,
    pub kind: IdentityKind,
    pub affiliation: Affiliation,
    pub status: IdentityStatus,
    pub verified_emails: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub key: EntityKey,
    pub login: String,
    pub kind: IdentityKind,
    pub affiliation: Affiliation,
    pub status: IdentityStatus,
    pub verified_emails: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub key: EntityKey,
    pub name: String,
    pub kind: Option<String>,
    pub parent: Option<EntityKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub key: EntityKey,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub method: String,
    pub observed_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub member: Subject,
    pub group: EntityKey,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: String,
    pub subject: Subject,
    pub resource: EntityKey,
    pub role: String,
    pub privilege: Privilege,
    pub certainty: Certainty,
    pub evidence_kind: EvidenceKind,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "key",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Subject {
    Account(EntityKey),
    Group(EntityKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Record {
    Identity(Identity),
    Account(Account),
    Resource(Resource),
    Group(Group),
    Membership(Membership),
    Grant(Grant),
}
impl Record {
    pub(crate) fn capability(&self) -> Capability {
        match self {
            Self::Identity(_) => Capability::Identities,
            Self::Account(_) => Capability::Accounts,
            Self::Resource(_) => Capability::Resources,
            Self::Group(_) => Capability::Groups,
            Self::Membership(_) => Capability::Memberships,
            Self::Grant(_) => Capability::Grants,
        }
    }
}
