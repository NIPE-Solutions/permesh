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
