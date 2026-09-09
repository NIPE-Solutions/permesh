// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
use crate::artifact::{IdentityContext, ProviderCapture, records};
fn now() -> OffsetDateTime {
    config::expiry("2026-09-09T12:00:00Z").unwrap()
}
fn policy() -> Policy {
    serde_json::from_value(json!({"version":1,"rules":["inactive_access"]})).unwrap()
}
async fn fixture() -> Artifact {
    use permesh_provider_sdk::Provider;
    let snapshot = permesh_provider_demo::DemoProvider::new("demo")
        .discover()
        .await
        .unwrap();
    Artifact {
        format: "permesh_snapshot".into(),
        format_version: 1,
        producer_version: "test".into(),
        started_at: "2026-09-09T11:00:00Z".into(),
        completed_at: "2026-09-09T11:01:00Z".into(),
        identity: IdentityContext {
            authorities: vec!["demo".into()],
            bindings: vec![],
        },
        providers: vec![ProviderCapture {
            instance: "demo".into(),
            provider_type: "demo".into(),
            provider_version: None,
            executable_sha256: None,
            capabilities: vec![
                "accounts",
                "identities",
                "resources",
                "groups",
                "memberships",
                "grants",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            context_sha256: "a".repeat(64),
            configured_scope: BTreeMap::new(),
            started_at: "2026-09-09T11:00:00Z".into(),
            completed_at: "2026-09-09T11:01:00Z".into(),
            state: State::Complete,
            limitations: vec![],
            data: Some((&snapshot).into()),
            failure_code: None,
            source_observation: None,
        }],
    }
}
#[tokio::test]
async fn findings_remain_visible_when_collection_is_partial_and_precedence_is_explicit() {
    let mut artifact = fixture().await;
    let (result, code, complete) = evaluate(&policy(), &artifact, now()).unwrap();
    assert_eq!(code, 6);
    assert!(complete);
    assert!(result["findings"].as_u64().unwrap() > 0);
    artifact.providers[0].state = State::Partial;
    artifact.providers[0].data.as_mut().unwrap().complete = false;
    let (result, code, complete) = evaluate(&policy(), &artifact, now()).unwrap();
    assert_eq!(code, 4);
    assert!(!complete);
    assert!(result["findings"].as_u64().unwrap() > 0);
    assert_eq!(result["clean"], false);
    assert_eq!(exit_code(true, true, 9), 3);
    assert_eq!(exit_code(false, true, 9), 4);
    assert_eq!(exit_code(false, false, 9), 6);
    assert_eq!(exit_code(false, false, 0), 0);
}
#[tokio::test]
async fn exceptions_match_exact_scope_expire_and_never_suppress_unknown_evidence() {
    let artifact = fixture().await;
    let mut policy = policy();
    let (result, _, _) = evaluate(&policy, &artifact, now()).unwrap();
    let finding = result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["state"] == "finding")
        .unwrap();
    let exception = json!({"id":"review-1","rule":"inactive_access","instance":finding["instance"],"account":finding["account"],"resource":finding["resource"],"grant_id":finding["grant_id"],"reason":"Temporary documented review","owner":"security:owner-1","expires_at":"2026-09-10T00:00:00Z"});
    policy.exceptions = vec![serde_json::from_value(exception.clone()).unwrap()];
    let (result, _, _) = evaluate(&policy, &artifact, now()).unwrap();
    assert_eq!(result["excepted"], 1);
    assert_eq!(result["clean"], false);
    policy.exceptions[0].resource = "another-resource".into();
    assert_eq!(
        evaluate(&policy, &artifact, now()).unwrap().0["excepted"],
        0
    );
    policy.exceptions = vec![serde_json::from_value(exception).unwrap()];
    policy.rules.push(Rule::ExpiredException);
    policy.exceptions[0].expires_at = "2026-09-09T12:00:00Z".into();
    let (result, _, _) = evaluate(&policy, &artifact, now()).unwrap();
    assert_eq!(result["excepted"], 0);
    assert!(
        result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["rule"] == "expired_exception" && r["state"] == "finding")
    );
    let mut unknown = artifact;
    for grant in &mut unknown.providers[0].data.as_mut().unwrap().grants {
        grant.privilege = records::Privilege::Unknown;
    }
    let p: Policy = serde_json::from_value(json!({"version":1,"rules":["machine_owner"]})).unwrap();
    let (result, code, _) = evaluate(&p, &unknown, now()).unwrap();
    assert_eq!(code, 4);
    assert!(result["not_evaluable"].as_u64().unwrap() > 0);
}
#[tokio::test]
async fn coverage_declarations_and_capability_gaps_cannot_pass_silently() {
    let artifact = fixture().await;
    let policy:Policy=serde_json::from_value(json!({"version":1,"rules":["required_coverage"],"required_providers":[{"instance":"missing","capabilities":["accounts","grants"]}]})).unwrap();
    let (result, code, complete) = evaluate(&policy, &artifact, now()).unwrap();
    assert_eq!(code, 4);
    assert!(!complete);
    assert_eq!(result["findings"], 1);
}
#[test]
fn strict_policy_scopes_owners_and_expiries_are_validated_without_reflection() {
    for doc in [
        json!({"version":2,"rules":["inactive_access"]}),
        json!({"version":1,"rules":["inactive_access","inactive_access"]}),
        json!({"version":1,"rules":["required_coverage"]}),
        json!({"version":1,"rules":["inactive_access"],"owners":[{"instance":"p","account":"a","owner":"","reason":"SENTINEL"}]}),
    ] {
        let p: Policy = serde_json::from_value(doc).unwrap();
        let error = p.validate().err().unwrap();
        assert!(!error.message.contains("SENTINEL"));
    }
    assert!(
        serde_json::from_value::<Policy>(json!({"version":1,"rules":["arbitrary_code"]})).is_err()
    );
    assert!(
        serde_json::from_value::<Policy>(
            json!({"version":1,"rules":["inactive_access"],"unexpected":true})
        )
        .is_err()
    );
}

