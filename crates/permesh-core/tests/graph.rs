// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;
#[test]
fn empty_snapshot_is_valid() {
    assert!(Snapshot::new("demo").validate().is_ok());
}
#[test]
fn cross_provider_account_is_rejected() {
    let mut snapshot = Snapshot::new("demo");
    snapshot.accounts.push(Account {
        key: EntityKey::new("other", "1"),
        login: "alice".into(),
        kind: IdentityKind::Human,
        verified_emails: vec![],
    });
    assert!(snapshot.validate().is_err());
}
fn fixture() -> Snapshot {
    let mut s = Snapshot::new("demo");
    s.accounts.push(Account {
        key: EntityKey::new("demo", "1"),
        login: "alice-dev".into(),
        kind: IdentityKind::Human,
        verified_emails: vec![],
    });
    s.identities.push(Identity {
        id: "alice@example.com".into(),
        kind: IdentityKind::Human,
        status: IdentityStatus::Active,
        verified_emails: vec!["alice@example.com".into()],
    });
    s.resources.push(Resource {
        key: EntityKey::new("demo", "r"),
        name: "acme/api".into(),
    });
    for id in ["a", "b"] {
        s.groups.push(Group {
            key: EntityKey::new("demo", id),
            name: format!("team/{id}"),
        });
    }
    let p = Provenance {
        method: "fixture".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    s.memberships.push(Membership {
        member: Subject::Account(s.accounts[0].key.clone()),
        group: s.groups[0].key.clone(),
        provenance: p.clone(),
    });
    s.memberships.push(Membership {
        member: Subject::Group(s.groups[0].key.clone()),
        group: s.groups[1].key.clone(),
        provenance: p.clone(),
    });
    s.memberships.push(Membership {
        member: Subject::Group(s.groups[1].key.clone()),
        group: s.groups[0].key.clone(),
        provenance: p.clone(),
    });
    s.grants.push(Grant {
        id: "g".into(),
        subject: Subject::Group(s.groups[1].key.clone()),
        resource: s.resources[0].key.clone(),
        role: "write".into(),
        privilege: Privilege::Standard,
        certainty: Certainty::Observed,
        provenance: p,
    });
    s
}
fn aliases() -> Aliases {
    [(
        "alice@example.com".into(),
        [("demo".into(), vec!["1".into()])].into(),
    )]
    .into()
}
#[test]
fn alias_preserves_nested_path_and_terminates_cycles() {
    let result = query_user(&[fixture()], &aliases(), "alice@example.com").unwrap();
    assert_eq!(result.access.len(), 1);
    assert_eq!(
        result.access[0]
            .groups
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        vec!["team/a", "team/b"]
    );
    assert_eq!(result.access[0].memberships.len(), 2);
}
#[test]
fn login_without_verified_email_does_not_silently_match_identity() {
    let result = query_user(&[fixture()], &Aliases::new(), "alice-dev").unwrap();
    assert!(result.identity.is_none());
    assert!(
        matches!(query_user(&[fixture()],&Aliases::new(),"alice@example.com"),Ok(r) if r.accounts.is_empty())
    );
}
#[test]
fn conflicting_verified_identities_remain_ambiguous() {
    let mut s = fixture();
    s.accounts[0]
        .verified_emails
        .push("alice@example.com".into());
    s.identities.push(Identity {
        id: "someone-else".into(),
        kind: IdentityKind::Human,
        status: IdentityStatus::Active,
        verified_emails: vec!["alice@example.com".into()],
    });
    assert!(matches!(
        query_user(&[s], &Aliases::new(), "alice-dev"),
        Err(DomainError::Ambiguous)
    ));
}
#[test]
fn snapshot_order_does_not_change_access_order() {
    let a = fixture();
    let mut b = a.clone();
    b.memberships.reverse();
    b.groups.reverse();
    let x = query_user(&[a], &aliases(), "alice@example.com").unwrap();
    let y = query_user(&[b], &aliases(), "alice@example.com").unwrap();
    assert_eq!(x.access[0].groups[0].key, y.access[0].groups[0].key);
}
#[test]
fn unknown_identity_is_not_found() {
    assert!(matches!(
        query_user(&[fixture()], &Aliases::new(), "stranger"),
        Err(DomainError::NotFound)
    ));
}
#[test]
fn matching_authorities_union_verified_emails_deterministically() {
    let mut a = fixture();
    a.identities[0].verified_emails = vec!["z@example.com".into(), "a@example.com".into()];
    let mut b = Snapshot::new("authority-two");
    b.identities = a.identities.clone();
    b.identities[0].verified_emails = vec!["b@example.com".into()];
    for snapshots in [vec![a.clone(), b.clone()], vec![b, a]] {
        let result = query_user(&snapshots, &Aliases::new(), "alice@example.com").unwrap();
        assert_eq!(
            result.identity.unwrap().verified_emails,
            vec!["a@example.com", "b@example.com", "z@example.com"]
        );
    }
}
#[test]
fn conflicting_authority_status_is_ambiguous_in_both_orders() {
    let a = fixture();
    let mut b = Snapshot::new("authority-two");
    b.identities = a.identities.clone();
    b.identities[0].status = IdentityStatus::Inactive;
    for snapshots in [vec![a.clone(), b.clone()], vec![b, a]] {
        assert!(matches!(
            query_user(&snapshots, &aliases(), "alice@example.com"),
            Err(DomainError::Ambiguous)
        ));
    }
}
#[test]
fn explicit_alias_cannot_override_conflicting_verified_authority() {
    let mut s = fixture();
    s.accounts[0].verified_emails = vec!["alice@example.com".into()];
    let aliases = [(
        "other-identity".into(),
        [("demo".into(), vec!["1".into()])].into(),
    )]
    .into();
    assert!(matches!(
        query_user(&[s], &aliases, "other-identity"),
        Err(DomainError::Ambiguous)
    ));
}
#[test]
fn invalid_or_non_utc_observation_times_are_rejected() {
    for timestamp in [
        "",
        "not-a-date",
        "2026-02-30T00:00:00Z",
        "2026-01-01T00:00:00+02:00",
    ] {
        let mut s = fixture();
        s.grants[0].provenance.observed_at = timestamp.into();
        assert!(s.validate().is_err(), "{timestamp}");
        let mut s = fixture();
        s.memberships[0].provenance.observed_at = timestamp.into();
        assert!(s.validate().is_err(), "{timestamp}");
    }
}
#[test]
fn diamond_preserves_both_paths_through_cycle() {
    let mut s = fixture();
    let p = s.memberships[0].provenance.clone();
    s.groups.push(Group {
        key: EntityKey::new("demo", "c"),
        name: "team/c".into(),
    });
    s.memberships.push(Membership {
        member: Subject::Account(s.accounts[0].key.clone()),
        group: s.groups[2].key.clone(),
        provenance: p.clone(),
    });
    s.memberships.push(Membership {
        member: Subject::Group(s.groups[2].key.clone()),
        group: s.groups[1].key.clone(),
        provenance: p,
    });
    let result = query_user(&[s], &aliases(), "alice@example.com").unwrap();
    assert_eq!(result.access.len(), 2);
    assert_eq!(
        result
            .access
            .iter()
            .map(|p| p
                .groups
                .iter()
                .map(|g| g.key.id.as_str())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec!["a", "b"], vec!["c", "b"]]
    );
}
#[test]
fn same_login_and_native_id_across_providers_remain_scoped() {
    let a = fixture();
    let mut b = Snapshot::new("second");
    let mut account = a.accounts[0].clone();
    account.key.provider = "second".into();
    b.accounts.push(account);
    assert!(matches!(
        query_user(&[a.clone(), b.clone()], &Aliases::new(), "alice-dev"),
        Err(DomainError::Ambiguous)
    ));
    let result = query_user(&[a, b], &Aliases::new(), "second:1").unwrap();
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.accounts[0].key.provider, "second");
    assert!(result.access.is_empty());
}
#[test]
fn deep_graph_fails_explicitly_instead_of_truncating() {
    let mut s = fixture();
    s.memberships.clear();
    s.groups.clear();
    s.grants.clear();
    let p = Provenance {
        method: "fixture".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    let mut member = Subject::Account(s.accounts[0].key.clone());
    for i in 0..257 {
        let key = EntityKey::new("demo", format!("group-{i}"));
        s.groups.push(Group {
            key: key.clone(),
            name: format!("group-{i}"),
        });
        s.memberships.push(Membership {
            member,
            group: key.clone(),
            provenance: p.clone(),
        });
        member = Subject::Group(key);
    }
    assert!(matches!(
        query_user(&[s], &aliases(), "alice@example.com"),
        Err(DomainError::PathLimit)
    ));
}
#[test]
fn duplicate_provider_snapshots_cannot_overwrite_graph_records() {
    let a = fixture();
    let mut b = Snapshot::new("demo");
    b.resources = a.resources.clone();
    b.resources[0].name = "conflicting resource".into();
    assert!(matches!(
        query_user(&[a, b], &aliases(), "alice@example.com"),
        Err(DomainError::Identifier)
    ));
}
#[test]
fn returned_account_emails_are_sorted_and_unique() {
    let mut s = fixture();
    s.accounts[0].verified_emails = vec![
        "z@example.com".into(),
        "a@example.com".into(),
        "a@example.com".into(),
    ];
    let result = query_user(&[s], &Aliases::new(), "alice-dev").unwrap();
    assert_eq!(
        result.accounts[0].verified_emails,
        vec!["a@example.com", "z@example.com"]
    );
}

#[test]
fn admins_separates_all_levels_and_preserves_native_roles() {
    let mut s = fixture();
    s.grants.clear();
    for (i, privilege) in [
        Privilege::Standard,
        Privilege::Elevated,
        Privilege::Admin,
        Privilege::Owner,
        Privilege::Unknown,
    ]
    .into_iter()
    .enumerate()
    {
        s.grants.push(Grant {
            id: i.to_string(),
            subject: Subject::Account(s.accounts[0].key.clone()),
            resource: s.resources[0].key.clone(),
            role: format!("native-{i}"),
            privilege,
            certainty: Certainty::Inferred,
            provenance: s.memberships[0].provenance.clone(),
        });
    }
    let r = query_admins(&[s], &Aliases::new()).unwrap();
    assert_eq!(
        r.access
            .iter()
            .map(|p| p.grant.role.as_str())
            .collect::<Vec<_>>(),
        vec!["native-1", "native-2", "native-3"]
    );
    assert_eq!(r.unknown_access.len(), 1);
    assert_eq!(r.unknown_access[0].grant.certainty, Certainty::Inferred);
    assert!(matches!(
        r.accounts[0].identity,
        IdentityResolution::Unmapped
    ));
}

#[test]
fn admins_retains_diamond_cycle_paths_and_unresolved_groups() {
    let mut s = fixture();
    s.grants[0].privilege = Privilege::Admin;
    s.memberships.push(Membership {
        member: Subject::Account(s.accounts[0].key.clone()),
        group: s.groups[1].key.clone(),
        provenance: s.memberships[0].provenance.clone(),
    });
    s.groups.push(Group {
        key: EntityKey::new("demo", "empty"),
        name: "unobserved".into(),
    });
    let mut grant = s.grants[0].clone();
    grant.id = "unresolved".into();
    grant.subject = Subject::Group(s.groups[2].key.clone());
    grant.privilege = Privilege::Unknown;
    s.grants.push(grant);
    let r = query_admins(std::slice::from_ref(&s), &aliases()).unwrap();
    assert_eq!(r.access.len(), 2);
    assert_eq!(r.access[0].groups.len(), 2);
    assert_eq!(r.access[0].memberships.len(), 2);
    assert_eq!(r.access[1].groups.len(), 1);
    assert_eq!(r.unresolved_grants.len(), 1);
    assert_eq!(
        r.unresolved_grants[0].group.as_ref().unwrap().name,
        "unobserved"
    );
    assert!(
        matches!(&r.accounts[0].identity, IdentityResolution::Resolved { identity } if identity.id == "alice@example.com")
    );
    s.memberships.reverse();
    s.grants.reverse();
    s.groups.reverse();
    let other = query_admins(&[s], &aliases()).unwrap();
    assert_eq!(format!("{r:?}"), format!("{other:?}"));
}

#[test]
fn admins_keeps_ambiguous_service_accounts_and_conflicting_authorities() {
    let mut s = fixture();
    s.accounts[0].kind = IdentityKind::Service;
    s.accounts[0].verified_emails = vec!["alice@example.com".into()];
    s.grants[0].privilege = Privilege::Owner;
    let conflicting_aliases = [("other".into(), [("demo".into(), vec!["1".into()])].into())].into();
    let r = query_admins(std::slice::from_ref(&s), &conflicting_aliases).unwrap();
    assert_eq!(r.accounts[0].account.kind, IdentityKind::Service);
    assert!(
        matches!(&r.accounts[0].identity, IdentityResolution::Ambiguous { candidates } if candidates == &vec!["alice@example.com".to_string(), "other".to_string()])
    );
    let mut authority = Snapshot::new("authority");
    authority.identities = s.identities.clone();
    authority.identities[0].status = IdentityStatus::Inactive;
    for snapshots in [vec![s.clone(), authority.clone()], vec![authority, s]] {
        let r = query_admins(&snapshots, &Aliases::new()).unwrap();
        assert_eq!(r.access.len(), 1);
        assert!(
            matches!(&r.accounts[0].identity, IdentityResolution::Ambiguous { candidates } if candidates == &vec!["alice@example.com".to_string()])
        );
    }
}

#[test]
fn admins_prunes_fifty_thousand_irrelevant_accounts_and_standard_paths() {
    let mut s = fixture();
    s.grants[0].privilege = Privilege::Admin;
    s.groups.push(Group {
        key: EntityKey::new("demo", "irrelevant"),
        name: "irrelevant".into(),
    });
    for i in 0..50_000 {
        let key = EntityKey::new("demo", format!("irrelevant-{i}"));
        s.accounts.push(Account {
            key: key.clone(),
            login: format!("service-{i}"),
            kind: IdentityKind::Bot,
            verified_emails: vec![],
        });
        s.memberships.push(Membership {
            member: Subject::Account(key),
            group: s.groups[2].key.clone(),
            provenance: s.memberships[0].provenance.clone(),
        });
    }
    // Irrelevant depth must not consume the privileged traversal budget.
    let mut member = Subject::Group(s.groups[2].key.clone());
    for i in 0..300 {
        let key = EntityKey::new("demo", format!("deep-{i}"));
        s.groups.push(Group {
            key: key.clone(),
            name: key.id.clone(),
        });
        s.memberships.push(Membership {
            member,
            group: key.clone(),
            provenance: s.memberships[0].provenance.clone(),
        });
        member = Subject::Group(key);
    }
    let mut standard = s.grants[0].clone();
    standard.id = "irrelevant-standard".into();
    standard.subject = member;
    standard.privilege = Privilege::Standard;
    s.grants.push(standard);
    let r = query_admins(&[s], &Aliases::new()).unwrap();
    assert_eq!(r.accounts.len(), 1);
    assert_eq!(r.access.len(), 1);
}

#[test]
fn admins_validates_even_when_no_privileged_grants_exist() {
    let s = fixture();
    let r = query_admins(std::slice::from_ref(&s), &aliases()).unwrap();
    assert!(
        r.accounts.is_empty()
            && r.access.is_empty()
            && r.unknown_access.is_empty()
            && r.unresolved_grants.is_empty()
    );
    assert!(matches!(
        query_admins(&[s.clone(), s], &Aliases::new()),
        Err(DomainError::Identifier)
    ));
}

#[test]
fn admins_scopes_identical_native_ids_and_sorts_snapshot_order() {
    let mut a = fixture();
    a.grants[0].privilege = Privilege::Elevated;
    let mut b = Snapshot::new("second");
    let mut account = a.accounts[0].clone();
    account.key.provider = "second".into();
    account.kind = IdentityKind::Bot;
    b.accounts.push(account);
    let mut resource = a.resources[0].clone();
    resource.key.provider = "second".into();
    b.resources.push(resource);
    let mut grant = a.grants[0].clone();
    grant.subject = Subject::Account(b.accounts[0].key.clone());
    grant.resource = b.resources[0].key.clone();
    grant.privilege = Privilege::Unknown;
    b.grants.push(grant);
    let x = query_admins(&[a.clone(), b.clone()], &Aliases::new()).unwrap();
    let y = query_admins(&[b, a], &Aliases::new()).unwrap();
    assert_eq!(format!("{x:?}"), format!("{y:?}"));
    assert_eq!(x.accounts.len(), 2);
    assert_eq!(x.access[0].account.provider, "demo");
    assert_eq!(x.unknown_access[0].account.provider, "second");
    assert_eq!(x.accounts[1].account.kind, IdentityKind::Bot);
}

#[test]
fn admins_relevant_deep_graph_fails_explicitly() {
    let mut s = fixture();
    s.memberships.clear();
    s.groups.clear();
    let mut member = Subject::Account(s.accounts[0].key.clone());
    for i in 0..257 {
        let key = EntityKey::new("demo", format!("deep-{i}"));
        s.groups.push(Group {
            key: key.clone(),
            name: key.id.clone(),
        });
        s.memberships.push(Membership {
            member,
            group: key.clone(),
            provenance: s.grants[0].provenance.clone(),
        });
        member = Subject::Group(key);
    }
    s.grants[0].subject = member;
    s.grants[0].privilege = Privilege::Admin;
    assert!(matches!(
        query_admins(&[s], &Aliases::new()),
        Err(DomainError::PathLimit)
    ));
}

#[test]
fn admins_direct_grants_fail_explicitly_above_output_limit() {
    let mut s = fixture();
    let mut grant = s.grants.remove(0);
    grant.subject = Subject::Account(s.accounts[0].key.clone());
    grant.privilege = Privilege::Admin;
    s.memberships.clear();
    s.groups.clear();
    s.grants = (0..100_001)
        .map(|id| Grant {
            id: id.to_string(),
            ..grant.clone()
        })
        .collect();
    assert!(matches!(
        query_admins(&[s], &Aliases::new()),
        Err(DomainError::PathLimit)
    ));
}

#[test]
fn admins_observed_grant_does_not_hide_same_native_id_in_another_provider() {
    let mut a = fixture();
    a.grants[0].privilege = Privilege::Admin;
    let mut b = Snapshot::new("second");
    b.resources.push(Resource {
        key: EntityKey::new("second", "r"),
        name: "other resource".into(),
    });
    b.groups.push(Group {
        key: EntityKey::new("second", "b"),
        name: "unobserved group".into(),
    });
    b.grants.push(Grant {
        subject: Subject::Group(b.groups[0].key.clone()),
        resource: b.resources[0].key.clone(),
        ..a.grants[0].clone()
    });
    for snapshots in [vec![a.clone(), b.clone()], vec![b, a]] {
        let r = query_admins(&snapshots, &Aliases::new()).unwrap();
        assert_eq!(r.access.len(), 1);
        assert_eq!(r.access[0].account.provider, "demo");
        assert_eq!(r.unresolved_grants.len(), 1);
        assert_eq!(r.unresolved_grants[0].grant.id, r.access[0].grant.id);
        assert_eq!(r.unresolved_grants[0].resource.key.provider, "second");
        assert_eq!(
            r.unresolved_grants[0].group.as_ref().unwrap().key.provider,
            "second"
        );
    }
}

#[test]
fn admins_rejects_dangling_subjects_and_resources_before_filtering() {
    for privilege in [Privilege::Standard, Privilege::Admin, Privilege::Unknown] {
        let mut s = fixture();
        s.grants[0].privilege = privilege;
        for subject in [
            Subject::Account(EntityKey::new("demo", "missing")),
            Subject::Group(EntityKey::new("demo", "missing")),
        ] {
            let mut missing_subject = s.clone();
            missing_subject.grants[0].subject = subject;
            assert!(matches!(
                query_admins(&[missing_subject], &Aliases::new()),
                Err(DomainError::Reference)
            ));
        }
        s.grants[0].resource = EntityKey::new("demo", "missing");
        assert!(matches!(
            query_admins(&[s], &Aliases::new()),
            Err(DomainError::Reference)
        ));
    }
}
