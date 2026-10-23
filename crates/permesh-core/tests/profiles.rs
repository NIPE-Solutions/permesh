// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;
use std::collections::{BTreeMap, BTreeSet};

const ALICE: &str = "alice@example.com";
const BOB: &str = "bob@example.com";

fn provenance() -> Provenance {
    Provenance {
        method: "fixture".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    }
}
fn key(id: &str) -> EntityKey {
    EntityKey::new("demo", id)
}
fn person(s: &mut Snapshot, id: &str, login: &str, email: &str, kind: IdentityKind) {
    s.identities.push(Identity {
        id: email.into(),
        kind,
        affiliation: Affiliation::Internal,
        status: IdentityStatus::Active,
        verified_emails: vec![email.into()],
    });
    s.accounts.push(Account {
        key: key(id),
        login: login.into(),
        kind,
        affiliation: Affiliation::Internal,
        status: IdentityStatus::Active,
        verified_emails: vec![email.into()],
    });
}
fn member(s: &mut Snapshot, member: Subject, group: &str) {
    s.memberships.push(Membership {
        member,
        group: key(group),
        provenance: provenance(),
    });
}
fn grant(s: &mut Snapshot, id: &str, subject: Subject, resource: &str, role: &str) {
    s.grants.push(Grant {
        id: id.into(),
        subject,
        resource: key(resource),
        role: role.into(),
        privilege: if role == "Admin" {
            Privilege::Admin
        } else {
            Privilege::Standard
        },
        certainty: Certainty::Observed,
        evidence_kind: EvidenceKind::Permission,
        provenance: provenance(),
    });
}
/// Demo-shaped: alice (100) in team-10; team-10 Write repo-20; alice Read repo-21;
/// bob (101) Admin repo-21; contractor and former unassigned humans; a bot.
fn fixture() -> Snapshot {
    let mut s = Snapshot::new("demo");
    person(&mut s, "100", "alice-dev", ALICE, IdentityKind::Human);
    person(&mut s, "101", "bob-admin", BOB, IdentityKind::Human);
    person(
        &mut s,
        "102",
        "former-dev",
        "former@example.com",
        IdentityKind::Human,
    );
    person(
        &mut s,
        "103",
        "build-bot",
        "build@example.com",
        IdentityKind::Bot,
    );
    person(
        &mut s,
        "104",
        "contractor-dev",
        "contractor@example.com",
        IdentityKind::Human,
    );
    for (id, name) in [("repo-20", "acme/payments-api"), ("repo-21", "acme/infra")] {
        s.resources.push(Resource {
            key: key(id),
            name: name.into(),
            kind: None,
            parent: None,
        });
    }
    s.groups.push(Group {
        key: key("team-10"),
        name: "team/backend".into(),
    });
    member(&mut s, Subject::Account(key("100")), "team-10");
    grant(
        &mut s,
        "grant-1",
        Subject::Group(key("team-10")),
        "repo-20",
        "Write",
    );
    grant(
        &mut s,
        "grant-2",
        Subject::Account(key("100")),
        "repo-21",
        "Read",
    );
    grant(
        &mut s,
        "grant-3",
        Subject::Account(key("101")),
        "repo-21",
        "Admin",
    );
    grant(
        &mut s,
        "grant-5",
        Subject::Account(key("103")),
        "repo-20",
        "Write",
    );
    s
}
fn team() -> ExpectedEntry {
    ExpectedEntry::Group {
        instance: "demo".into(),
        group: "team-10".into(),
    }
}
fn role(role: &str, resource: &str) -> ExpectedEntry {
    ExpectedEntry::Role {
        instance: "demo".into(),
        role: role.into(),
        resource: resource.into(),
    }
}
fn observable() -> BTreeMap<String, BTreeSet<String>> {
    let caps = [
        "accounts",
        "grants",
        "groups",
        "identities",
        "memberships",
        "resources",
    ];
    [(
        "demo".to_string(),
        caps.iter().map(|c| c.to_string()).collect(),
    )]
    .into()
}
fn input(profile: Vec<ExpectedEntry>, people: &[&str]) -> ProfileInput {
    ProfileInput {
        profiles: [("backend".to_string(), profile.into_iter().collect())].into(),
        people: people
            .iter()
            .map(|p| (p.to_string(), ["backend".to_string()].into()))
            .collect(),
        directory_groups: vec![],
        left: BTreeSet::new(),
        observable: observable(),
    }
}
fn compare(snapshots: &[Snapshot], input: &ProfileInput) -> ProfileComparison {
    compare_profiles(snapshots, &Aliases::new(), input).unwrap()
}
fn of<'a>(result: &'a ProfileComparison, id: &str) -> &'a IdentityComparison {
    result.identities.iter().find(|i| i.identity == id).unwrap()
}
fn backend() -> Vec<ExpectedEntry> {
    vec![team(), role("Read", "repo-21")]
}

