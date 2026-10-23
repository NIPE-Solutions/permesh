// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const BACKEND: &str = "profile: backend\ngrants:\n  - {instance: demo, group: team-10}\n  - {instance: demo, role: Read, resource: repo-21}\n";
const BASE: &str = "profile: base\ngrants:\n  - {instance: demo, role: Read, resource: repo-21}\n";
const BACKEND_ON_BASE: &str =
    "profile: backend\nincludes: [base]\ngrants:\n  - {instance: demo, group: team-10}\n";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .args(args)
        .output()
        .unwrap()
}
fn demo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    for (name, body) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    dir
}
fn person(identity: &str, profile: &str) -> String {
    format!("identity: {identity}\nprofiles: [{profile}]\n")
}
fn report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}
fn access(dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let mut all = vec!["access"];
    all.extend_from_slice(args);
    all.push("--json");
    let output = run(dir, &all);
    (output.status.code(), report(&output))
}
fn steps(result: &Value) -> &Vec<Value> {
    result["result"]["access_plan"]["steps"].as_array().unwrap()
}
fn of_type<'a>(result: &'a Value, kind: &str) -> Vec<&'a Value> {
    steps(result)
        .iter()
        .filter(|s| s["operation"]["type"] == kind)
        .collect()
}
fn saved(dir: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(dir.join(name)).unwrap()).unwrap()
}

