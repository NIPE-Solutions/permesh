// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn key(id: &str) -> Value {
    json!({"provider":"github","id":id})
}
fn fixture() -> Value {
    let provenance = json!({"method":"synthetic_fixture","observed_at":"2026-09-09T00:00:00Z"});
    let account = |id: &str, kind: &str| json!({"key":key(id),"login":format!("<script>{id}</script>"),"kind":kind,"status":"active","affiliation":"external","verified_emails":[]});
    let grant = |id: &str, subject: Value, privilege: &str| json!({"id":id,"subject":subject,"resource":key("repo"),"role":privilege,"privilege":privilege,"certainty":"observed","evidence_kind":"assignment","provenance":provenance});
    let data = |id: &str| json!({"provider":id,"identities":[],"accounts":[],"resources":[],"groups":[],"memberships":[],"grants":[],"limitations":[],"complete":true});
    let source = |id: &str| json!({"instance":id,"provider_type":"fixture","provider_version":"1","executable_sha256":null,"capabilities":["identities","accounts","groups","memberships","resources","grants"],"context_sha256":"a".repeat(64),"configured_scope":{},"started_at":"2026-09-09T00:00:00Z","completed_at":"2026-09-09T00:00:01Z","state":"complete","limitations":[],"failure_code":null,"source_observation":null,"data":data(id)});
    let mut authority = source("directory");
    authority["data"]["identities"] = json!([{"id":"contractor-7","kind":"human","affiliation":"external","status":"inactive","verified_emails":[]}]);
    let mut github = source("github");
    github["data"]["accounts"] = json!([account("42", "human"), account("99", "service")]);
    github["data"]["resources"] = json!([{"key":key("repo"),"name":"<img src=x onerror=alert(1)>","kind":"github.repository","parent":null}]);
    github["data"]["groups"] = json!([{"key":key("team"),"name":"Contractors"}]);
    github["data"]["memberships"] = json!([{"member":{"kind":"account","key":key("42")},"group":key("team"),"provenance":provenance}]);
    github["data"]["grants"] = json!([
        grant(
            "direct",
            json!({"kind":"account","key":key("42")}),
            "standard"
        ),
        grant(
            "inherited",
            json!({"kind":"group","key":key("team")}),
            "standard"
        ),
        grant("owner", json!({"kind":"account","key":key("42")}), "owner")
    ]);
    let mut partial = source("other");
    partial["state"] = "partial".into();
    partial["data"]["complete"] = false.into();
    partial["limitations"] = json!(["Fixture visibility incomplete"]);
    partial["data"]["limitations"] = partial["limitations"].clone();
    let mut result = json!({"format":"permesh_snapshot","format_version":1,"producer_version":"fixture","started_at":"2026-09-09T00:00:00Z","completed_at":"2026-09-09T00:00:01Z","identity":{"authorities":["directory"],"bindings":[{"identity":"contractor-7","instance":"github","account":"42"}]},"providers":[authority,github,partial]});
    let now = time::OffsetDateTime::now_utc() - time::Duration::minutes(2);
    let start = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let end = (now + time::Duration::seconds(1))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    result["started_at"] = start.clone().into();
    result["completed_at"] = end.clone().into();
    for p in result["providers"].as_array_mut().unwrap() {
        p["started_at"] = start.clone().into();
        p["completed_at"] = end.clone().into();
    }
    result
}
fn save(dir: &Path, name: &str, value: &Value) {
    fs::write(dir.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}
fn result(output: Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{} {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}
#[test]
fn contractor_plan_preserves_paths_dependencies_and_partial_verification() {
    let dir = tempfile::tempdir().unwrap();
    let before = fixture();
    save(dir.path(), "before.json", &before);
    save(
        dir.path(),
        "owners.json",
        &json!({"version":1,"owners":[{"account":key("99"),"identity":"contractor-7"}]}),
    );
    let planned = result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--annotations",
                "owners.json",
                "--output",
                "plan.json",
                "--html",
                "report.html",
                "--json",
            ],
        ),
        4,
    );
    assert_eq!(planned["mode"], "snapshot_replay");
    let plan: Value =
        serde_json::from_slice(&fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    assert_eq!(plan["assessment"]["paths"].as_array().unwrap().len(), 3);
    assert_eq!(plan["assessment"]["ownership"].as_array().unwrap().len(), 1);
    assert_eq!(plan["assessment"]["accounts"].as_array().unwrap().len(), 1);
    let kinds: Vec<_> = plan["assessment"]["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"review_group_membership")
            && kinds.contains(&"review_ownership")
            && kinds.contains(&"review_machine_dependency")
    );
    let html = fs::read_to_string(dir.path().join("report.html")).unwrap();
    assert!(!html.contains("<script>42"));
    assert!(html.contains("&lt;script&gt;42"));
    assert!(!html.contains("<img src=x"));
    let mut after = before.clone();
    let now = time::OffsetDateTime::now_utc() - time::Duration::minutes(1);
    let start = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let end = (now + time::Duration::seconds(1))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    after["started_at"] = start.clone().into();
    after["completed_at"] = end.clone().into();
    for p in after["providers"].as_array_mut().unwrap() {
        p["started_at"] = start.clone().into();
        p["completed_at"] = end.clone().into();
    }
    after["providers"][1]["data"]["grants"]
        .as_array_mut()
        .unwrap()
        .retain(|g| g["id"] != "direct");
    save(dir.path(), "after.json", &after);
    let verified = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    let checks = verified["checks"].as_array().unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c["native_id"] == "direct" && c["state"] == "no_longer_observed")
    );
    assert!(
        checks
            .iter()
            .any(|c| c["native_id"] == "inherited" && c["state"] == "still_observed")
    );
    assert!(
        verified["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["provider"] == "other")
    );
    let repeated = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert_eq!(verified["checks"], repeated["checks"]);
}
#[test]
fn ambiguous_target_cannot_produce_a_plan_and_untrusted_json_is_not_reflected() {
    let dir = tempfile::tempdir().unwrap();
    let mut input = fixture();
    input["identity"]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(json!({"identity":"conflicting","instance":"github","account":"42"}));
    save(dir.path(), "before.json", &input);
    let output = run(
        dir.path(),
        &[
            "offboard",
            "plan",
            "contractor-7",
            "--snapshot",
            "before.json",
            "--output",
            "plan.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!dir.path().join("plan.json").exists());
    fs::write(
        dir.path().join("evil.json"),
        r#"{"format":"REMOTE_SECRET","format":"x"}"#,
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "offboard",
            "verify",
            "evil.json",
            "--snapshot",
            "before.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("REMOTE_SECRET"));
}

#[test]
fn default_uses_fresh_discovery_and_never_exports_unrelated_accounts() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let assessed = result(
        run(
            dir.path(),
            &["offboard", "assess", "alice@example.com", "--json"],
        ),
        0,
    );
    assert_eq!(assessed["mode"], "fresh_discovery");
    assert_eq!(
        assessed["assessment"]["accounts"].as_array().unwrap().len(),
        1
    );
    let planned = result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "alice@example.com",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        0,
    );
    assert_eq!(planned["mode"], "fresh_discovery");
    let text = fs::read_to_string(dir.path().join("plan.json")).unwrap();
    assert!(!text.contains("bob-admin"));
    assert!(!text.contains("former@example.com"));
    let verified = result(
        run(dir.path(), &["offboard", "verify", "plan.json", "--json"]),
        0,
    );
    assert_eq!(verified["mode"], "fresh_discovery");
    assert!(
        verified["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "still_observed")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.path().join("plan.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
fn later(mut input: Value) -> Value {
    let start = (time::OffsetDateTime::now_utc() - time::Duration::seconds(30))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let end = (time::OffsetDateTime::now_utc() - time::Duration::seconds(29))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    input["started_at"] = start.clone().into();
    input["completed_at"] = end.clone().into();
    for p in input["providers"].as_array_mut().unwrap() {
        p["started_at"] = start.clone().into();
        p["completed_at"] = end.clone().into();
    }
    input
}
#[test]
fn context_authority_and_expiry_block_absence_while_inactive_is_separate() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture();
    save(dir.path(), "before.json", &input);
    result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        4,
    );
    let mut after = later(input.clone());
    after["providers"][1]["data"]["accounts"][0]["status"] = "suspended".into();
    save(dir.path(), "after.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["native_id"] == "42" && c["state"] == "account_inactive_observed")
    );
    after["providers"][1]["data"]["grants"] = json!([]);
    after["providers"][1]["context_sha256"] = "b".repeat(64).into();
    save(dir.path(), "after.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "cannot_verify")
    );
    after["providers"][0]["data"]["identities"] = json!([]);
    after["identity"]["authorities"] = json!([]);
    save(dir.path(), "after.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert_eq!(report["assessment_origin"], "plan_baseline");
    assert!(
        report["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["reason"] == "identity_not_established")
    );
    let mut plan: Value =
        serde_json::from_slice(&fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    let start = time::OffsetDateTime::now_utc() - time::Duration::hours(3);
    let fmt = &time::format_description::well_known::Rfc3339;
    for s in plan["assessment"]["sources"].as_array_mut().unwrap() {
        s["capture"]["started_at"] = start.format(fmt).unwrap().into();
        s["capture"]["completed_at"] = (start + time::Duration::seconds(1))
            .format(fmt)
            .unwrap()
            .into();
    }
    plan["created_at"] = (start + time::Duration::seconds(2))
        .format(fmt)
        .unwrap()
        .into();
    plan["expires_at"] = (start + time::Duration::hours(1))
        .format(fmt)
        .unwrap()
        .into();
    plan["max_age_hours"] = 1.into();
    save(dir.path(), "expired.json", &plan);
    save(dir.path(), "after.json", &later(input));
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "expired.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "cannot_verify")
    );
    assert!(
        report["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["reason"] == "plan_expired")
    );
}
#[test]
fn invalid_output_and_freshness_fail_before_loading_any_workspace() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("existing.json"), "original").unwrap();
    for args in [
        vec![
            "offboard",
            "plan",
            "person",
            "--output",
            "existing.json",
            "--json",
        ],
        vec![
            "offboard",
            "plan",
            "person",
            "--output",
            "new.json",
            "--expires-in-hours",
            "169",
            "--json",
        ],
    ] {
        let output = run(dir.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            !report["error"]["message"]
                .as_str()
                .unwrap()
                .contains("workspace")
        );
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.json")).unwrap(),
        "original"
    );
}

#[test]
fn missing_connectors_preserve_failure_class_and_manual_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let before = fixture();
    save(dir.path(), "before.json", &before);
    save(
        dir.path(),
        "owners.json",
        &json!({"version":1,"owners":[{"account":key("99"),"identity":"contractor-7"}]}),
    );
    result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--annotations",
                "owners.json",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        4,
    );
    let mut after = later(before);
    after["providers"][1]["state"] = "failed".into();
    after["providers"][1]["failure_code"] = "provider_collection_failed".into();
    after["providers"][1]["data"] = Value::Null;
    save(dir.path(), "after.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "cannot_verify")
    );
    assert_eq!(
        report["assessment"]["ownership"].as_array().unwrap().len(),
        1
    );
    for p in after["providers"].as_array_mut().unwrap() {
        p["state"] = "failed".into();
        p["failure_code"] = "provider_collection_failed".into();
        p["data"] = Value::Null;
    }
    save(dir.path(), "failed.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "failed.json",
                "--json",
            ],
        ),
        3,
    );
    assert_eq!(report["complete"], false);
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "cannot_verify")
    );
    let output = run(
        dir.path(),
        &[
            "offboard",
            "assess",
            "contractor-7",
            "--snapshot",
            "failed.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
}
#[test]
fn standalone_membership_is_verified_and_sole_owner_is_visibility_qualified() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = fixture();
    before["providers"][1]["provider_type"] = "github".into();
    before["providers"][1]["data"]["grants"][2]["provenance"]["method"] =
        "github.organization_role".into();
    before["providers"][1]["data"]["resources"][0]["kind"] = "github.organization".into();
    save(dir.path(), "owners.json", &before);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "assess",
                "contractor-7",
                "--snapshot",
                "owners.json",
                "--json",
            ],
        ),
        4,
    );
    assert_eq!(
        report["assessment"]["ownership_findings"][0]["sole_observed_owner"],
        true
    );
    assert_eq!(
        report["assessment"]["ownership_findings"][0]["basis"],
        "complete_visible_github_organization_membership"
    );
    before["providers"][1]["data"]["grants"] = json!([]);
    save(dir.path(), "before.json", &before);
    result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        4,
    );
    let mut after = later(before);
    after["providers"][1]["data"]["memberships"] = json!([]);
    save(dir.path(), "after.json", &after);
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "after.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["native_id"] == "team" && c["state"] == "no_longer_observed")
    );
}