#[tokio::test]
async fn owners_and_exceptions_never_apply_to_another_account_or_unknown_privilege() {
    let mut artifact = fixture().await;
    let data = artifact.providers[0].data.as_mut().unwrap();
    let grant = data
        .grants
        .iter_mut()
        .find(|g| matches!(&g.subject,records::Subject::Account(key) if key.id=="103"))
        .unwrap();
    grant.privilege = records::Privilege::Admin;
    let grant_id = grant.id.clone();
    let resource = grant.resource.id.clone();
    let mut policy:Policy=serde_json::from_value(json!({"version":1,"rules":["machine_owner"],"owners":[{"instance":"demo","account":"100","owner":"security:1","reason":"Declared owner"}]})).unwrap();
    assert!(
        evaluate(&policy, &artifact, now()).unwrap().0["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["account"] == "103" && r["state"] == "finding")
    );
    policy.owners[0].account = "103".into();
    assert!(
        evaluate(&policy, &artifact, now()).unwrap().0["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["account"] == "103" && r["state"] == "pass")
    );
    policy.exceptions=vec![serde_json::from_value(json!({"id":"e1","rule":"machine_owner","instance":"demo","account":"103","resource":resource,"grant_id":grant_id,"owner":"security:1","reason":"Scoped review","expires_at":"2027-01-01T00:00:00Z"})).unwrap()];
    for grant in &mut artifact.providers[0].data.as_mut().unwrap().grants {
        grant.privilege = records::Privilege::Unknown;
    }
    let result = evaluate(&policy, &artifact, now()).unwrap().0;
    assert_eq!(result["excepted"], 0);
    assert!(
        result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["account"] == "103" && r["state"] == "not_evaluable")
    );
}

#[tokio::test]
async fn precancelled_policy_does_not_even_load_its_rules() {
    use clap::Parser;
    let cli = Cli::parse_from([
        "permesh",
        "policy",
        "check",
        "--rules",
        "missing-policy.json",
    ]);
    let cancel = Cancellation::new();
    cancel.cancel();
    let command = PolicyCommand::Check {
        rules: "missing-policy.json".into(),
        snapshot: None,
    };
    let result = run(&cli, &command, &BlockingPool::new(), &cancel).await;
    assert!(result.is_err_and(|e| e.code == 130));
}
#[test]
fn policy_row_budget_fails_before_retaining_an_oversized_check() {
    let mut rows = vec![];
    let mut remaining = 16;
    assert!(
        push_check(
            &mut rows,
            &mut remaining,
            json!({"reason":"larger than budget"})
        )
        .is_err()
    );
    assert!(rows.is_empty());
}
