// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const BACKEND: &str = "profile: backend\ngrants:\n  - {instance: demo, group: team-10}\n  - {instance: demo, role: Read, resource: repo-21}\n";
const READER: &str =
    "profile: reader\ngrants:\n  - {instance: demo, role: Read, resource: repo-21}\n";

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
fn identity<'a>(result: &'a Value, id: &str) -> &'a Value {
    result["identities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["identity"] == id)
        .unwrap()
}
fn check(dir: &Path, extra: &[&str]) -> (Option<i32>, Value) {
    let mut args = vec!["profile", "check", "--json"];
    args.extend_from_slice(extra);
    let output = run(dir, &args);
    (output.status.code(), report(&output)["result"].clone())
}
/// Alice holds only the `reader` profile, so her team path to repo-20 is unexplained.
fn reader_alice() -> tempfile::TempDir {
    demo(&[
        ("profiles/reader.yaml", READER),
        ("people/alice.yaml", &person("alice@example.com", "reader")),
    ])
}
fn team_exception(expires_at: &str) -> String {
    json!({"version":2,"rules":["inactive_access"],"exceptions":[{"id":"alice-team","rule":"unexplained_access","instance":"demo","account":"100","group":"team-10","reason":"Pairing on payments","owner":"it@example.com","expires_at":expires_at}]}).to_string()
}

#[test]
fn profile_check_reports_missing_and_unexplained_and_exits_6() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/alice.yaml", &person("alice@example.com", "backend")),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let (code, result) = check(dir.path(), &[]);
    assert_eq!(code, Some(6), "{result}");
    assert_eq!(result["profile_review_version"], 1);
    assert_eq!(result["enforce"], false);
    assert_eq!(result["complete"], true);
    let alice = identity(&result, "alice@example.com");
    assert_eq!(alice["missing"], json!([]));
    assert_eq!(alice["unexplained"], json!([]));
    let bob = identity(&result, "bob@example.com");
    assert_eq!(bob["missing"].as_array().unwrap().len(), 2, "{bob}");
    let unexplained = bob["unexplained"].as_array().unwrap();
    assert_eq!(unexplained.len(), 1);
    assert_eq!(unexplained[0]["grant_id"], "grant-3");
    assert_eq!(unexplained[0]["state"], "finding");
    assert_eq!(unexplained[0]["via"], json!([]));
    let counts = &result["counts"];
    assert_eq!(counts["covered"], 2);
    assert_eq!(counts["not_covered"], 2);
    assert_eq!(counts["missing"], 2);
    assert_eq!(counts["unexplained"], 1);
    assert_eq!(counts["cannot_tell"], 0);
    assert_eq!(counts["excepted"], 0);
}

#[test]
fn derived_path_shows_route() {
    let dir = reader_alice();
    let (code, result) = check(dir.path(), &[]);
    assert_eq!(code, Some(6), "{result}");
    let row = &identity(&result, "alice@example.com")["unexplained"][0];
    assert_eq!(row["resource"], "repo-20");
    assert_eq!(row["role"], "Write");
    assert_eq!(row["via"], json!(["team-10"]));
    assert_eq!(row["certainty"], "derived");
}

#[test]
fn fully_explained_people_exit_0() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/alice.yaml", &person("alice@example.com", "backend")),
    ]);
    let (code, result) = check(dir.path(), &[]);
    assert_eq!(code, Some(0), "{result}");
    assert_eq!(result["counts"]["covered"], 1);
}

