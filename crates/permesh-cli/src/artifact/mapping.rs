// SPDX-License-Identifier: MIT
use super::records::*;
use permesh_core as core;
impl From<IdentityKind> for core::IdentityKind {
    fn from(v: IdentityKind) -> Self {
        match v {
            IdentityKind::Human => Self::Human,
            IdentityKind::Service => Self::Service,
            IdentityKind::Bot => Self::Bot,
            IdentityKind::Unknown => Self::Unknown,
        }
    }
}
impl From<core::IdentityKind> for IdentityKind {
    fn from(v: core::IdentityKind) -> Self {
        match v {
            core::IdentityKind::Human => Self::Human,
            core::IdentityKind::Service => Self::Service,
            core::IdentityKind::Bot => Self::Bot,
            core::IdentityKind::Unknown => Self::Unknown,
        }
    }
}
impl From<IdentityStatus> for core::IdentityStatus {
    fn from(v: IdentityStatus) -> Self {
        match v {
            IdentityStatus::Active => Self::Active,
            IdentityStatus::Inactive => Self::Inactive,
            IdentityStatus::Suspended => Self::Suspended,
            IdentityStatus::Unknown => Self::Unknown,
        }
    }
}
impl From<core::IdentityStatus> for IdentityStatus {
    fn from(v: core::IdentityStatus) -> Self {
        match v {
            core::IdentityStatus::Active => Self::Active,
            core::IdentityStatus::Inactive => Self::Inactive,
            core::IdentityStatus::Suspended => Self::Suspended,
            core::IdentityStatus::Unknown => Self::Unknown,
        }
    }
}
impl From<Affiliation> for core::Affiliation {
    fn from(v: Affiliation) -> Self {
        match v {
            Affiliation::Internal => Self::Internal,
            Affiliation::External => Self::External,
            Affiliation::Unknown => Self::Unknown,
        }
    }
}
impl From<core::Affiliation> for Affiliation {
    fn from(v: core::Affiliation) -> Self {
        match v {
            core::Affiliation::Internal => Self::Internal,
            core::Affiliation::External => Self::External,
            core::Affiliation::Unknown => Self::Unknown,
        }
    }
}
impl From<Privilege> for core::Privilege {
    fn from(v: Privilege) -> Self {
        match v {
            Privilege::Standard => Self::Standard,
            Privilege::Elevated => Self::Elevated,
            Privilege::Admin => Self::Admin,
            Privilege::Owner => Self::Owner,
            Privilege::Unknown => Self::Unknown,
        }
    }
}
impl From<core::Privilege> for Privilege {
    fn from(v: core::Privilege) -> Self {
        match v {
            core::Privilege::Standard => Self::Standard,
            core::Privilege::Elevated => Self::Elevated,
            core::Privilege::Admin => Self::Admin,
            core::Privilege::Owner => Self::Owner,
            core::Privilege::Unknown => Self::Unknown,
        }
    }
}
impl From<EvidenceKind> for core::EvidenceKind {
    fn from(v: EvidenceKind) -> Self {
        match v {
            EvidenceKind::Permission => Self::Permission,
            EvidenceKind::Assignment => Self::Assignment,
            EvidenceKind::PolicyAttachment => Self::PolicyAttachment,
            EvidenceKind::Unknown => Self::Unknown,
        }
    }
}
impl From<core::EvidenceKind> for EvidenceKind {
    fn from(v: core::EvidenceKind) -> Self {
        match v {
            core::EvidenceKind::Permission => Self::Permission,
            core::EvidenceKind::Assignment => Self::Assignment,
            core::EvidenceKind::PolicyAttachment => Self::PolicyAttachment,
            core::EvidenceKind::Unknown => Self::Unknown,
        }
    }
}
impl From<Certainty> for core::Certainty {
    fn from(v: Certainty) -> Self {
        match v {
            Certainty::Observed => Self::Observed,
            Certainty::Derived => Self::Derived,
            Certainty::Inferred => Self::Inferred,
            Certainty::Unknown => Self::Unknown,
        }
    }
}
impl From<core::Certainty> for Certainty {
    fn from(v: core::Certainty) -> Self {
        match v {
            core::Certainty::Observed => Self::Observed,
            core::Certainty::Derived => Self::Derived,
            core::Certainty::Inferred => Self::Inferred,
            core::Certainty::Unknown => Self::Unknown,
        }
    }
}
impl From<&EntityKey> for core::EntityKey {
    fn from(v: &EntityKey) -> Self {
        Self {
            provider: v.provider.clone(),
            id: v.id.clone(),
        }
    }
}
impl From<&core::EntityKey> for EntityKey {
    fn from(v: &core::EntityKey) -> Self {
        Self {
            provider: v.provider.clone(),
            id: v.id.clone(),
        }
    }
}
impl From<&Identity> for core::Identity {
    fn from(v: &Identity) -> Self {
        Self {
            id: v.id.clone(),
            kind: v.kind.into(),
            affiliation: v.affiliation.into(),
            status: v.status.into(),
            verified_emails: v.verified_emails.clone(),
        }
    }
}
impl From<&core::Identity> for Identity {
    fn from(v: &core::Identity) -> Self {
        Self {
            id: v.id.clone(),
            kind: v.kind.into(),
            affiliation: v.affiliation.into(),
            status: v.status.into(),
            verified_emails: v.verified_emails.clone(),
        }
    }
}
impl From<&Account> for core::Account {
    fn from(v: &Account) -> Self {
        Self {
            key: (&v.key).into(),
            login: v.login.clone(),
            kind: v.kind.into(),
            affiliation: v.affiliation.into(),
            status: v.status.into(),
            verified_emails: v.verified_emails.clone(),
        }
    }
}
impl From<&core::Account> for Account {
    fn from(v: &core::Account) -> Self {
        Self {
            key: (&v.key).into(),
            login: v.login.clone(),
            kind: v.kind.into(),
            affiliation: v.affiliation.into(),
            status: v.status.into(),
            verified_emails: v.verified_emails.clone(),
        }
    }
}
impl From<&Resource> for core::Resource {
    fn from(v: &Resource) -> Self {
        Self {
            key: (&v.key).into(),
            name: v.name.clone(),
            kind: v.kind.clone(),
            parent: v.parent.as_ref().map(Into::into),
        }
    }
}
impl From<&core::Resource> for Resource {
    fn from(v: &core::Resource) -> Self {
        Self {
            key: (&v.key).into(),
            name: v.name.clone(),
            kind: v.kind.clone(),
            parent: v.parent.as_ref().map(Into::into),
        }
    }
}
impl From<&Group> for core::Group {
    fn from(v: &Group) -> Self {
        Self {
            key: (&v.key).into(),
            name: v.name.clone(),
        }
    }
}
impl From<&core::Group> for Group {
    fn from(v: &core::Group) -> Self {
        Self {
            key: (&v.key).into(),
            name: v.name.clone(),
        }
    }
}
impl From<&Provenance> for core::Provenance {
    fn from(v: &Provenance) -> Self {
        Self {
            method: v.method.clone(),
            observed_at: v.observed_at.clone(),
        }
    }
}
impl From<&core::Provenance> for Provenance {
    fn from(v: &core::Provenance) -> Self {
        Self {
            method: v.method.clone(),
            observed_at: v.observed_at.clone(),
        }
    }
}
impl From<&Membership> for core::Membership {
    fn from(v: &Membership) -> Self {
        Self {
            member: (&v.member).into(),
            group: (&v.group).into(),
            provenance: (&v.provenance).into(),
        }
    }
}
impl From<&core::Membership> for Membership {
    fn from(v: &core::Membership) -> Self {
        Self {
            member: (&v.member).into(),
            group: (&v.group).into(),
            provenance: (&v.provenance).into(),
        }
    }
}
impl From<&Grant> for core::Grant {
    fn from(v: &Grant) -> Self {
        Self {
            id: v.id.clone(),
            subject: (&v.subject).into(),
            resource: (&v.resource).into(),
            role: v.role.clone(),
            privilege: v.privilege.into(),
            certainty: v.certainty.into(),
            evidence_kind: v.evidence_kind.into(),
            provenance: (&v.provenance).into(),
        }
    }
}
impl From<&core::Grant> for Grant {
    fn from(v: &core::Grant) -> Self {
        Self {
            id: v.id.clone(),
            subject: (&v.subject).into(),
            resource: (&v.resource).into(),
            role: v.role.clone(),
            privilege: v.privilege.into(),
            certainty: v.certainty.into(),
            evidence_kind: v.evidence_kind.into(),
            provenance: (&v.provenance).into(),
        }
    }
}
impl From<&Dataset> for core::Snapshot {
    fn from(v: &Dataset) -> Self {
        Self {
            provider: v.provider.clone(),
            identities: v.identities.iter().map(Into::into).collect(),
            accounts: v.accounts.iter().map(Into::into).collect(),
            resources: v.resources.iter().map(Into::into).collect(),
            groups: v.groups.iter().map(Into::into).collect(),
            memberships: v.memberships.iter().map(Into::into).collect(),
            grants: v.grants.iter().map(Into::into).collect(),
            limitations: v.limitations.clone(),
            complete: v.complete,
        }
    }
}
impl From<&core::Snapshot> for Dataset {
    fn from(v: &core::Snapshot) -> Self {
        Self {
            provider: v.provider.clone(),
            identities: v.identities.iter().map(Into::into).collect(),
            accounts: v.accounts.iter().map(Into::into).collect(),
            resources: v.resources.iter().map(Into::into).collect(),
            groups: v.groups.iter().map(Into::into).collect(),
            memberships: v.memberships.iter().map(Into::into).collect(),
            grants: v.grants.iter().map(Into::into).collect(),
            limitations: v.limitations.clone(),
            complete: v.complete,
        }
    }
}
impl From<&Subject> for core::Subject {
    fn from(v: &Subject) -> Self {
        match v {
            Subject::Account(k) => Self::Account(k.into()),
            Subject::Group(k) => Self::Group(k.into()),
        }
    }
}
impl From<&core::Subject> for Subject {
    fn from(v: &core::Subject) -> Self {
        match v {
            core::Subject::Account(k) => Self::Account(k.into()),
            core::Subject::Group(k) => Self::Group(k.into()),
        }
    }
}
