// SPDX-License-Identifier: MIT
//! Entirely synthetic, deterministic, offline observations.
use permesh_core::*;
use permesh_provider_sdk::*;
pub struct DemoProvider {
    id: String,
}
impl DemoProvider {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
    fn snapshot(&self) -> Snapshot {
        let mut s = Snapshot::new(&self.id);
        let key = |id: &str| EntityKey::new(&self.id, id);
        let provenance = || Provenance {
            method: "synthetic demo fixture".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        };
        for (id, email, login, kind, status, affiliation) in [
            (
                "100",
                "alice@example.com",
                "alice-dev",
                IdentityKind::Human,
                IdentityStatus::Active,
                Affiliation::Internal,
            ),
            (
                "101",
                "bob@example.com",
                "bob-admin",
                IdentityKind::Human,
                IdentityStatus::Active,
                Affiliation::Internal,
            ),
            (
                "102",
                "former@example.com",
                "former-dev",
                IdentityKind::Human,
                IdentityStatus::Inactive,
                Affiliation::Internal,
            ),
            (
                "103",
                "build@example.com",
                "build-bot",
                IdentityKind::Bot,
                IdentityStatus::Active,
                Affiliation::Internal,
            ),
            (
                "104",
                "contractor@example.com",
                "contractor-dev",
                IdentityKind::Human,
                IdentityStatus::Active,
                Affiliation::External,
            ),
            (
                "105",
                "automation@example.com",
                "automation-service",
                IdentityKind::Service,
                IdentityStatus::Active,
                Affiliation::Internal,
            ),
        ] {
            s.identities.push(Identity {
                id: email.into(),
                kind,
                status,
                affiliation,
                verified_emails: vec![email.into()],
            });
            s.accounts.push(Account {
                key: key(id),
                login: login.into(),
                kind,
                // Accounts remain active even after the authoritative identity departs.
                status: IdentityStatus::Active,
                affiliation,
                verified_emails: vec![email.into()],
            });
        }
        s.groups.push(Group {
            key: key("team-10"),
            name: "team/backend".into(),
        });
        s.resources.push(Resource {
            key: key("org-1"),
            name: "acme".into(),
            kind: Some("demo.organization".into()),
            parent: None,
        });
        for (id, name) in [
            ("repo-20", "acme/payments-api"),
            ("repo-21", "acme/infrastructure"),
        ] {
            s.resources.push(Resource {
                kind: Some("demo.repository".into()),
                parent: Some(key("org-1")),
                key: key(id),
                name: name.into(),
            });
        }
        s.memberships.push(Membership {
            member: Subject::Account(key("100")),
            group: key("team-10"),
            provenance: provenance(),
        });
        for (id, subject, resource, role, privilege) in [
            (
                "grant-1",
                Subject::Group(key("team-10")),
                "repo-20",
                "Write",
                Privilege::Standard,
            ),
            (
                "grant-2",
                Subject::Account(key("100")),
                "repo-21",
                "Read",
                Privilege::Standard,
            ),
            (
                "grant-3",
                Subject::Account(key("101")),
                "repo-21",
                "Admin",
                Privilege::Admin,
            ),
            (
                "grant-4",
                Subject::Account(key("102")),
                "repo-20",
                "Read",
                Privilege::Standard,
            ),
            (
                "grant-5",
                Subject::Account(key("103")),
                "repo-20",
                "Write",
                Privilege::Standard,
            ),
            (
                "grant-6",
                Subject::Account(key("104")),
                "repo-20",
                "Read",
                Privilege::Standard,
            ),
            (
                "grant-7",
                Subject::Account(key("105")),
                "repo-21",
                "Write",
                Privilege::Standard,
            ),
        ] {
            s.grants.push(Grant {
                id: id.into(),
                evidence_kind: if matches!(&subject, Subject::Group(_)) {
                    EvidenceKind::Assignment
                } else {
                    EvidenceKind::Permission
                },
                subject,
                resource: key(resource),
                role: role.into(),
                privilege,
                certainty: Certainty::Observed,
                provenance: provenance(),
            });
        }
        s.sort();
        s
    }
}
/// Static capabilities; does not construct a provider or resolve credentials.
pub fn provider_metadata() -> Metadata {
    Metadata {
        kind: "demo".into(),
        capabilities: vec![
            Capability::Accounts,
            Capability::Identities,
            Capability::Resources,
            Capability::Groups,
            Capability::Memberships,
            Capability::Grants,
        ],
    }
}

impl Provider for DemoProvider {
    fn metadata(&self) -> Metadata {
        provider_metadata()
    }
    fn check(&self) -> ProviderFuture<'_, Health> {
        Box::pin(async {
            Ok(Health {
                message: "Synthetic demo ready (offline)".into(),
                limitations: vec![],
            })
        })
    }
    fn discover(&self) -> ProviderFuture<'_, Snapshot> {
        Box::pin(async {
            let s = self.snapshot();
            validate_snapshot(&s)?;
            Ok(s)
        })
    }
}
