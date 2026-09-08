// SPDX-License-Identifier: MIT
//! CLI-owned output schema 1. Domain serde implementations are not this contract.
//!
//! Borrow payload strings while mapping every field and enum explicitly. Preserve
//! the query's ordering and evidence; presentation must not reclassify or truncate.
use permesh_core as core;
use serde::Serialize;

fn rows<'a, T, U: From<&'a T>>(values: &'a [T]) -> Vec<U> {
    values.iter().map(U::from).collect()
}

#[derive(Serialize)]
struct EntityKey<'a> {
    provider: &'a str,
    id: &'a str,
}
impl<'a> From<&'a core::EntityKey> for EntityKey<'a> {
    fn from(value: &'a core::EntityKey) -> Self {
        Self {
            provider: &value.provider,
            id: &value.id,
        }
    }
}

#[derive(Serialize)]
struct Identity<'a> {
    id: &'a str,
    kind: &'static str,
    status: &'static str,
    verified_emails: &'a [String],
}
impl<'a> From<&'a core::Identity> for Identity<'a> {
    fn from(value: &'a core::Identity) -> Self {
        Self {
            id: &value.id,
            kind: identity_kind(value.kind),
            status: identity_status(value.status),
            verified_emails: &value.verified_emails,
        }
    }
}

#[derive(Serialize)]
struct Account<'a> {
    key: EntityKey<'a>,
    login: &'a str,
    kind: &'static str,
    verified_emails: &'a [String],
}
impl<'a> From<&'a core::Account> for Account<'a> {
    fn from(value: &'a core::Account) -> Self {
        Self {
            key: (&value.key).into(),
            login: &value.login,
            kind: identity_kind(value.kind),
            verified_emails: &value.verified_emails,
        }
    }
}

#[derive(Serialize)]
struct Resource<'a> {
    key: EntityKey<'a>,
    name: &'a str,
}
impl<'a> From<&'a core::Resource> for Resource<'a> {
    fn from(value: &'a core::Resource) -> Self {
        Self {
            key: (&value.key).into(),
            name: &value.name,
        }
    }
}

#[derive(Serialize)]
struct Group<'a> {
    key: EntityKey<'a>,
    name: &'a str,
}
impl<'a> From<&'a core::Group> for Group<'a> {
    fn from(value: &'a core::Group) -> Self {
        Self {
            key: (&value.key).into(),
            name: &value.name,
        }
    }
}

#[derive(Serialize)]
struct Provenance<'a> {
    method: &'a str,
    observed_at: &'a str,
}
impl<'a> From<&'a core::Provenance> for Provenance<'a> {
    fn from(value: &'a core::Provenance) -> Self {
        Self {
            method: &value.method,
            observed_at: &value.observed_at,
        }
    }
}

#[derive(Serialize)]
struct Membership<'a> {
    member: Subject<'a>,
    group: EntityKey<'a>,
    provenance: Provenance<'a>,
}
impl<'a> From<&'a core::Membership> for Membership<'a> {
    fn from(value: &'a core::Membership) -> Self {
        Self {
            member: (&value.member).into(),
            group: (&value.group).into(),
            provenance: (&value.provenance).into(),
        }
    }
}

#[derive(Serialize)]
struct Grant<'a> {
    id: &'a str,
    subject: Subject<'a>,
    resource: EntityKey<'a>,
    role: &'a str,
    privilege: &'static str,
    certainty: &'static str,
    provenance: Provenance<'a>,
}
impl<'a> From<&'a core::Grant> for Grant<'a> {
    fn from(value: &'a core::Grant) -> Self {
        Self {
            id: &value.id,
            subject: (&value.subject).into(),
            resource: (&value.resource).into(),
            role: &value.role,
            privilege: privilege(value.privilege),
            certainty: certainty(value.certainty),
            provenance: (&value.provenance).into(),
        }
    }
}

#[derive(Serialize)]
struct AccessPath<'a> {
    account: EntityKey<'a>,
    groups: Vec<Group<'a>>,
    memberships: Vec<Membership<'a>>,
    resource: Resource<'a>,
    grant: Grant<'a>,
}
impl<'a> From<&'a core::AccessPath> for AccessPath<'a> {
    fn from(value: &'a core::AccessPath) -> Self {
        Self {
            account: (&value.account).into(),
            groups: rows(&value.groups),
            memberships: rows(&value.memberships),
            resource: (&value.resource).into(),
            grant: (&value.grant).into(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct UserAccess<'a> {
    identity: Option<Identity<'a>>,
    accounts: Vec<Account<'a>>,
    access: Vec<AccessPath<'a>>,
}
impl<'a> From<&'a core::UserAccess> for UserAccess<'a> {
    fn from(value: &'a core::UserAccess) -> Self {
        Self {
            identity: value.identity.as_ref().map(Into::into),
            accounts: rows(&value.accounts),
            access: rows(&value.access),
        }
    }
}

#[derive(Serialize)]
struct AdminAccount<'a> {
    account: Account<'a>,
    identity: IdentityResolution<'a>,
}
impl<'a> From<&'a core::AdminAccount> for AdminAccount<'a> {
    fn from(value: &'a core::AdminAccount) -> Self {
        Self {
            account: (&value.account).into(),
            identity: (&value.identity).into(),
        }
    }
}