#[test]
fn no_profiles_is_input_error() {
    let dir = demo(&[]);
    let output = run(dir.path(), &["profile", "check"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No access profiles are configured"),
        "{stderr}"
    );
}

#[test]
fn expired_exception_does_not_hide_unexplained() {
    let dir = reader_alice();
    fs::write(
        dir.path().join("policy.json"),
        team_exception("2020-01-01T00:00:00Z"),
    )
    .unwrap();
    let (code, result) = check(dir.path(), &["--policy", "policy.json"]);
    assert_eq!(code, Some(6), "{result}");
    let row = &identity(&result, "alice@example.com")["unexplained"][0];
    assert_eq!(row["state"], "finding");
    assert_eq!(result["counts"]["excepted"], 0);
}

#[test]
fn membership_exception_marks_excepted() {
    let dir = reader_alice();
    fs::write(
        dir.path().join("policy.json"),
        team_exception("2099-01-01T00:00:00Z"),
    )
    .unwrap();
    let (code, result) = check(dir.path(), &["--policy", "policy.json"]);
    assert_eq!(code, Some(0), "{result}");
    let row = &identity(&result, "alice@example.com")["unexplained"][0];
    assert_eq!(row["state"], "excepted");
    assert_eq!(row["exception"]["id"], "alice-team");
    assert_eq!(result["counts"]["excepted"], 1);
    assert_eq!(result["counts"]["unexplained"], 0);
}

#[test]
fn saved_snapshot_is_accepted() {
    let dir = reader_alice();
    assert!(
        run(dir.path(), &["snapshot", "create", "--output", "snap.json"])
            .status
            .success()
    );
    let (code, result) = check(dir.path(), &["--snapshot", "snap.json"]);
    assert_eq!(code, Some(6), "{result}");
    assert_eq!(result["counts"]["unexplained"], 1);
}

#[test]
fn human_output_lists_people_and_routes() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("profiles/reader.yaml", READER),
        ("people/alice.yaml", &person("alice@example.com", "reader")),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let output = run(dir.path(), &["profile", "check"]);
    assert_eq!(output.status.code(), Some(6));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for text in ["bob@example.com", "unexplained", "via team-10"] {
        assert!(stdout.contains(text), "{text} in {stdout}");
    }
}

fn suggest(dir: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["profile", "suggest", "--name", "backend"];
    args.extend_from_slice(extra);
    run(dir, &args)
}

#[test]
fn suggest_prints_draft_that_loads_as_profile() {
    let dir = demo(&[]);
    let output = suggest(dir.path(), &["--group", "demo:team-10"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("# Draft from demo group team/backend: 1 members (0 unresolved, excluded). Review before committing.\n"), "{stdout}");
    assert!(
        stdout.contains("# team/backend: 1 of 1 members"),
        "{stdout}"
    );
    fs::create_dir_all(dir.path().join("profiles")).unwrap();
    fs::write(dir.path().join("profiles/backend.yaml"), &stdout).unwrap();
    fs::create_dir_all(dir.path().join("people")).unwrap();
    fs::write(
        dir.path().join("people/alice.yaml"),
        person("alice@example.com", "backend"),
    )
    .unwrap();
    let (code, result) = check(dir.path(), &[]);
    assert_eq!(code, Some(0), "{result}");
}

#[test]
fn suggest_json_has_counts() {
    let dir = demo(&[]);
    let output = suggest(dir.path(), &["--group", "demo:team-10", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let result = &report(&output)["result"];
    assert_eq!(result["profile_suggestion_version"], 1);
    assert_eq!(result["group"]["id"], "team-10");
    assert_eq!(result["members"], 1);
    assert_eq!(result["unresolved_members"], 0);
    let entries = result["entries"].as_array().unwrap();
    assert!(
        entries.iter().any(|e| e["entry"]["kind"] == "group"
            && e["entry"]["group"] == "team-10"
            && e["count"] == 1),
        "{result}"
    );
    assert!(
        result["draft"]
            .as_str()
            .unwrap()
            .contains("profile: backend")
    );
}

#[test]
fn bad_group_argument_is_input_error() {
    let dir = demo(&[]);
    for group in ["team-10", ":team-10", "demo:"] {
        assert_eq!(
            suggest(dir.path(), &["--group", group]).status.code(),
            Some(2),
            "{group}"
        );
    }
    let output = run(
        dir.path(),
        &[
            "profile",
            "suggest",
            "--group",
            "demo:team-10",
            "--name",
            "bad name",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn unknown_group_exits_1() {
    let dir = demo(&[]);
    assert_eq!(
        suggest(dir.path(), &["--group", "demo:team-99"])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn ambiguous_alias_is_incomplete_and_shown() {
    let dir = demo(&[
        ("profiles/backend.yaml", BACKEND),
        ("people/bob.yaml", &person("bob@example.com", "backend")),
    ]);
    let config = fs::read_to_string(dir.path().join("permesh.yaml")).unwrap();
    let config = config.replace(
        "aliases: {}",
        "aliases:\n    alice@example.com:\n      demo: ['101']",
    );
    fs::write(dir.path().join("permesh.yaml"), config).unwrap();
    let (code, result) = check(dir.path(), &[]);
    assert_eq!(code, Some(4), "{result}");
    assert_eq!(result["counts"]["ambiguous_accounts"], 1);
    let output = run(dir.path(), &["profile", "check"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 accounts claimed by more than one identity"),
        "{stdout}"
    );
    assert!(stdout.contains("ambiguous account"), "{stdout}");
}