#[test]
fn join_plan_lists_missing_access_and_writes_file() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = access(
        dir.path(),
        &[
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(result["command"], "access_plan");
    let types: Vec<_> = steps(&result)
        .iter()
        .map(|s| s["operation"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, vec!["add_membership", "add_role"]);
    assert_eq!(result["result"]["counts"]["steps"], 2);
    let plan = saved(dir.path(), "p.json");
    assert_eq!(plan["format"], "permesh_access_plan");
    assert_eq!(plan["kind"], "join");
    assert_eq!(plan, result["result"]["access_plan"]);
    assert_eq!(result["result"]["plan_sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn joiner_without_accounts_gets_account_needed_steps() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = access(
        dir.path(),
        &[
            "join",
            "carol@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    let needed = of_type(&result, "account_needed");
    assert_eq!(needed.len(), 1);
    let id = &needed[0]["id"];
    let adds: Vec<_> = steps(&result)
        .iter()
        .filter(|s| s["operation"]["type"] != "account_needed")
        .collect();
    assert_eq!(adds.len(), 2);
    for add in adds {
        assert_eq!(add["prerequisites"], serde_json::json!([id]));
    }
    assert_eq!(result["result"]["counts"]["account_needed"], 1);
}

#[test]
fn move_keeps_shared_access() {
    let dir = demo(&[
        ("profiles/base.yaml", BASE),
        ("profiles/backend.yaml", BACKEND_ON_BASE),
        ("people/alice.yaml", &person("alice@example.com", "backend")),
    ]);
    let (code, result) = access(
        dir.path(),
        &[
            "move",
            "alice@example.com",
            "--remove",
            "backend",
            "--add",
            "base",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    let plan = &result["result"]["access_plan"];
    assert_eq!(plan["kind"], "move");
    assert_eq!(plan["profiles_after"], serde_json::json!(["base"]));
    assert_eq!(plan["profiles_removed"], serde_json::json!(["backend"]));
    let types: Vec<_> = steps(&result)
        .iter()
        .map(|s| (&s["operation"]["type"], &s["operation"]["group"]))
        .collect();
    assert_eq!(
        types,
        vec![(&Value::from("remove_membership"), &Value::from("team-10"))]
    );
    assert_eq!(plan["kept"][0]["entry"]["resource"], "repo-21");
    assert_eq!(plan["kept"][0]["because"], serde_json::json!(["base"]));
}

#[test]
fn leave_separates_expected_from_unexplained() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let (code, result) = access(
        dir.path(),
        &["leave", "bob@example.com", "--output", "p.json"],
    );
    assert_eq!(code, Some(0), "{result}");
    let removals = of_type(&result, "remove_role");
    assert_eq!(removals.len(), 1);
    assert_eq!(removals[0]["operation"]["grant_id"], "grant-3");
    assert_eq!(removals[0]["justification"], "unexplained");
    assert_eq!(of_type(&result, "review_account_lifecycle").len(), 1);
    assert_eq!(
        result["result"]["access_plan"]["profiles_removed"],
        serde_json::json!(["backend"])
    );
}

#[test]
fn leave_uses_directory_group_assignment() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut config = fs::read_to_string(dir.path().join("permesh.yaml")).unwrap();
    config.push_str(
        "profiles:\n  directory_groups:\n    - {instance: demo, group: team-10, profiles: [backend]}\n",
    );
    fs::write(dir.path().join("permesh.yaml"), config).unwrap();
    let (code, result) = access(
        dir.path(),
        &["leave", "alice@example.com", "--output", "p.json"],
    );
    assert_eq!(code, Some(0), "{result}");
    let membership = of_type(&result, "remove_membership");
    assert_eq!(membership.len(), 1);
    assert_eq!(
        membership[0]["justification"],
        serde_json::json!({"profiles": ["backend"]})
    );
}

#[test]
fn leave_with_owner_annotation_reviews_machine_dependency_first() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
        (
            "owners.json",
            r#"{"version":1,"owners":[{"account":{"provider":"demo","id":"103"},"identity":"bob@example.com"}]}"#,
        ),
    ]);
    let (code, result) = access(
        dir.path(),
        &[
            "leave",
            "bob@example.com",
            "--annotations",
            "owners.json",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    let review = of_type(&result, "review_machine_dependency");
    assert_eq!(review.len(), 1);
    assert_eq!(review[0]["operation"]["account"], "103");
    assert_eq!(review[0]["verification"], "manual_review");
    for removal in of_type(&result, "remove_role") {
        assert_eq!(
            removal["prerequisites"],
            serde_json::json!([review[0]["id"]])
        );
    }
}

#[test]
fn existing_output_is_refused() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    fs::write(dir.path().join("p.json"), "keep").unwrap();
    let (code, _) = access(
        dir.path(),
        &[
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(2));
    assert_eq!(
        fs::read_to_string(dir.path().join("p.json")).unwrap(),
        "keep"
    );
}

#[test]
fn unknown_profile_is_input_error() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = access(
        dir.path(),
        &[
            "join",
            "bob@example.com",
            "--profile",
            "nope",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(2), "{result}");
    assert!(!dir.path().join("p.json").exists());
    // Nothing configured at all is the Phase 2 input error.
    let dir = demo(&[]);
    let (code, result) = access(
        dir.path(),
        &[
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(2), "{result}");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("No access profiles are configured"),
        "{result}"
    );
}

#[test]
fn html_is_written_and_escaped() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = access(
        dir.path(),
        &[
            "join",
            "<b>carol</b>@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
            "--html",
            "p.html",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    let html = fs::read_to_string(dir.path().join("p.html")).unwrap();
    assert!(html.contains("default-src 'none'"), "{html}");
    assert!(html.contains("&lt;b&gt;carol&lt;/b&gt;"), "{html}");
    assert!(!html.contains("<b>carol"), "{html}");
    // Plan and HTML must be different files.
    let (code, _) = access(
        dir.path(),
        &[
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--output",
            "q.json",
            "--html",
            "q.json",
        ],
    );
    assert_eq!(code, Some(2));
}

#[test]
fn human_output_lists_steps_and_files() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let output = run(
        dir.path(),
        &[
            "access",
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("add membership team-10 in demo"), "{text}");
    assert!(text.contains("p.json"), "{text}");
}

fn snapshot(dir: &Path) -> Value {
    assert!(
        run(dir, &["snapshot", "create", "--output", "s.json"])
            .status
            .success()
    );
    saved(dir, "s.json")
}
fn save(dir: &Path, name: &str, value: &Value) {
    fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn data(snapshot: &mut Value) -> &mut Value {
    &mut snapshot["providers"][0]["data"]
}
fn provenance() -> Value {
    serde_json::json!({"method": "test edit", "observed_at": "2026-01-01T00:00:00Z"})
}
fn add_membership(snapshot: &mut Value, account: &str, group: &str) {
    data(snapshot)["memberships"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "member": {"kind": "account", "key": {"provider": "demo", "id": account}},
            "group": {"provider": "demo", "id": group},
            "provenance": provenance(),
        }));
}
fn add_grant(snapshot: &mut Value, id: &str, account: &str, resource: &str, role: &str) {
    data(snapshot)["grants"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": id,
            "subject": {"kind": "account", "key": {"provider": "demo", "id": account}},
            "resource": {"provider": "demo", "id": resource},
            "role": role,
            "privilege": "standard",
            "certainty": "observed",
            "evidence_kind": "permission",
            "provenance": provenance(),
        }));
}
fn join_bob(dir: &Path) -> Value {
    snapshot(dir);
    let (code, result) = access(
        dir,
        &[
            "join",
            "bob@example.com",
            "--profile",
            "backend",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    saved(dir, "s.json")
}
fn verify(dir: &Path, extra: &[&str]) -> (Option<i32>, Value) {
    let mut args = vec!["verify", "p.json"];
    args.extend_from_slice(extra);
    access(dir, &args)
}
fn checks(result: &Value) -> Vec<(String, String)> {
    result["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["operation"]["type"].as_str().unwrap().to_owned(),
                c["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn fresh_verify_of_join_plan_reports_not_done_and_exits_6() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    join_bob(dir.path());
    let (code, result) = verify(dir.path(), &[]);
    assert_eq!(code, Some(6), "{result}");
    assert_eq!(result["command"], "access_verify");
    assert_eq!(result["result"]["access_verification_version"], 1);
    assert_eq!(
        checks(&result),
        vec![
            ("add_membership".into(), "not_done".into()),
            ("add_role".into(), "not_done".into())
        ]
    );
    assert_eq!(result["result"]["counts"]["not_done"], 2);
    assert_eq!(result["result"]["complete"], true);
}

#[test]
fn verify_after_change_reports_done() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut s = join_bob(dir.path());
    add_membership(&mut s, "101", "team-10");
    add_grant(&mut s, "grant-99", "101", "repo-21", "Read");
    save(dir.path(), "edited.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "edited.json"]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(
        checks(&result),
        vec![
            ("add_membership".into(), "done".into()),
            ("add_role".into(), "done".into())
        ]
    );
    assert_eq!(result["result"]["counts"]["done"], 2);
}

#[test]
fn partial_capture_cannot_verify_removal() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let mut s = snapshot(dir.path());
    let (code, result) = access(
        dir.path(),
        &[
            "leave",
            "bob@example.com",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{result}");
    // The grant is gone, but the capture is partial: absence proves nothing.
    data(&mut s)["grants"]
        .as_array_mut()
        .unwrap()
        .retain(|g| g["id"] != "grant-3");
    s["providers"][0]["state"] = "partial".into();
    data(&mut s)["complete"] = false.into();
    s["providers"][0]["limitations"] = serde_json::json!(["Fixture visibility incomplete"]);
    data(&mut s)["limitations"] = s["providers"][0]["limitations"].clone();
    save(dir.path(), "partial.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "partial.json"]);
    assert_eq!(code, Some(4), "{result}");
    let removal = result["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["operation"]["type"] == "remove_role")
        .unwrap();
    assert_eq!(removal["state"], "cannot_verify");
    assert_eq!(removal["reason"], "instance_not_observable");
    assert_eq!(result["result"]["complete"], false);
}

#[test]
fn removal_is_done_once_absent_from_a_complete_capture() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let mut s = snapshot(dir.path());
    access(
        dir.path(),
        &[
            "leave",
            "bob@example.com",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    data(&mut s)["grants"]
        .as_array_mut()
        .unwrap()
        .retain(|g| g["id"] != "grant-3");
    save(dir.path(), "after.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "after.json"]);
    // The lifecycle review is manual: counted, but it does not make the result incomplete.
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(
        checks(&result),
        vec![
            ("remove_role".into(), "done".into()),
            ("review_account_lifecycle".into(), "cannot_verify".into())
        ]
    );
    assert_eq!(result["result"]["checks"][1]["reason"], "manual_review");
    assert_eq!(result["result"]["complete"], true);
}

#[test]
fn expired_plan_still_verifies_but_tampered_does_not() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    join_bob(dir.path());
    let mut plan = saved(dir.path(), "p.json");
    plan["created_at"] = "2026-01-01T00:00:00Z".into();
    plan["expires_at"] = "2026-01-01T12:00:00Z".into();
    save(dir.path(), "p.json", &plan);
    let (code, result) = verify(dir.path(), &["--snapshot", "s.json"]);
    assert_eq!(code, Some(6), "{result}");
    plan["steps"][0]["operation"]["group"] = "team-99".into();
    save(dir.path(), "p.json", &plan);
    // No --snapshot: a refused plan never reaches provider discovery.
    let (code, result) = verify(dir.path(), &[]);
    assert_eq!(code, Some(2), "{result}");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid access plan"),
        "{result}"
    );
    assert!(
        result
            .get("providers")
            .is_none_or(|p| p.as_array().is_none_or(Vec::is_empty))
    );
}

#[test]
fn extra_access_is_flagged() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut s = join_bob(dir.path());
    add_grant(&mut s, "grant-98", "101", "repo-20", "Write");
    save(dir.path(), "edited.json", &s);
    let (_, result) = verify(dir.path(), &["--snapshot", "edited.json"]);
    let extra = result["result"]["extra"].as_array().unwrap();
    assert!(
        extra.iter().any(|e| e["grant_id"] == "grant-98"),
        "{result}"
    );
    assert_eq!(
        result["result"]["counts"]["extra"].as_u64().unwrap() as usize,
        extra.len()
    );
}

#[test]
fn account_needed_without_mapping_cannot_verify() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    snapshot(dir.path());
    access(
        dir.path(),
        &[
            "join",
            "carol@example.com",
            "--profile",
            "backend",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    let (code, result) = verify(dir.path(), &["--snapshot", "s.json"]);
    assert_eq!(code, Some(4), "{result}");
    for check in result["result"]["checks"].as_array().unwrap() {
        assert_eq!(check["state"], "cannot_verify", "{check}");
        assert_eq!(check["reason"], "no_mapped_account", "{check}");
    }
}

fn bind(snapshot: &mut Value, identity: &str, account: &str) {
    snapshot["identity"]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"identity": identity, "instance": "demo", "account": account}));
}
fn gaps(result: &Value) -> Vec<&str> {
    result["result"]["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap())
        .collect()
}

#[test]
fn identity_context_change_is_a_gap() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut s = join_bob(dir.path());
    // Bob's account is now also claimed for alice: attribution changed.
    bind(&mut s, "alice@example.com", "101");
    save(dir.path(), "edited.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "edited.json"]);
    assert_eq!(code, Some(4), "{result}");
    assert!(
        gaps(&result).contains(&"identity_context_changed"),
        "{result}"
    );
}

#[test]
fn unrelated_identity_mapping_is_not_a_gap() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut s = join_bob(dir.path());
    bind(&mut s, "contractor@example.com", "104");
    save(dir.path(), "edited.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "edited.json"]);
    assert_eq!(code, Some(6), "{result}");
    assert!(gaps(&result).is_empty(), "{result}");
}

#[test]
fn joiner_mapped_after_the_plan_verifies_done() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let mut s = snapshot(dir.path());
    access(
        dir.path(),
        &[
            "join",
            "carol@example.com",
            "--profile",
            "backend",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    // Carol's account was invited, added as planned and mapped to her identity.
    data(&mut s)["accounts"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "key": {"provider": "demo", "id": "106"},
            "login": "carol-dev",
            "kind": "human",
            "affiliation": "internal",
            "status": "active",
            "verified_emails": [],
        }));
    add_membership(&mut s, "106", "team-10");
    add_grant(&mut s, "grant-96", "106", "repo-21", "Read");
    bind(&mut s, "carol@example.com", "106");
    save(dir.path(), "after.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "after.json"]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(result["result"]["counts"]["done"], 3, "{result}");
}

#[test]
fn verify_html_is_written() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    join_bob(dir.path());
    let (code, _) = verify(dir.path(), &["--snapshot", "s.json", "--html", "v.html"]);
    assert_eq!(code, Some(6));
    let html = fs::read_to_string(dir.path().join("v.html")).unwrap();
    assert!(html.contains("default-src 'none'"));
    assert!(html.contains("not_done"), "{html}");
}

fn left(identity: &str, profile: &str) -> String {
    format!("identity: {identity}\nstatus: left\nprofiles: [{profile}]\n")
}
/// A base checkout next to the head workspace: same permesh.yaml, its own files.
fn base_checkout(head: &Path, files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(head.join("permesh.yaml"), dir.path().join("permesh.yaml")).unwrap();
    for (name, body) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    dir
}
fn review(head: &Path, base: &Path, out: &str, extra: &[&str]) -> (Option<i32>, Value) {
    let base = base.display().to_string();
    let mut args = vec!["review", "--base", base.as_str(), "--out", out];
    args.extend_from_slice(extra);
    access(head, &args)
}
fn plans(result: &Value) -> Vec<(String, String)> {
    result["result"]["plans"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["identity"].as_str().unwrap().to_owned(),
                p["kind"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn review_plans_changed_people_only() {
    let alice = person("alice@example.com", "backend");
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/alice.yaml", &alice),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let base = base_checkout(
        head.path(),
        &[
            ("profiles/backend.yaml", BACKEND),
            ("people/alice.yaml", &alice),
        ],
    );
    let (code, result) = review(head.path(), base.path(), "plans", &[]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(result["command"], "access_review");
    assert_eq!(result["result"]["access_review_version"], 1);
    assert_eq!(
        plans(&result),
        vec![("bob@example.com".into(), "join".into())]
    );
    assert_eq!(result["result"]["profiles_changed"], false);
    let plan = saved(head.path(), "plans/000.json");
    assert_eq!(plan["identity"], "bob@example.com");
    let index = saved(head.path(), "plans/index.json");
    assert_eq!(index["plans"][0]["file"], "000.json");
    assert_eq!(index["plans"][0]["steps"], 2);
    let summary = fs::read_to_string(head.path().join("plans/summary.md")).unwrap();
    assert!(summary.starts_with("# Permesh access plan"), "{summary}");
    assert!(
        summary.contains("People affected: 1 (join 1, move 0, leave 0)"),
        "{summary}"
    );
    assert!(
        summary.contains("add membership 1, add role 1"),
        "{summary}"
    );
    for name in ["bob", "@example.com", "team-10", "repo-21"] {
        assert!(!summary.contains(name), "{name} in {summary}");
    }
    // Detail is an explicit opt-in.
    let (_, _) = review(head.path(), base.path(), "detailed", &["--detail"]);
    let summary = fs::read_to_string(head.path().join("detailed/summary.md")).unwrap();
    assert!(summary.contains("bob@example.com"), "{summary}");
}

#[test]
fn detail_summary_escapes_markdown() {
    let identity = "<b>x</b>_[a](https://e.example)@example.com";
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        (
            "people/x.yaml",
            &format!("identity: '{identity}'\nprofiles: [backend]\n"),
        ),
    ]);
    let base = base_checkout(head.path(), &[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = review(head.path(), base.path(), "detailed", &["--detail"]);
    assert_eq!(code, Some(0), "{result}");
    let summary = fs::read_to_string(head.path().join("detailed/summary.md")).unwrap();
    // Names render as text: no HTML, no link.
    assert!(
        summary.contains(r"\<b\>x\</b\>\_\[a\]\(https://e.example\)@example.com"),
        "{summary}"
    );
    assert!(!summary.contains("<b>") && !summary.contains("]("), "{summary}");
}

#[test]
fn deleted_people_file_is_a_move_and_left_is_a_leave() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &left("bob@example.com", "backend")),
    ]);
    let base = base_checkout(
        head.path(),
        &[
            ("profiles/backend.yaml", BACKEND),
            ("people/alice.yaml", &person("alice@example.com", "backend")),
            ("people/bob.yaml", &person("bob@example.com", "backend")),
        ],
    );
    let (code, result) = review(head.path(), base.path(), "plans", &[]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(
        plans(&result),
        vec![
            ("alice@example.com".into(), "move".into()),
            ("bob@example.com".into(), "leave".into())
        ]
    );
    let alice = saved(head.path(), "plans/000.json");
    assert_eq!(alice["profiles_after"], serde_json::json!([]));
    assert_eq!(alice["profiles_removed"], serde_json::json!(["backend"]));
}

#[test]
fn matching_reviewed_plan_becomes_execution_plan() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    snapshot(head.path());
    let base = base_checkout(head.path(), &[("profiles/backend.yaml", BACKEND)]);
    let (code, _) = review(head.path(), base.path(), "r1", &["--snapshot", "s.json"]);
    assert_eq!(code, Some(0));
    let (code, result) = review(
        head.path(),
        base.path(),
        "r2",
        &["--snapshot", "s.json", "--reviewed", "r1"],
    );
    assert_eq!(code, Some(0), "{result}");
    let execution = saved(head.path(), "r2/000.execution.json");
    let reviewed = fs::read(head.path().join("r1/000.json")).unwrap();
    assert_eq!(execution["reviewed_plan_sha256"], sha256(&reviewed));
    assert_eq!(execution["identity"], "bob@example.com");
    assert_eq!(result["result"]["rereview"], serde_json::json!([]));
    // The execution plan is a valid plan in its own right.
    fs::copy(
        head.path().join("r2/000.execution.json"),
        head.path().join("p.json"),
    )
    .unwrap();
    let (code, _) = verify(head.path(), &["--snapshot", "s.json"]);
    assert_eq!(code, Some(6));
}

#[test]
fn changed_access_after_review_requires_rereview() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &left("bob@example.com", "backend")),
    ]);
    let mut s = snapshot(head.path());
    let base = base_checkout(
        head.path(),
        &[
            ("profiles/backend.yaml", BACKEND),
            ("people/bob.yaml", &person("bob@example.com", "backend")),
        ],
    );
    let (code, result) = review(head.path(), base.path(), "r1", &["--snapshot", "s.json"]);
    assert_eq!(code, Some(0), "{result}");
    // Between review and merge, bob loses grant-3 and gains grant-97.
    data(&mut s)["grants"]
        .as_array_mut()
        .unwrap()
        .retain(|g| g["id"] != "grant-3");
    add_grant(&mut s, "grant-97", "101", "repo-20", "Write");
    save(head.path(), "s2.json", &s);
    let (code, result) = review(
        head.path(),
        base.path(),
        "r2",
        &["--snapshot", "s2.json", "--reviewed", "r1"],
    );
    assert_eq!(code, Some(6), "{result}");
    assert!(!head.path().join("r2/000.execution.json").exists());
    let rereview = &result["result"]["rereview"][0];
    assert_eq!(rereview["identity"], "bob@example.com");
    assert_eq!(rereview["added_steps"].as_array().unwrap().len(), 1);
    assert_eq!(rereview["removed_steps"].as_array().unwrap().len(), 1);
    let summary = fs::read_to_string(head.path().join("r2/summary.md")).unwrap();
    assert!(
        summary.contains("Not comparable with the reviewed plan: 1 people; re-review required"),
        "{summary}"
    );
}

