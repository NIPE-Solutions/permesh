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
fn group(id: &str) -> ExpectedEntry {
    ExpectedEntry::Group {
        instance: "demo".into(),
        group: id.into(),
    }
}
fn input(profiles: &[(&str, Vec<ExpectedEntry>)]) -> ProfileInput {
    ProfileInput {
        profiles: profiles
            .iter()
            .map(|(name, entries)| (name.to_string(), entries.iter().cloned().collect()))
            .collect(),
        people: BTreeMap::new(),
        directory_groups: vec![],
        observable: observable(),
    }
}
fn backend() -> Vec<ExpectedEntry> {
    vec![team(), role("Read", "repo-21")]
}
fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}
fn join(identity: &str, after: &[&str]) -> PlanRequest {
    PlanRequest {
        identity: identity.into(),
        after: set(after),
        removed: BTreeSet::new(),
        leave: false,
    }
}
fn plan(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    input: &ProfileInput,
    request: &PlanRequest,
) -> PlanResult {
    plan_access(snapshots, aliases, input, request).unwrap()
}
fn operations(result: &PlanResult) -> Vec<&Operation> {
    result.steps.iter().map(|s| &s.operation).collect()
}
fn add_team() -> Operation {
    Operation::AddMembership {
        instance: "demo".into(),
        group: "team-10".into(),
    }
}
fn add_read() -> Operation {
    Operation::AddRole {
        instance: "demo".into(),
        role: "Read".into(),
        resource: "repo-21".into(),
    }
}

#[test]
fn join_adds_unsatisfied_entries_only() {
    let input = input(&[("backend", backend())]);
    let result = plan(
        &[fixture()],
        &Aliases::new(),
        &input,
        &join(BOB, &["backend"]),
    );
    assert_eq!(operations(&result), vec![&add_team(), &add_read()]);
    for step in &result.steps {
        assert_eq!(step.state, EntryState::Finding);
        assert_eq!(step.reason, "not_observed");
        assert_eq!(
            step.justification,
            Justification::Profiles(vec!["backend".into()])
        );
        assert_eq!(step.needs_account, None);
    }
    assert_eq!(result.state, ComparisonState::Compared);
    assert_eq!(result.accounts, vec![key("101")]);
    assert!(result.complete);
    let result = plan(
        &[fixture()],
        &Aliases::new(),
        &input,
        &join(ALICE, &["backend"]),
    );
    assert!(result.steps.is_empty(), "{:?}", result.steps);
    assert!(result.complete);
}

#[test]
fn unknown_identity_gets_account_needed_with_prerequisites() {
    let input = input(&[("backend", backend())]);
    let result = plan(
        &[fixture()],
        &Aliases::new(),
        &input,
        &join("carol@example.com", &["backend"]),
    );
    assert_eq!(
        operations(&result),
        vec![
            &Operation::AccountNeeded {
                instance: "demo".into()
            },
            &add_team(),
            &add_read()
        ]
    );
    assert_eq!(result.steps[0].needs_account, None);
    assert_eq!(result.steps[0].reason, "no_mapped_account");
    for step in &result.steps[1..] {
        assert_eq!(step.needs_account.as_deref(), Some("demo"));
        assert_eq!(step.reason, "no_mapped_account");
        assert_eq!(step.state, EntryState::Finding);
    }
    assert_eq!(result.state, ComparisonState::NotFound);
    assert!(result.complete);
}

#[test]
fn partial_instance_add_steps_are_cannot_tell() {
    let mut input = input(&[("backend", backend())]);
    input.observable.clear();
    let result = plan(
        &[fixture()],
        &Aliases::new(),
        &input,
        &join(BOB, &["backend"]),
    );
    assert_eq!(result.steps.len(), 2);
    for step in &result.steps {
        assert_eq!(step.state, EntryState::CannotTell);
        assert_eq!(step.reason, "instance_not_observable");
    }
    assert!(!result.complete);
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
fn ambiguous_account_makes_steps_cannot_tell() {
    let input = input(&[("backend", backend())]);
    let result = plan(
        &[fixture()],
        &bob_claimed_by_alice(),
        &input,
        &join(BOB, &["backend"]),
    );
    assert_eq!(operations(&result), vec![&add_team(), &add_read()]);
    for step in &result.steps {
        assert_eq!(step.state, EntryState::CannotTell);
        assert_eq!(step.reason, "ambiguous_account");
        assert_eq!(step.needs_account, None);
    }
    assert!(!result.complete);
}

#[test]
fn unknown_profile_is_identifier_error() {
    let input = input(&[("backend", backend())]);
    assert!(matches!(
        plan_access(&[fixture()], &Aliases::new(), &input, &join(BOB, &["nope"])),
        Err(DomainError::Identifier)
    ));
    let mut request = join(BOB, &["backend"]);
    request.removed = set(&["nope"]);
    assert!(matches!(
        plan_access(&[fixture()], &Aliases::new(), &input, &request),
        Err(DomainError::Identifier)
    ));
}
