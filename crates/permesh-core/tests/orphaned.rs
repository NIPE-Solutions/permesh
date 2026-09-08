// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;

fn account(id: &str, kind: IdentityKind) -> Account {
    Account {
        key: EntityKey::new("app", id),
        login: id.into(),
        kind,
        verified_emails: vec![format!("{id}@example.com")],
    }
}
fn identity(id: &str, kind: IdentityKind, status: IdentityStatus) -> Identity {
    Identity {
        id: id.into(),
        kind,
        status,
        verified_emails: vec![format!("{id}@example.com")],
    }
}
fn query(snapshots: &[Snapshot], aliases: &Aliases) -> OrphanedAccess {
    query_orphaned(snapshots, aliases, &["directory".into()]).unwrap()
}
#[test]
fn classifies_accounts_without_grants_and_preserves_uncertainty() {
    let mut directory = Snapshot::new("directory");
    directory.identities = vec![
        identity("active", IdentityKind::Human, IdentityStatus::Active),
        identity("inactive", IdentityKind::Human, IdentityStatus::Inactive),
        identity("unknown", IdentityKind::Human, IdentityStatus::Unknown),
        identity("service", IdentityKind::Service, IdentityStatus::Service),
        identity("external", IdentityKind::Human, IdentityStatus::External),
        identity("bot", IdentityKind::Service, IdentityStatus::Service),
        identity("inactive-bot", IdentityKind::Bot, IdentityStatus::Inactive),
    ];
    let mut app = Snapshot::new("app");
    app.accounts = [
        "unknown",
        "active",
        "inactive",
        "service",
        "external",
        "unmatched",
        "alias",
        "ambiguous",
    ]
    .into_iter()
    .map(|id| account(id, IdentityKind::Human))
    .collect();
    app.accounts.push(account("bot", IdentityKind::Bot));
    app.accounts
        .push(account("inactive-bot", IdentityKind::Bot));
    app.accounts
        .push(account("unmapped-service", IdentityKind::Service));
    app.accounts
        .push(account("unmapped-bot", IdentityKind::Bot));
    app.accounts
        .push(account("unmapped-external", IdentityKind::External));
    let aliases = [
        (
            "alias-only".into(),
            [("app".into(), vec!["alias".into(), "ambiguous".into()])].into(),
        ),
        (
            "other".into(),
            [("app".into(), vec!["ambiguous".into()])].into(),
        ),
    ]
    .into();
    let result = query(&[directory, app], &aliases);
    assert!(result.authority_complete);
    let reasons: Vec<_> = result
        .accounts
        .iter()
        .map(|a| (a.account.key.id.as_str(), a.reason))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("alias", OrphanReason::UnknownStatus),
            ("ambiguous", OrphanReason::AmbiguousIdentity),
            ("bot", OrphanReason::Bot),
            ("external", OrphanReason::ExternalIdentity),
            ("inactive", OrphanReason::InactiveIdentity),
            ("inactive-bot", OrphanReason::InactiveIdentity),
            ("service", OrphanReason::ServiceAccount),
            ("unknown", OrphanReason::UnknownStatus),
            ("unmapped-bot", OrphanReason::Bot),
            ("unmapped-external", OrphanReason::ExternalIdentity),
            ("unmapped-service", OrphanReason::ServiceAccount),
            ("unmatched", OrphanReason::UnknownIdentity),
        ]
    );
    assert!(result.access.is_empty());
}
#[test]
fn missing_or_partial_authority_keeps_every_account_unassessed() {
    let mut directory = Snapshot::new("directory");
    directory.identities.push(identity(
        "active",
        IdentityKind::Human,
        IdentityStatus::Active,
    ));
    directory.complete = false;
    let mut app = Snapshot::new("app");
    app.accounts = vec![
        account("active", IdentityKind::Human),
        account("bot", IdentityKind::Bot),
    ];
    for snapshots in [vec![directory, app.clone()], vec![app]] {
        let result = query(&snapshots, &Aliases::new());
        assert!(!result.authority_complete);
        assert_eq!(result.accounts.len(), 2);
        assert!(
            result
                .accounts
                .iter()
                .all(|a| a.reason == OrphanReason::Unassessed)
        );
    }
    assert!(matches!(
        query_orphaned(&[], &Aliases::new(), &[]),
        Err(DomainError::Identifier)
    ));
}
#[test]
fn ignores_untrusted_identities_and_marks_conflicting_authorities_ambiguous() {
    let mut directory = Snapshot::new("directory");
    directory.identities.push(identity(
        "known",
        IdentityKind::Human,
        IdentityStatus::Inactive,
    ));
    let mut app = Snapshot::new("app");
    app.accounts = vec![
        account("known", IdentityKind::Human),
        account("untrusted", IdentityKind::Human),
    ];
    app.identities = vec![
        identity("known", IdentityKind::Human, IdentityStatus::Active),
        identity("untrusted", IdentityKind::Human, IdentityStatus::Active),
    ];
    let result = query(&[directory.clone(), app.clone()], &Aliases::new());
    assert_eq!(result.accounts[0].reason, OrphanReason::InactiveIdentity);
    assert_eq!(result.accounts[1].reason, OrphanReason::UnknownIdentity);
    let mut second = Snapshot::new("second");
    second.identities.push(identity(
        "known",
        IdentityKind::Human,
        IdentityStatus::Active,
    ));
    let snapshots = [directory, app, second];
    let result = query_orphaned(
        &snapshots,
        &Aliases::new(),
        &["directory".into(), "second".into()],
    )
    .unwrap();
    assert!(result.authority_complete);
    assert_eq!(result.accounts[0].reason, OrphanReason::AmbiguousIdentity);
    assert!(
        matches!(&result.accounts[0].identity, IdentityResolution::Ambiguous { candidates } if candidates == &["known"])
    );
    let result = query_orphaned(
        &snapshots,
        &Aliases::new(),
        &["directory".into(), "missing".into()],
    )
    .unwrap();
    assert!(!result.authority_complete);
    assert!(
        result
            .accounts
            .iter()
            .all(|a| a.reason == OrphanReason::Unassessed)
    );
}
#[test]
fn large_active_directory_leaves_only_small_unmatched_lookup() {
    let mut directory = Snapshot::new("directory");
    let mut app = Snapshot::new("app");
    for n in 0..50_000 {
        let id = format!("active-{n}");
        directory
            .identities
            .push(identity(&id, IdentityKind::Human, IdentityStatus::Active));
        app.accounts.push(account(&id, IdentityKind::Human));
    }
    app.accounts.push(account("unmatched", IdentityKind::Human));
    let result = query(&[directory, app], &Aliases::new());
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.accounts[0].account.login, "unmatched");
}
#[test]
fn keeps_direct_and_cyclic_group_paths_with_original_evidence_in_stable_order() {
    let mut directory = Snapshot::new("directory");
    directory.identities.push(identity(
        "active",
        IdentityKind::Human,
        IdentityStatus::Active,
    ));
    let mut app = Snapshot::new("app");
    app.accounts = vec![
        account("unmatched", IdentityKind::Human),
        account("active", IdentityKind::Human),
    ];
    let p = Provenance {
        method: "provider membership endpoint".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    app.resources.push(Resource {
        key: EntityKey::new("app", "repo"),
        name: "repository".into(),
    });
    app.groups = ["one", "two"]
        .map(|id| Group {
            key: EntityKey::new("app", id),
            name: id.into(),
        })
        .into();
    app.memberships = vec![
        Membership {
            member: Subject::Account(EntityKey::new("app", "unmatched")),
            group: EntityKey::new("app", "one"),
            provenance: p.clone(),
        },
        Membership {
            member: Subject::Group(EntityKey::new("app", "one")),
            group: EntityKey::new("app", "two"),
            provenance: p.clone(),
        },
        Membership {
            member: Subject::Group(EntityKey::new("app", "two")),
            group: EntityKey::new("app", "one"),
            provenance: p.clone(),
        },
    ];
    for (id, subject) in [
        ("group", Subject::Group(EntityKey::new("app", "two"))),
        (
            "direct",
            Subject::Account(EntityKey::new("app", "unmatched")),
        ),
        ("active", Subject::Account(EntityKey::new("app", "active"))),
    ] {
        app.grants.push(Grant {
            id: id.into(),
            subject,
            resource: EntityKey::new("app", "repo"),
            role: "custom-reader".into(),
            privilege: Privilege::Standard,
            certainty: Certainty::Inferred,
            provenance: Provenance {
                method: "provider grant endpoint".into(),
                ..p.clone()
            },
        });
    }
    let result = query(&[directory.clone(), app.clone()], &Aliases::new());
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.access.len(), 2);
    assert_eq!(result.access[0].grant.id, "direct");
    let grouped = &result.access[1];
    assert_eq!(grouped.grant.id, "group");
    assert_eq!(
        grouped
            .groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_eq!(grouped.memberships.len(), 2);
    assert_eq!(
        grouped.memberships[0].provenance.method,
        "provider membership endpoint"
    );
    assert_eq!(grouped.grant.provenance.method, "provider grant endpoint");
    assert_eq!(grouped.grant.provenance.observed_at, "2026-01-01T00:00:00Z");
    assert_eq!(grouped.grant.role, "custom-reader");
    assert_eq!(grouped.grant.certainty, Certainty::Inferred);
    app.accounts.reverse();
    app.groups.reverse();
    app.grants.reverse();
    app.memberships.reverse();
    let reversed = query(&[app, directory], &Aliases::new());
    assert_eq!(format!("{result:?}"), format!("{reversed:?}"));
}
#[test]
fn authority_names_are_sorted_and_deduplicated() {
    let result = query_orphaned(
        &[Snapshot::new("z"), Snapshot::new("a")],
        &Aliases::new(),
        &["z".into(), "a".into(), "z".into()],
    )
    .unwrap();
    assert_eq!(result.authorities, ["a", "z"]);
    assert!(result.authority_complete);
}

#[test]
fn inactive_service_and_external_override_kind_and_ambiguous_bot_stays_ambiguous() {
    let mut directory = Snapshot::new("directory");
    directory.identities = vec![
        identity(
            "inactive-service",
            IdentityKind::Service,
            IdentityStatus::Inactive,
        ),
        identity(
            "inactive-external",
            IdentityKind::External,
            IdentityStatus::Inactive,
        ),
        identity("ambiguous-bot", IdentityKind::Bot, IdentityStatus::Inactive),
    ];
    let mut app = Snapshot::new("app");
    app.accounts = vec![
        account("inactive-service", IdentityKind::Service),
        account("inactive-external", IdentityKind::External),
        account("ambiguous-bot", IdentityKind::Bot),
    ];
    let aliases = [(
        "other-owner".into(),
        [("app".into(), vec!["ambiguous-bot".into()])].into(),
    )]
    .into();
    let result = query(&[directory, app], &aliases);
    let reasons: Vec<_> = result
        .accounts
        .iter()
        .map(|a| (a.account.key.id.as_str(), a.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            ("ambiguous-bot", OrphanReason::AmbiguousIdentity),
            ("inactive-external", OrphanReason::InactiveIdentity),
            ("inactive-service", OrphanReason::InactiveIdentity),
        ]
    );
    assert!(
        matches!(&result.accounts[0].identity, IdentityResolution::Ambiguous { candidates } if candidates == &["ambiguous-bot", "other-owner"])
    );
}

#[test]
fn exponentially_branching_memberships_fail_with_path_limit_instead_of_truncating() {
    let mut app = Snapshot::new("app");
    app.accounts.push(account("unmatched", IdentityKind::Human));
    let provenance = Provenance {
        method: "provider membership endpoint".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    let mut previous = vec![Subject::Account(app.accounts[0].key.clone())];
    // Only 34 groups, but more than 100,000 traversal steps through their diamonds.
    // The graph depth remains safely below the independent depth bound.
    for level in 0..17 {
        let mut next = Vec::new();
        for branch in 0..2 {
            let key = EntityKey::new("app", format!("{level}-{branch}"));
            app.groups.push(Group {
                key: key.clone(),
                name: key.id.clone(),
            });
            for member in &previous {
                app.memberships.push(Membership {
                    member: member.clone(),
                    group: key.clone(),
                    provenance: provenance.clone(),
                });
            }
            next.push(Subject::Group(key));
        }
        previous = next;
    }
    app.resources.push(Resource {
        key: EntityKey::new("app", "r"),
        name: "Resource".into(),
    });
    app.grants.push(Grant {
        id: "g".into(),
        subject: previous[0].clone(),
        resource: EntityKey::new("app", "r"),
        role: "read".into(),
        privilege: Privilege::Standard,
        certainty: Certainty::Observed,
        provenance,
    });
    app.validate().unwrap();
    assert!(matches!(
        query_orphaned(
            &[Snapshot::new("directory"), app],
            &Aliases::new(),
            &["directory".into()]
        ),
        Err(DomainError::PathLimit)
    ));
}

#[test]
fn more_than_a_hundred_thousand_accounts_without_grants_remain_reviewable() {
    let mut app = Snapshot::new("app");
    for n in 0..100_001 {
        app.accounts
            .push(account(&format!("unmatched-{n:06}"), IdentityKind::Human));
    }
    let result = query(&[Snapshot::new("directory"), app], &Aliases::new());
    assert!(result.authority_complete);
    assert_eq!(result.accounts.len(), 100_001);
    assert!(result.access.is_empty());
    for (n, entry) in result.accounts.iter().enumerate() {
        assert_eq!(
            entry.account.key,
            EntityKey::new("app", format!("unmatched-{n:06}"))
        );
        assert_eq!(entry.reason, OrphanReason::UnknownIdentity);
        assert!(matches!(entry.identity, IdentityResolution::Unmapped));
    }
}
