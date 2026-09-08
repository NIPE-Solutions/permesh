// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
#[path = "../src/schema2.rs"]
mod schema2;
use permesh_core::{Aliases, query_admins, query_orphaned, query_user};
use permesh_provider_sdk::Provider;
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let source = match name {
        "user" => include_str!("fixtures/schema2/user.json"),
        "admins" => include_str!("fixtures/schema2/admins.json"),
        "orphaned" => include_str!("fixtures/schema2/orphaned.json"),
        "snapshot" => include_str!("fixtures/schema2/snapshot.json"),
        _ => unreachable!(),
    };
    serde_json::from_str(source).unwrap()
}

// These complete static fixtures freeze schema 2 independently of core's serde derives.
// Removing/renaming fields, changing enum tags, dropping nulls or reordering arrays fails.
#[tokio::test]
async fn demo_query_and_snapshot_dtos_preserve_schema_two_fixtures() {
    let snapshot = permesh_provider_demo::DemoProvider::new("demo")
        .discover()
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(schema2::Snapshot::from(&snapshot)).unwrap(),
        fixture("snapshot")
    );
    let snapshots = [snapshot];
    let aliases = Aliases::new();
    let user = query_user(&snapshots, &aliases, "alice@example.com").unwrap();
    assert_eq!(
        serde_json::to_value(schema2::UserAccess::from(&user)).unwrap(),
        fixture("user")
    );
    let admins = query_admins(&snapshots, &aliases).unwrap();
    assert_eq!(
        serde_json::to_value(schema2::AdminAccess::from(&admins)).unwrap(),
        fixture("admins")
    );
    let orphaned = query_orphaned(&snapshots, &aliases, &["demo".into()]).unwrap();
    assert_eq!(
        serde_json::to_value(schema2::OrphanedAccess::from(&orphaned)).unwrap(),
        fixture("orphaned")
    );
}

#[test]
fn empty_query_null_identity_and_partial_snapshot_keep_required_fields() {
    let user = permesh_core::UserAccess {
        identity: None,
        accounts: vec![],
        access: vec![],
    };
    assert_eq!(
        serde_json::to_value(schema2::UserAccess::from(&user)).unwrap(),
        serde_json::json!({"identity":null,"accounts":[],"access":[]})
    );
    let mut snapshot = permesh_core::Snapshot::new("empty");
    snapshot.complete = false;
    snapshot.limitations = vec!["Visibility restricted".into()];
    assert_eq!(
        serde_json::to_value(schema2::Snapshot::from(&snapshot)).unwrap(),
        serde_json::json!({"provider":"empty","identities":[],"accounts":[],"resources":[],
            "groups":[],"memberships":[],"grants":[],"complete":false,
            "limitations":["Visibility restricted"]})
    );
}

#[test]
fn resolution_tags_and_unresolved_grant_nulls_remain_schema_two() {
    use permesh_core::*;
    let account = Account {
        key: EntityKey::new("app", "1"),
        login: "login".into(),
        status: IdentityStatus::Unknown,
        kind: IdentityKind::Unknown,
        affiliation: Affiliation::Unknown,
        verified_emails: vec![],
    };
    let identity = Identity {
        id: "person".into(),
        kind: IdentityKind::Unknown,
        affiliation: Affiliation::Unknown,
        status: IdentityStatus::Unknown,
        verified_emails: vec![],
    };
    let grant = Grant {
        id: "g".into(),
        subject: Subject::Group(EntityKey::new("app", "group")),
        resource: EntityKey::new("app", "resource"),
        role: "Custom role".into(),
        privilege: Privilege::Unknown,
        certainty: Certainty::Unknown,
        evidence_kind: EvidenceKind::Unknown,
        provenance: Provenance {
            method: "fixture".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        },
    };
    let result = AdminAccess {
        accounts: vec![
            AdminAccount {
                account: account.clone(),
                identity: IdentityResolution::Resolved { identity },
            },
            AdminAccount {
                account: account.clone(),
                identity: IdentityResolution::Unmapped,
            },
            AdminAccount {
                account,
                identity: IdentityResolution::Ambiguous {
                    candidates: vec!["one-conflicting-id".into()],
                },
            },
        ],
        access: vec![],
        unknown_access: vec![],
        unresolved_grants: vec![UnresolvedGrant {
            grant,
            resource: Resource {
                key: EntityKey::new("app", "resource"),
                name: "Resource".into(),
                kind: None,
                parent: None,
            },
            group: None,
        }],
    };
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/schema2/resolutions.json")).unwrap();
    assert_eq!(
        serde_json::to_value(schema2::AdminAccess::from(&result)).unwrap(),
        expected
    );
}

