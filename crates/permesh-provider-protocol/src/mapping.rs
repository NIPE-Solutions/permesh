// SPDX-License-Identifier: MIT
//! Explicit draft-to-domain conversion. No serde round trips cross this boundary.
//! The session validates the assembled domain snapshot before returning it.
use crate::records::*;

pub(crate) fn capability(value: Capability) -> permesh_provider_sdk::Capability {
    use permesh_provider_sdk::Capability as SdkCapability;
    match value {
        Capability::Accounts => SdkCapability::Accounts,
        Capability::Identities => SdkCapability::Identities,
        Capability::Resources => SdkCapability::Resources,
        Capability::Groups => SdkCapability::Groups,
        Capability::Memberships => SdkCapability::Memberships,
        Capability::Grants => SdkCapability::Grants,
    }
}

fn identity_kind(value: IdentityKind) -> permesh_core::IdentityKind {
    match value {
        IdentityKind::Human => permesh_core::IdentityKind::Human,
        IdentityKind::External => permesh_core::IdentityKind::External,
        IdentityKind::Service => permesh_core::IdentityKind::Service,
        IdentityKind::Bot => permesh_core::IdentityKind::Bot,
        IdentityKind::Unknown => permesh_core::IdentityKind::Unknown,
    }
}

fn identity_status(value: IdentityStatus) -> permesh_core::IdentityStatus {
    match value {
        IdentityStatus::Active => permesh_core::IdentityStatus::Active,
        IdentityStatus::Inactive => permesh_core::IdentityStatus::Inactive,
        IdentityStatus::External => permesh_core::IdentityStatus::External,
        IdentityStatus::Service => permesh_core::IdentityStatus::Service,
        IdentityStatus::Unknown => permesh_core::IdentityStatus::Unknown,
    }
}

fn privilege(value: Privilege) -> permesh_core::Privilege {
    match value {
        Privilege::Standard => permesh_core::Privilege::Standard,
        Privilege::Elevated => permesh_core::Privilege::Elevated,
        Privilege::Admin => permesh_core::Privilege::Admin,
        Privilege::Owner => permesh_core::Privilege::Owner,
        Privilege::Unknown => permesh_core::Privilege::Unknown,
    }
}

fn certainty(value: Certainty) -> permesh_core::Certainty {
    match value {
        Certainty::Observed => permesh_core::Certainty::Observed,
        Certainty::Inferred => permesh_core::Certainty::Inferred,
        Certainty::Unknown => permesh_core::Certainty::Unknown,
    }
}

fn entity_key(value: EntityKey) -> permesh_core::EntityKey {
    permesh_core::EntityKey {
        provider: value.provider,
        id: value.id,
    }
}

pub(crate) fn identity(value: Identity) -> permesh_core::Identity {
    permesh_core::Identity {
        id: value.id,
        kind: identity_kind(value.kind),
        status: identity_status(value.status),
        verified_emails: value.verified_emails,
    }
}

pub(crate) fn account(value: Account) -> permesh_core::Account {
    permesh_core::Account {
        key: entity_key(value.key),
        login: value.login,
        kind: identity_kind(value.kind),
        verified_emails: value.verified_emails,
    }
}

pub(crate) fn resource(value: Resource) -> permesh_core::Resource {
    permesh_core::Resource {
        key: entity_key(value.key),
        name: value.name,
    }
}

pub(crate) fn group(value: Group) -> permesh_core::Group {
    permesh_core::Group {
        key: entity_key(value.key),
        name: value.name,
    }
}

fn provenance(value: Provenance) -> permesh_core::Provenance {
    permesh_core::Provenance {
        method: value.method,
        observed_at: value.observed_at,
    }
}

pub(crate) fn membership(value: Membership) -> permesh_core::Membership {
    permesh_core::Membership {
        member: subject(value.member),
        group: entity_key(value.group),
        provenance: provenance(value.provenance),
    }
}

pub(crate) fn grant(value: Grant) -> permesh_core::Grant {
    permesh_core::Grant {
        id: value.id,
        subject: subject(value.subject),
        resource: entity_key(value.resource),
        role: value.role,
        privilege: privilege(value.privilege),
        certainty: certainty(value.certainty),
        provenance: provenance(value.provenance),
    }
}

fn subject(value: Subject) -> permesh_core::Subject {
    match value {
        Subject::Account(key) => permesh_core::Subject::Account(entity_key(key)),
        Subject::Group(key) => permesh_core::Subject::Group(entity_key(key)),
    }
}