#[test]
fn future_source_times_cannot_mint_a_fresh_plan() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture();
    save(dir.path(), "before.json", &input);
    result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        4,
    );
    let mut plan: Value =
        serde_json::from_slice(&fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    let future = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    let fmt = &time::format_description::well_known::Rfc3339;
    for s in plan["assessment"]["sources"].as_array_mut().unwrap() {
        s["capture"]["started_at"] = future.format(fmt).unwrap().into();
        s["capture"]["completed_at"] = (future + time::Duration::seconds(1))
            .format(fmt)
            .unwrap()
            .into();
    }
    plan["expires_at"] = (future + time::Duration::hours(24))
        .format(fmt)
        .unwrap()
        .into();
    save(dir.path(), "future-plan.json", &plan);
    let output = run(
        dir.path(),
        &[
            "offboard",
            "verify",
            "future-plan.json",
            "--snapshot",
            "before.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let mut future_snapshot = input;
    future_snapshot["started_at"] = future.format(fmt).unwrap().into();
    future_snapshot["completed_at"] = (future + time::Duration::seconds(1))
        .format(fmt)
        .unwrap()
        .into();
    for p in future_snapshot["providers"].as_array_mut().unwrap() {
        p["started_at"] = future_snapshot_time(future);
        p["completed_at"] = future_snapshot_time(future + time::Duration::seconds(1));
    }
    save(dir.path(), "future.json", &future_snapshot);
    let output = run(
        dir.path(),
        &[
            "offboard",
            "plan",
            "contractor-7",
            "--snapshot",
            "future.json",
            "--output",
            "new.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!dir.path().join("new.json").exists());
    let report = result(
        run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "plan.json",
                "--snapshot",
                "future.json",
                "--json",
            ],
        ),
        4,
    );
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "cannot_verify")
    );
}
fn future_snapshot_time(t: time::OffsetDateTime) -> Value {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
        .into()
}