#[test]
fn assigned_member_is_fully_explained() {
    let result = compare(&[fixture()], &input(backend(), &[ALICE]));
    let alice = of(&result, ALICE);
    assert_eq!(alice.state, ComparisonState::Compared);
    assert_eq!(alice.explained, 2);
    assert!(alice.missing.is_empty() && alice.unexplained.is_empty());
    assert_eq!(alice.profiles, vec!["backend".to_string()]);
    assert!(result.complete);
}

#[test]
fn group_derived_access_is_explained_by_the_group_entry() {
    let result = compare(&[fixture()], &input(vec![team()], &[ALICE]));
    let alice = of(&result, ALICE);
    assert_eq!(alice.explained, 1);
    assert_eq!(alice.unexplained.len(), 1);
    assert_eq!(alice.unexplained[0].path.grant.id, "grant-2");
}

#[test]
fn non_member_gets_missing_membership_and_unexplained_direct_role() {
    let result = compare(&[fixture()], &input(backend(), &[BOB]));
    let bob = of(&result, BOB);
    let missing: Vec<_> = bob
        .missing
        .iter()
        .map(|m| (&m.entry, m.state, m.reason))
        .collect();
    assert_eq!(
        missing,
        vec![
            (&team(), EntryState::Finding, "not_observed"),
            (
                &role("Read", "repo-21"),
                EntryState::Finding,
                "not_observed"
            ),
        ]
    );
    assert_eq!(bob.unexplained.len(), 1);
    assert_eq!(bob.unexplained[0].path.grant.id, "grant-3");
    assert_eq!(bob.unexplained[0].state, EntryState::Finding);
}

#[test]
fn different_role_is_unexplained_and_missing() {
    let result = compare(
        &[fixture()],
        &input(vec![team(), role("Write", "repo-21")], &[ALICE]),
    );
    let alice = of(&result, ALICE);
    assert_eq!(alice.unexplained[0].path.grant.role, "Read");
    assert_eq!(alice.missing[0].entry, role("Write", "repo-21"));
}

#[test]
fn partial_instance_is_cannot_tell_not_missing() {
    let mut input = input(backend(), &[BOB]);
    input.observable.clear();
    let result = compare(&[fixture()], &input);
    let bob = of(&result, BOB);
    assert_eq!(bob.missing.len(), 2);
    assert!(
        bob.missing
            .iter()
            .all(|m| m.state == EntryState::CannotTell && m.reason == "instance_not_observable")
    );
    assert!(!result.complete);
}

#[test]
fn missing_account_in_instance_is_reported_as_such() {
    let mut s = fixture();
    s.accounts.retain(|a| a.key.id != "101");
    s.grants.retain(|g| g.id != "grant-3");
    let result = compare(&[s], &input(backend(), &[BOB]));
    assert!(
        of(&result, BOB)
            .missing
            .iter()
            .all(|m| m.reason == "no_mapped_account")
    );
}