#[test]
fn profile_definition_change_is_noted() {
    let alice = person("alice@example.com", "backend");
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/alice.yaml", &alice),
    ]);
    let base = base_checkout(
        head.path(),
        &[
            (
                "profiles/backend.yaml",
                "profile: backend\ngrants:\n  - {instance: demo, group: team-10}\n",
            ),
            ("people/alice.yaml", &alice),
        ],
    );
    let (code, result) = review(head.path(), base.path(), "plans", &[]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(result["result"]["profiles_changed"], true);
    assert!(plans(&result).is_empty());
    assert!(!head.path().join("plans/000.json").exists());
    let summary = fs::read_to_string(head.path().join("plans/summary.md")).unwrap();
    assert!(summary.contains("People affected: 0"), "{summary}");
    assert!(
        summary.contains("Profile definitions changed; run permesh profile check"),
        "{summary}"
    );
}

#[test]
fn reviewed_index_cannot_name_files_outside_its_directory() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let base = base_checkout(head.path(), &[("profiles/backend.yaml", BACKEND)]);
    let (code, _) = review(head.path(), base.path(), "r1", &[]);
    assert_eq!(code, Some(0));
    let mut index = saved(head.path(), "r1/index.json");
    index["plans"][0]["file"] = "../permesh.yaml".into();
    save(head.path(), "r1/index.json", &index);
    let (code, _) = review(head.path(), base.path(), "r2", &["--reviewed", "r1"]);
    assert_eq!(code, Some(2));
}

