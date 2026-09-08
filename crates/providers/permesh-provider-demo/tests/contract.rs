// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_demo::DemoProvider;
use permesh_provider_sdk::{Provider, validate_snapshot};
#[tokio::test]
async fn demo_obeys_contract_and_explains_alice() {
    let provider = DemoProvider::new("demo");
    let s = provider.discover().await.unwrap();
    validate_snapshot(&s).unwrap();
    let result = permesh_core::query_user(&[s], &Default::default(), "alice@example.com").unwrap();
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.access.len(), 2);
    assert!(
        result
            .access
            .iter()
            .any(|p| p.groups.iter().any(|g| g.name == "team/backend"))
    );
    assert!(provider.check().await.is_ok());
}

#[tokio::test]
async fn demo_models_independent_dimensions_containment_and_evidence() {
    use permesh_core::{Affiliation, Certainty, EvidenceKind, IdentityKind, IdentityStatus};
    let snapshot = DemoProvider::new("demo").discover().await.unwrap();
    let build = snapshot
        .identities
        .iter()
        .find(|identity| identity.id == "build@example.com")
        .unwrap();
    assert_eq!(build.kind, IdentityKind::Bot);
    assert_eq!(build.status, IdentityStatus::Active);
    assert_eq!(build.affiliation, Affiliation::Internal);
    let contractor = snapshot
        .identities
        .iter()
        .find(|identity| identity.id == "contractor@example.com")
        .unwrap();
    assert_eq!(
        (contractor.kind, contractor.status, contractor.affiliation),
        (
            IdentityKind::Human,
            IdentityStatus::Active,
            Affiliation::External
        )
    );
    let automation = snapshot
        .identities
        .iter()
        .find(|identity| identity.id == "automation@example.com")
        .unwrap();
    assert_eq!(
        (automation.kind, automation.status, automation.affiliation),
        (
            IdentityKind::Service,
            IdentityStatus::Active,
            Affiliation::Internal
        )
    );
    let former = snapshot
        .accounts
        .iter()
        .find(|account| account.login == "former-dev")
        .unwrap();
    assert_eq!(former.status, IdentityStatus::Active);
    let former_identity = snapshot
        .identities
        .iter()
        .find(|identity| identity.id == "former@example.com")
        .unwrap();
    assert_eq!(former_identity.status, IdentityStatus::Inactive);
    assert!(
        snapshot
            .accounts
            .iter()
            .all(|account| account.status == IdentityStatus::Active)
    );
    let organization = snapshot
        .resources
        .iter()
        .find(|resource| resource.kind.as_deref() == Some("demo.organization"))
        .unwrap();
    assert!(organization.parent.is_none());
    let repositories: Vec<_> = snapshot
        .resources
        .iter()
        .filter(|resource| resource.kind.as_deref() == Some("demo.repository"))
        .collect();
    assert_eq!(repositories.len(), 2);
    assert!(
        repositories
            .iter()
            .all(|resource| resource.parent.as_ref() == Some(&organization.key))
    );
    let grant = snapshot
        .grants
        .iter()
        .find(|grant| grant.id == "grant-1")
        .unwrap();
    assert_eq!(grant.evidence_kind, EvidenceKind::Assignment);
    assert_eq!(grant.certainty, Certainty::Observed);
    assert!(
        snapshot
            .grants
            .iter()
            .any(|grant| grant.evidence_kind == EvidenceKind::Permission)
    );
    let organization_key = organization.key.clone();
    let query =
        permesh_core::query_user(&[snapshot], &Default::default(), "alice@example.com").unwrap();
    assert_eq!(query.access.len(), 2);
    assert!(
        query
            .access
            .iter()
            .all(|path| path.resource.key != organization_key)
    );
    let indirect = query
        .access
        .iter()
        .find(|path| !path.groups.is_empty())
        .unwrap();
    assert_eq!(indirect.certainty(), Certainty::Derived);
}