#[test]
fn directory_group_assigns_members() {
    let mut input = input(backend(), &[]);
    input.directory_groups.push(DirectoryAssignment {
        instance: "demo".into(),
        group: "team-10".into(),
        profiles: vec!["backend".into()],
    });
    let result = compare(&[fixture()], &input);
    let ids: Vec<_> = result
        .identities
        .iter()
        .map(|i| i.identity.as_str())
        .collect();
    assert_eq!(ids, vec![ALICE]);
    assert!(result.complete);
}

#[test]
fn unobservable_directory_mapping_makes_unexplained_cannot_tell() {
    let mut input = input(backend(), &[BOB]);
    input.directory_groups.push(DirectoryAssignment {
        instance: "demo".into(),
        group: "missing-group".into(),
        profiles: vec!["backend".into()],
    });
    let result = compare(&[fixture()], &input);
    assert_eq!(
        result.unresolved_assignments[0].reason,
        "group_not_observed"
    );
    let bob = of(&result, BOB);
    assert_eq!(bob.unexplained[0].state, EntryState::CannotTell);
    assert_eq!(bob.missing[0].state, EntryState::Finding);
    assert!(!result.complete);
}

#[test]
fn unknown_person_is_not_found_and_incomplete() {
    let result = compare(&[fixture()], &input(backend(), &["ghost@example.com"]));
    let ghost = of(&result, "ghost@example.com");
    assert_eq!(ghost.state, ComparisonState::NotFound);
    assert!(ghost.missing.is_empty() && ghost.unexplained.is_empty());
    assert!(!result.complete);
}

#[test]
fn duplicate_assignment_is_deduplicated() {
    let mut input = input(backend(), &[ALICE]);
    input
        .profiles
        .insert("base".into(), [team()].into_iter().collect());
    input.people.get_mut(ALICE).unwrap().insert("base".into());
    input.directory_groups.push(DirectoryAssignment {
        instance: "demo".into(),
        group: "team-10".into(),
        profiles: vec!["backend".into()],
    });
    let result = compare(&[fixture()], &input);
    assert_eq!(result.identities.len(), 1);
    let alice = of(&result, ALICE);
    assert_eq!(
        alice.profiles,
        vec!["backend".to_string(), "base".to_string()]
    );
    assert_eq!(alice.explained, 2);
}

#[test]
fn membership_cycle_terminates() {
    let mut s = fixture();
    s.groups.push(Group {
        key: key("team-11"),
        name: "team/outer".into(),
    });
    member(&mut s, Subject::Group(key("team-10")), "team-11");
    member(&mut s, Subject::Group(key("team-11")), "team-10");
    let outer = ExpectedEntry::Group {
        instance: "demo".into(),
        group: "team-11".into(),
    };
    let result = compare(&[s], &input(vec![outer, role("Read", "repo-21")], &[ALICE]));
    let alice = of(&result, ALICE);
    assert!(alice.missing.is_empty(), "{:?}", alice.missing);
}

#[test]
fn ambiguous_account_is_not_attributed() {
    let mut s = fixture();
    let mut aliases = Aliases::new();
    // An alias claims bob's account for alice too: two candidates.
    aliases
        .entry(ALICE.to_string())
        .or_default()
        .insert("demo".into(), vec!["101".into()]);
    let result = compare_profiles(
        std::slice::from_mut(&mut s),
        &aliases,
        &input(backend(), &[ALICE, BOB]),
    )
    .unwrap();
    assert_eq!(result.ambiguous_accounts, 1);
    assert!(of(&result, BOB).unexplained.is_empty());
    assert!(
        of(&result, ALICE)
            .unexplained
            .iter()
            .all(|u| u.path.account.id != "101")
    );
}

#[test]
fn unassigned_humans_count_as_not_covered() {
    let result = compare(&[fixture()], &input(backend(), &[ALICE]));
    assert_eq!(result.not_covered, 3);
}

/// Former (102) joins team-10 and also holds Read on repo-21, like alice.
fn two_member_team() -> Snapshot {
    let mut s = fixture();
    member(&mut s, Subject::Account(key("102")), "team-10");
    grant(
        &mut s,
        "grant-9",
        Subject::Account(key("102")),
        "repo-21",
        "Read",
    );
    s
}