#[test]
fn every_classification_enum_has_an_explicit_schema_two_spelling() {
    use permesh_core::*;
    let kinds = [
        IdentityKind::Human,
        IdentityKind::Service,
        IdentityKind::Bot,
        IdentityKind::Unknown,
    ];
    let statuses = [
        IdentityStatus::Active,
        IdentityStatus::Inactive,
        IdentityStatus::Suspended,
        IdentityStatus::Unknown,
    ];
    let privileges = [
        Privilege::Standard,
        Privilege::Elevated,
        Privilege::Admin,
        Privilege::Owner,
        Privilege::Unknown,
    ];
    let certainties = [
        Certainty::Observed,
        Certainty::Derived,
        Certainty::Inferred,
        Certainty::Unknown,
    ];
    let affiliations = [
        Affiliation::Internal,
        Affiliation::External,
        Affiliation::Unknown,
    ];
    let evidence = [
        EvidenceKind::Permission,
        EvidenceKind::Assignment,
        EvidenceKind::PolicyAttachment,
        EvidenceKind::Unknown,
    ];
    let mut snapshot = Snapshot::new("enums");
    for (n, (&kind, &status)) in kinds.iter().zip(&statuses).enumerate() {
        snapshot.identities.push(Identity {
            id: n.to_string(),
            kind,
            affiliation: affiliations[n % affiliations.len()],
            status,
            verified_emails: vec![],
        });
        snapshot.accounts.push(Account {
            key: EntityKey::new("enums", n.to_string()),
            login: n.to_string(),
            kind,
            affiliation: affiliations[n % affiliations.len()],
            status,
            verified_emails: vec![],
        });
    }
    for (n, privilege) in privileges.into_iter().enumerate() {
        snapshot.grants.push(Grant {
            id: n.to_string(),
            subject: Subject::Account(EntityKey::new("enums", "0")),
            resource: EntityKey::new("enums", "r"),
            role: "Native role".into(),
            privilege,
            certainty: certainties[n % certainties.len()],
            evidence_kind: evidence[n % evidence.len()],
            provenance: Provenance {
                method: "fixture".into(),
                observed_at: "2026-01-01T00:00:00Z".into(),
            },
        });
    }
    let value = serde_json::to_value(schema2::Snapshot::from(&snapshot)).unwrap();
    let strings = |collection: &str, field: &str| -> Vec<String> {
        value[collection]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v[field].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        strings("identities", "kind"),
        ["human", "service", "bot", "unknown"]
    );
    assert_eq!(
        strings("accounts", "kind"),
        ["human", "service", "bot", "unknown"]
    );
    assert_eq!(
        strings("identities", "status"),
        ["active", "inactive", "suspended", "unknown"]
    );
    assert_eq!(
        strings("grants", "privilege"),
        ["standard", "elevated", "admin", "owner", "unknown"]
    );
    assert_eq!(
        strings("grants", "certainty"),
        ["observed", "derived", "inferred", "unknown", "observed"]
    );
    assert_eq!(
        strings("accounts", "status"),
        ["active", "inactive", "suspended", "unknown"]
    );
    assert_eq!(
        strings("identities", "affiliation"),
        ["internal", "external", "unknown", "internal"]
    );
    assert_eq!(
        strings("accounts", "affiliation"),
        ["internal", "external", "unknown", "internal"]
    );
    assert_eq!(
        strings("grants", "evidence_kind"),
        [
            "permission",
            "assignment",
            "policy_attachment",
            "unknown",
            "permission"
        ]
    );
    let reasons = [
        OrphanReason::InactiveIdentity,
        OrphanReason::SuspendedIdentity,
        OrphanReason::UnknownIdentity,
        OrphanReason::UnknownStatus,
        OrphanReason::AmbiguousIdentity,
        OrphanReason::ExternalIdentity,
        OrphanReason::ServiceAccount,
        OrphanReason::Bot,
        OrphanReason::Unassessed,
    ];
    let orphaned = OrphanedAccess {
        authorities: vec!["directory".into()],
        authority_complete: false,
        accounts: reasons
            .into_iter()
            .map(|reason| OrphanedAccount {
                account: snapshot.accounts[0].clone(),
                identity: IdentityResolution::Unmapped,
                reason,
            })
            .collect(),
        access: vec![],
    };
    let value = serde_json::to_value(schema2::OrphanedAccess::from(&orphaned)).unwrap();
    let actual: Vec<_> = value["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["reason"].as_str().unwrap())
        .collect();
    assert_eq!(
        actual,
        [
            "inactive_identity",
            "suspended_identity",
            "unknown_identity",
            "unknown_status",
            "ambiguous_identity",
            "external_identity",
            "service_account",
            "bot",
            "unassessed"
        ]
    );
    assert_eq!(value["authority_complete"], false);
}

#[test]
fn cli_demo_query_results_match_complete_schema_two_fixtures() {
    use std::process::Command;
    let directory = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
            .args(args)
            .arg("--json")
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    run(&["init", "--demo"]);
    for (name, args) in [
        ("user", vec!["user", "alice@example.com"]),
        ("admins", vec!["admins"]),
        ("orphaned", vec!["orphaned"]),
    ] {
        let report = run(&args);
        assert_eq!(report["schema_version"], 2);
        assert_eq!(report["command"], name);
        assert_eq!(report["complete"], true);
        assert_eq!(report["result"], fixture(name));
    }
}

#[tokio::test]
async fn unknown_access_and_unresolved_group_records_keep_full_evidence() {
    use permesh_core::*;
    let snapshot = permesh_provider_demo::DemoProvider::new("demo")
        .discover()
        .await
        .unwrap();
    let mut user = query_user(&[snapshot], &Aliases::new(), "alice@example.com").unwrap();
    let mut path = user.access.remove(0);
    path.grant.privilege = Privilege::Unknown;
    path.grant.certainty = Certainty::Inferred;
    let result = AdminAccess {
        accounts: vec![],
        access: vec![],
        unknown_access: vec![path.clone()],
        unresolved_grants: vec![UnresolvedGrant {
            grant: path.grant.clone(),
            resource: path.resource.clone(),
            group: path.groups.first().cloned(),
        }],
    };
    let mut expected_path = fixture("user")["access"][0].clone();
    expected_path["certainty"] = serde_json::json!("inferred");
    expected_path["grant"]["privilege"] = serde_json::json!("unknown");
    expected_path["grant"]["certainty"] = serde_json::json!("inferred");
    let expected = serde_json::json!({"accounts":[],"access":[],"unknown_access":[expected_path.clone()],
        "unresolved_grants":[{"grant":expected_path["grant"],"resource":expected_path["resource"],
            "group":expected_path["groups"][0]}]});
    assert_eq!(
        serde_json::to_value(schema2::AdminAccess::from(&result)).unwrap(),
        expected
    );
}