#[derive(Serialize)]
struct UnresolvedGrant<'a> {
    grant: Grant<'a>,
    resource: Resource<'a>,
    group: Option<Group<'a>>,
}
impl<'a> From<&'a core::UnresolvedGrant> for UnresolvedGrant<'a> {
    fn from(value: &'a core::UnresolvedGrant) -> Self {
        Self {
            grant: (&value.grant).into(),
            resource: (&value.resource).into(),
            group: value.group.as_ref().map(Into::into),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct AdminAccess<'a> {
    accounts: Vec<AdminAccount<'a>>,
    access: Vec<AccessPath<'a>>,
    unknown_access: Vec<AccessPath<'a>>,
    unresolved_grants: Vec<UnresolvedGrant<'a>>,
}
impl<'a> From<&'a core::AdminAccess> for AdminAccess<'a> {
    fn from(value: &'a core::AdminAccess) -> Self {
        Self {
            accounts: rows(&value.accounts),
            access: rows(&value.access),
            unknown_access: rows(&value.unknown_access),
            unresolved_grants: rows(&value.unresolved_grants),
        }
    }
}

#[derive(Serialize)]
struct OrphanedAccount<'a> {
    account: Account<'a>,
    identity: IdentityResolution<'a>,
    reason: &'static str,
}
impl<'a> From<&'a core::OrphanedAccount> for OrphanedAccount<'a> {
    fn from(value: &'a core::OrphanedAccount) -> Self {
        Self {
            account: (&value.account).into(),
            identity: (&value.identity).into(),
            reason: orphan_reason(value.reason),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct OrphanedAccess<'a> {
    authorities: &'a [String],
    authority_complete: bool,
    accounts: Vec<OrphanedAccount<'a>>,
    access: Vec<AccessPath<'a>>,
}
impl<'a> From<&'a core::OrphanedAccess> for OrphanedAccess<'a> {
    fn from(value: &'a core::OrphanedAccess) -> Self {
        Self {
            authorities: &value.authorities,
            authority_complete: value.authority_complete,
            accounts: rows(&value.accounts),
            access: rows(&value.access),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Snapshot<'a> {
    provider: &'a str,
    identities: Vec<Identity<'a>>,
    accounts: Vec<Account<'a>>,
    resources: Vec<Resource<'a>>,
    groups: Vec<Group<'a>>,
    memberships: Vec<Membership<'a>>,
    grants: Vec<Grant<'a>>,
    limitations: &'a [String],
    complete: bool,
}
impl<'a> From<&'a core::Snapshot> for Snapshot<'a> {
    fn from(value: &'a core::Snapshot) -> Self {
        Self {
            provider: &value.provider,
            identities: rows(&value.identities),
            accounts: rows(&value.accounts),
            resources: rows(&value.resources),
            groups: rows(&value.groups),
            memberships: rows(&value.memberships),
            grants: rows(&value.grants),
            limitations: &value.limitations,
            complete: value.complete,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "key")]
enum Subject<'a> {
    #[serde(rename = "account")]
    Account(EntityKey<'a>),
    #[serde(rename = "group")]
    Group(EntityKey<'a>),
}
impl<'a> From<&'a core::Subject> for Subject<'a> {
    fn from(value: &'a core::Subject) -> Self {
        match value {
            core::Subject::Account(key) => Self::Account(key.into()),
            core::Subject::Group(key) => Self::Group(key.into()),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "state")]
enum IdentityResolution<'a> {
    #[serde(rename = "resolved")]
    Resolved { identity: Identity<'a> },
    #[serde(rename = "unmapped")]
    Unmapped,
    #[serde(rename = "ambiguous")]
    Ambiguous { candidates: &'a [String] },
}
impl<'a> From<&'a core::IdentityResolution> for IdentityResolution<'a> {
    fn from(value: &'a core::IdentityResolution) -> Self {
        match value {
            core::IdentityResolution::Resolved { identity } => Self::Resolved {
                identity: identity.into(),
            },
            core::IdentityResolution::Unmapped => Self::Unmapped,
            core::IdentityResolution::Ambiguous { candidates } => Self::Ambiguous { candidates },
        }
    }
}

fn identity_kind(value: core::IdentityKind) -> &'static str {
    match value {
        core::IdentityKind::Human => "human",
        core::IdentityKind::External => "external",
        core::IdentityKind::Service => "service",
        core::IdentityKind::Bot => "bot",
        core::IdentityKind::Unknown => "unknown",
    }
}

fn identity_status(value: core::IdentityStatus) -> &'static str {
    match value {
        core::IdentityStatus::Active => "active",
        core::IdentityStatus::Inactive => "inactive",
        core::IdentityStatus::External => "external",
        core::IdentityStatus::Service => "service",
        core::IdentityStatus::Unknown => "unknown",
    }
}

fn privilege(value: core::Privilege) -> &'static str {
    match value {
        core::Privilege::Standard => "standard",
        core::Privilege::Elevated => "elevated",
        core::Privilege::Admin => "admin",
        core::Privilege::Owner => "owner",
        core::Privilege::Unknown => "unknown",
    }
}

fn certainty(value: core::Certainty) -> &'static str {
    match value {
        core::Certainty::Observed => "observed",
        core::Certainty::Inferred => "inferred",
        core::Certainty::Unknown => "unknown",
    }
}

fn orphan_reason(value: core::OrphanReason) -> &'static str {
    match value {
        core::OrphanReason::InactiveIdentity => "inactive_identity",
        core::OrphanReason::UnknownIdentity => "unknown_identity",
        core::OrphanReason::UnknownStatus => "unknown_status",
        core::OrphanReason::AmbiguousIdentity => "ambiguous_identity",
        core::OrphanReason::ExternalIdentity => "external_identity",
        core::OrphanReason::ServiceAccount => "service_account",
        core::OrphanReason::Bot => "bot",
        core::OrphanReason::Unassessed => "unassessed",
    }
}