#[test]
fn suggest_from_team_includes_team_and_shared_direct_roles() {
    let s = two_member_team();
    let suggestion = suggest_profile(&[s], &Aliases::new(), &key("team-10"), 100).unwrap();
    assert_eq!(suggestion.members, 2);
    let entries: Vec<_> = suggestion
        .entries
        .iter()
        .map(|e| (&e.entry, e.count))
        .collect();
    assert_eq!(entries, vec![(&team(), 2), (&role("Read", "repo-21"), 2)]);
    assert_eq!(suggestion.entries[0].label, "team/backend");
    assert_eq!(suggestion.entries[1].label, "acme/infra");
}

#[test]
fn threshold_filters_entries() {
    let mut s = two_member_team();
    grant(
        &mut s,
        "grant-8",
        Subject::Account(key("100")),
        "repo-20",
        "Admin",
    );
    let at = |pct| {
        suggest_profile(
            std::slice::from_ref(&s),
            &Aliases::new(),
            &key("team-10"),
            pct,
        )
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.entry)
        .collect::<Vec<_>>()
    };
    assert!(!at(100).contains(&role("Admin", "repo-20")));
    assert!(at(50).contains(&role("Admin", "repo-20")));
}

#[test]
fn unresolved_members_are_counted_not_used() {
    let mut s = two_member_team();
    s.accounts.push(Account {
        key: key("200"),
        login: "stranger".into(),
        kind: IdentityKind::Human,
        affiliation: Affiliation::Unknown,
        status: IdentityStatus::Unknown,
        verified_emails: vec![],
    });
    member(&mut s, Subject::Account(key("200")), "team-10");
    let suggestion = suggest_profile(&[s], &Aliases::new(), &key("team-10"), 100).unwrap();
    assert_eq!((suggestion.members, suggestion.unresolved_members), (2, 1));
}

#[test]
fn unknown_group_is_not_found() {
    assert!(matches!(
        suggest_profile(&[fixture()], &Aliases::new(), &key("nope"), 100),
        Err(DomainError::NotFound)
    ));
    assert!(suggest_profile(&[fixture()], &Aliases::new(), &key("team-10"), 0).is_err());
}

fn bob_claimed_by_alice() -> Aliases {
    let mut aliases = Aliases::new();
    aliases
        .entry(ALICE.to_string())
        .or_default()
        .insert("demo".into(), vec!["101".into()]);
    aliases
}

#[test]
fn ambiguous_account_makes_claimants_cannot_tell_and_incomplete() {
    let result = compare_profiles(
        &[fixture()],
        &bob_claimed_by_alice(),
        &input(backend(), &[BOB]),
    )
    .unwrap();
    let bob = of(&result, BOB);
    assert_eq!(bob.missing.len(), 2);
    for missing in &bob.missing {
        assert_eq!(missing.state, EntryState::CannotTell);
        assert_eq!(missing.reason, "ambiguous_account");
    }
    assert!(!result.complete);
    // Even with nothing expected, hidden access means the comparison cannot be clean.
    let result = compare_profiles(
        &[fixture()],
        &bob_claimed_by_alice(),
        &input(vec![], &[BOB]),
    )
    .unwrap();
    assert!(!result.complete);
}

#[test]
fn unresolved_directory_member_is_counted_once_and_incomplete() {
    let mut s = fixture();
    s.groups.push(Group {
        key: key("team-11"),
        name: "team/ops".into(),
    });
    member(&mut s, Subject::Account(key("101")), "team-10");
    member(&mut s, Subject::Account(key("101")), "team-11");
    let mut input = input(backend(), &[]);
    for group in ["team-10", "team-11"] {
        input.directory_groups.push(DirectoryAssignment {
            instance: "demo".into(),
            group: group.into(),
            profiles: vec!["backend".into()],
        });
    }
    let result = compare_profiles(&[s], &bob_claimed_by_alice(), &input).unwrap();
    assert_eq!(result.unresolved_members, 1);
    assert!(!result.complete);
}
