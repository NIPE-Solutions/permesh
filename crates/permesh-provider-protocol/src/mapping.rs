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
        IdentityKind::External => permesh_core::IdentityKind::Unknown,
        IdentityKind::Service => permesh_core::IdentityKind::Service,
        IdentityKind::Bot => permesh_core::IdentityKind::Bot,
        IdentityKind::Unknown => permesh_core::IdentityKind::Unknown,
    }
}

fn identity_status(value: IdentityStatus) -> permesh_core::IdentityStatus {
    match value {
        IdentityStatus::Active => permesh_core::IdentityStatus::Active,
        IdentityStatus::Inactive => permesh_core::IdentityStatus::Inactive,
        IdentityStatus::External | IdentityStatus::Service => permesh_core::IdentityStatus::Unknown,
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

// Legacy External describes affiliation, never actor type. Legacy Service status
// describes actor type, never lifecycle; contrary Human/Bot claims map to unknown kind.
fn affiliation(kind: IdentityKind, status: IdentityStatus) -> permesh_core::Affiliation {
    if kind == IdentityKind::External || status == IdentityStatus::External {
        permesh_core::Affiliation::External
    } else {
        permesh_core::Affiliation::Unknown
    }
}

pub(crate) fn identity(value: Identity) -> permesh_core::Identity {
    let kind = if value.status == IdentityStatus::Service {
        match value.kind {
            IdentityKind::Human | IdentityKind::Bot => permesh_core::IdentityKind::Unknown,
            _ => permesh_core::IdentityKind::Service,
        }
    } else {
        identity_kind(value.kind)
    };
    permesh_core::Identity {
        id: value.id,
        kind,
        status: identity_status(value.status),
        affiliation: affiliation(value.kind, value.status),
        verified_emails: value.verified_emails,
    }
}

pub(crate) fn account(value: Account) -> permesh_core::Account {
    permesh_core::Account {
        key: entity_key(value.key),
        login: value.login,
        kind: identity_kind(value.kind),
        status: permesh_core::IdentityStatus::Unknown,
        affiliation: affiliation(value.kind, IdentityStatus::Unknown),
        verified_emails: value.verified_emails,
    }
}

pub(crate) fn resource(value: Resource) -> permesh_core::Resource {
    permesh_core::Resource {
        kind: None,
        parent: None,
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
        evidence_kind: permesh_core::EvidenceKind::Unknown,
        provenance: provenance(value.provenance),
    }
}

fn subject(value: Subject) -> permesh_core::Subject {
    match value {
        Subject::Account(key) => permesh_core::Subject::Account(entity_key(key)),
        Subject::Group(key) => permesh_core::Subject::Group(entity_key(key)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_identity_dimensions_preserve_unknowns_and_conflicts() {
        for (kind, status, expected_kind, expected_status, affiliation) in [
            (
                IdentityKind::External,
                IdentityStatus::External,
                "unknown",
                "unknown",
                "external",
            ),
            (
                IdentityKind::External,
                IdentityStatus::Active,
                "unknown",
                "active",
                "external",
            ),
            (
                IdentityKind::External,
                IdentityStatus::Inactive,
                "unknown",
                "inactive",
                "external",
            ),
            (
                IdentityKind::Unknown,
                IdentityStatus::Service,
                "service",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Service,
                IdentityStatus::External,
                "service",
                "unknown",
                "external",
            ),
            (
                IdentityKind::External,
                IdentityStatus::Service,
                "service",
                "unknown",
                "external",
            ),
            (
                IdentityKind::Human,
                IdentityStatus::Service,
                "unknown",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Bot,
                IdentityStatus::Service,
                "unknown",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Human,
                IdentityStatus::External,
                "human",
                "unknown",
                "external",
            ),
            (
                IdentityKind::Service,
                IdentityStatus::Inactive,
                "service",
                "inactive",
                "unknown",
            ),
            (
                IdentityKind::Human,
                IdentityStatus::Active,
                "human",
                "active",
                "unknown",
            ),
            (
                IdentityKind::Unknown,
                IdentityStatus::Unknown,
                "unknown",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Human,
                IdentityStatus::Inactive,
                "human",
                "inactive",
                "unknown",
            ),
            (
                IdentityKind::Human,
                IdentityStatus::Unknown,
                "human",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::External,
                IdentityStatus::Unknown,
                "unknown",
                "unknown",
                "external",
            ),
            (
                IdentityKind::Service,
                IdentityStatus::Active,
                "service",
                "active",
                "unknown",
            ),
            (
                IdentityKind::Service,
                IdentityStatus::Service,
                "service",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Service,
                IdentityStatus::Unknown,
                "service",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Bot,
                IdentityStatus::Active,
                "bot",
                "active",
                "unknown",
            ),
            (
                IdentityKind::Bot,
                IdentityStatus::Inactive,
                "bot",
                "inactive",
                "unknown",
            ),
            (
                IdentityKind::Bot,
                IdentityStatus::External,
                "bot",
                "unknown",
                "external",
            ),
            (
                IdentityKind::Bot,
                IdentityStatus::Unknown,
                "bot",
                "unknown",
                "unknown",
            ),
            (
                IdentityKind::Unknown,
                IdentityStatus::Active,
                "unknown",
                "active",
                "unknown",
            ),
            (
                IdentityKind::Unknown,
                IdentityStatus::Inactive,
                "unknown",
                "inactive",
                "unknown",
            ),
            (
                IdentityKind::Unknown,
                IdentityStatus::External,
                "unknown",
                "unknown",
                "external",
            ),
        ] {
            let mapped = serde_json::to_value(identity(Identity {
                id: "identity".into(),
                kind,
                status,
                verified_emails: vec![],
            }))
            .unwrap();
            assert_eq!(mapped["kind"], json!(expected_kind), "{kind:?}/{status:?}");
            assert_eq!(
                mapped["status"],
                json!(expected_status),
                "{kind:?}/{status:?}"
            );
            assert_eq!(
                mapped["affiliation"],
                json!(affiliation),
                "{kind:?}/{status:?}"
            );
        }
    }

    #[test]
    fn legacy_accounts_and_evidence_do_not_invent_missing_dimensions() {
        let key = || EntityKey {
            provider: "legacy".into(),
            id: "a".into(),
        };
        for (kind, expected_kind, affiliation) in [
            (IdentityKind::External, "unknown", "external"),
            (IdentityKind::Human, "human", "unknown"),
            (IdentityKind::Service, "service", "unknown"),
            (IdentityKind::Bot, "bot", "unknown"),
            (IdentityKind::Unknown, "unknown", "unknown"),
        ] {
            let mapped = serde_json::to_value(account(Account {
                key: key(),
                login: "account".into(),
                kind,
                verified_emails: vec![],
            }))
            .unwrap();
            assert_eq!(mapped["kind"], json!(expected_kind));
            assert_eq!(mapped["affiliation"], json!(affiliation));
            assert_eq!(mapped["status"], json!("unknown"));
        }
        let mapped = serde_json::to_value(resource(Resource {
            key: key(),
            name: "resource".into(),
        }))
        .unwrap();
        assert!(mapped["kind"].is_null());
        assert!(mapped["parent"].is_null());
        let mapped = serde_json::to_value(grant(Grant {
            id: "grant".into(),
            subject: Subject::Account(key()),
            resource: key(),
            role: "Read".into(),
            privilege: Privilege::Standard,
            certainty: Certainty::Observed,
            provenance: Provenance {
                method: "legacy".into(),
                observed_at: "2026-01-01T00:00:00Z".into(),
            },
        }))
        .unwrap();
        assert_eq!(mapped["evidence_kind"], json!("unknown"));
        assert_eq!(mapped["certainty"], json!("observed"));
    }
}