#[test]
fn missing_base_configuration_means_nothing_assigned_before() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let empty = tempfile::tempdir().unwrap();
    let (code, result) = review(head.path(), empty.path(), "plans", &[]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(
        plans(&result),
        vec![("bob@example.com".into(), "join".into())]
    );
}

#[test]
fn leave_for_unknown_identity_is_not_a_clean_plan() {
    let dir = demo(&[("profiles/backend.yaml", BACKEND)]);
    let (code, result) = access(
        dir.path(),
        &["leave", "bbo@example.com", "--output", "p.json"],
    );
    assert_eq!(code, Some(4), "{result}");
    assert_eq!(result["result"]["access_plan"]["state"], "not_found");
    let output = run(
        dir.path(),
        &["access", "leave", "bbo@example.com", "--output", "q.json"],
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("not found"), "{text}");
}

#[test]
fn leave_verification_reports_access_that_appeared_after_the_plan() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let mut s = snapshot(dir.path());
    access(
        dir.path(),
        &[
            "leave",
            "bob@example.com",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    data(&mut s)["grants"]
        .as_array_mut()
        .unwrap()
        .retain(|g| g["id"] != "grant-3");
    add_grant(&mut s, "grant-99", "101", "repo-20", "Admin");
    add_membership(&mut s, "101", "team-10");
    save(dir.path(), "after.json", &s);
    let (code, result) = verify(dir.path(), &["--snapshot", "after.json"]);
    assert_eq!(code, Some(6), "{result}");
    let grants: Vec<_> = result["result"]["extra"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["grant_id"].as_str().unwrap())
        .collect();
    assert_eq!(grants, vec!["grant-1", "grant-99"], "{result}");
}

#[test]
fn deleting_a_departed_persons_file_is_no_change() {
    let head = demo(&[("profiles/backend.yaml", BACKEND)]);
    let base = base_checkout(
        head.path(),
        &[
            ("profiles/backend.yaml", BACKEND),
            ("people/bob.yaml", &left("bob@example.com", "backend")),
        ],
    );
    let (code, result) = review(head.path(), base.path(), "plans", &[]);
    assert_eq!(code, Some(0), "{result}");
    assert!(plans(&result).is_empty(), "{result}");
}

#[test]
fn review_index_has_version() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let base = base_checkout(head.path(), &[("profiles/backend.yaml", BACKEND)]);
    let (code, _) = review(head.path(), base.path(), "plans", &[]);
    assert_eq!(code, Some(0));
    let index = saved(head.path(), "plans/index.json");
    assert_eq!(index["access_review_index_version"], 1, "{index}");
    assert_eq!(index["plans"][0]["identity"], "bob@example.com");
}

#[test]
fn bare_array_index_is_refused() {
    let head = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    snapshot(head.path());
    let base = base_checkout(head.path(), &[("profiles/backend.yaml", BACKEND)]);
    let (code, _) = review(head.path(), base.path(), "r1", &["--snapshot", "s.json"]);
    assert_eq!(code, Some(0));
    let index = saved(head.path(), "r1/index.json");
    let rows = index.get("plans").cloned().unwrap_or(index);
    save(head.path(), "r1/index.json", &rows);
    let (code, result) = review(
        head.path(),
        base.path(),
        "r2",
        &["--snapshot", "s.json", "--reviewed", "r1"],
    );
    assert_eq!(code, Some(2), "{result}");
    assert_eq!(
        result["error"]["message"],
        "Unsupported index.json format; regenerate the reviewed plans with this Permesh version"
    );
    assert!(!head.path().join("r2/000.execution.json").exists());
}