#[test]
fn inventory_export_age_bounds_plan_lifetime_and_imported_steps_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let mut input = fixture();
    input["providers"][0]["provider_type"] = "inventory".into();
    input["providers"][0]["source_observation"] = json!({"method":"file_inventory","declared_scope":"contractors","exported_at":future_snapshot_time(time::OffsetDateTime::now_utc()-time::Duration::hours(48)),"content_sha256":"b".repeat(64)});
    save(dir.path(), "before.json", &input);
    let output = run(
        dir.path(),
        &[
            "offboard",
            "plan",
            "contractor-7",
            "--snapshot",
            "before.json",
            "--output",
            "expired.json",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!dir.path().join("expired.json").exists());
    result(
        run(
            dir.path(),
            &[
                "offboard",
                "plan",
                "contractor-7",
                "--snapshot",
                "before.json",
                "--expires-in-hours",
                "72",
                "--output",
                "plan.json",
                "--json",
            ],
        ),
        4,
    );
    let original: Value =
        serde_json::from_slice(&fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    for change in 0..3 {
        let mut plan = original.clone();
        match change {
            0 => plan["assessment"]["recommendations"][0]["id"] = "c".repeat(64).into(),
            1 => plan["assessment"]["paths"][0]["grant"]["resource"]["id"] = "REMOTE_SECRET".into(),
            _ => plan["assessment"]["sources"][1]["capture"]["capabilities"] = json!([]),
        }
        save(dir.path(), "tampered.json", &plan);
        let output = run(
            dir.path(),
            &[
                "offboard",
                "verify",
                "tampered.json",
                "--snapshot",
                "before.json",
                "--json",
            ],
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("REMOTE_SECRET"));
    }
}
