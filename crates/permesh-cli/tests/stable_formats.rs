// SPDX-License-Identifier: MIT
//! Golden fixtures for the formats frozen by the second stability declaration
//! (ADR 0036). See fixtures/stable/README.md before changing any fixture.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const BACKEND: &str = "profile: backend\ngrants:\n  - {instance: demo, group: team-10}\n  - {instance: demo, role: Read, resource: repo-21}\n";
const BASE: &str = "profile: base\ngrants:\n  - {instance: demo, role: Read, resource: repo-21}\n";
const NORMALIZED: &str = "<normalized>";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .args(args)
        .output()
        .unwrap()
}
fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{output:?}"))
}
fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}
fn read(dir: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(dir.join(name)).unwrap()).unwrap()
}
/// A demo workspace where bob holds `backend` and alice holds `base`.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    write(dir.path(), "profiles/backend.yaml", BACKEND);
    write(dir.path(), "profiles/base.yaml", BASE);
    write(
        dir.path(),
        "people/bob.yaml",
        "identity: bob@example.com\nprofiles: [backend]\n",
    );
    write(
        dir.path(),
        "people/alice.yaml",
        "identity: alice@example.com\nprofiles: [base]\n",
    );
    dir
}
fn stable(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stable")
        .join(name)
}

/// Only run-specific values change between runs: times, durations, digests over them and
/// absolute paths. Everything else, including order and nulls, is compared exactly.
fn normalize(root: &Path, value: &mut Value) {
    // Any temporary directory (workspace, base checkout) is run-specific.
    let roots: Vec<String> = [root.to_path_buf(), std::env::temp_dir()]
        .iter()
        .flat_map(|path| [path.clone(), path.canonicalize().unwrap()])
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    fn walk(roots: &[String], key: Option<&str>, value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, value) in map.iter_mut() {
                    walk(roots, Some(key), value);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|item| walk(roots, key, item)),
            Value::String(text) => {
                let time = key.is_some_and(|key| key.ends_with("_at"));
                let digest =
                    key.is_some_and(|key| key.ends_with("sha256") || key.ends_with("digest"));
                // Shapes stay frozen even where values are normalized.
                if time {
                    assert!(
                        text.len() >= 20 && text.as_bytes()[10] == b'T' && text.ends_with('Z'),
                        "{key:?} is not an RFC 3339 UTC time: {text}"
                    );
                }
                if digest {
                    assert!(
                        text.len() == 64
                            && text
                                .bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                        "{key:?} is not a lowercase SHA-256 hex digest: {text}"
                    );
                }
                let run_specific = time || digest;
                if run_specific || roots.iter().any(|root| text.starts_with(root.as_str())) {
                    *value = json!(NORMALIZED);
                }
            }
            Value::Number(_) if key.is_some_and(|key| key.ends_with("_ms")) => {
                *value = json!(NORMALIZED);
            }
            _ => {}
        }
    }
    walk(&roots, None, value);
}

/// Compares without writing; `Err` carries the reason.
fn compare(name: &str, value: &Value) -> Result<(), String> {
    let expected: Value = serde_json::from_slice(
        &fs::read(stable(name)).map_err(|_| format!("missing fixture {name}"))?,
    )
    .map_err(|_| format!("fixture {name} is not JSON"))?;
    if &expected == value {
        Ok(())
    } else {
        Err(format!(
            "{name} differs from the frozen format; a change needs a version bump and migration (fixtures/stable/README.md).\nactual: {}",
            serde_json::to_string_pretty(value).unwrap()
        ))
    }
}
/// `PERMESH_BLESS=1` writes the fixture instead; only for an intended, reviewed change.
fn assert_fixture(name: &str, value: &Value) {
    if std::env::var_os("PERMESH_BLESS").is_some() {
        let mut text = serde_json::to_string_pretty(value).unwrap();
        text.push('\n');
        fs::write(stable(name), text).unwrap();
        return;
    }
    if let Err(message) = compare(name, value) {
        panic!("{message}");
    }
}
fn normalized(root: &Path, mut value: Value) -> Value {
    normalize(root, &mut value);
    value
}

#[test]
fn check_report_fixture() {
    let dir = workspace();
    let output = run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    assert_eq!(output.status.code(), Some(6), "{output:?}");
    let report = read(dir.path(), "out/check.json");
    assert_eq!(report["complete"], true);
    assert_fixture("check-report.json", &normalized(dir.path(), report));
}

#[test]
fn check_report_incomplete_fixture() {
    let dir = workspace();
    let config = fs::read_to_string(dir.path().join("permesh.yaml")).unwrap();
    let config = config.replace(
        "aliases: {}",
        "aliases:\n    bob@example.com:\n      demo: ['100']",
    );
    fs::write(dir.path().join("permesh.yaml"), config).unwrap();
    let output = run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let report = read(dir.path(), "out/check.json");
    assert_eq!(report["complete"], false);
    assert_fixture(
        "check-report-incomplete.json",
        &normalized(dir.path(), report),
    );
}

#[test]
fn profile_check_fixture() {
    let dir = workspace();
    let report = json(&run(dir.path(), &["profile", "check", "--json"]));
    assert_eq!(report["command"], "profile_check");
    assert_fixture(
        "profile-check.json",
        &normalized(dir.path(), report["result"].clone()),
    );
}

#[test]
fn profile_suggest_fixture() {
    let dir = workspace();
    let report = json(&run(
        dir.path(),
        &[
            "profile",
            "suggest",
            "--group",
            "demo:team-10",
            "--name",
            "team",
            "--json",
        ],
    ));
    assert_eq!(report["command"], "profile_suggest");
    let mut result = report["result"].clone();
    // The draft is review text (human output, not frozen); it must stay a loadable profile.
    let draft = result["draft"].as_str().unwrap().to_owned();
    write(dir.path(), "profiles/team.yaml", &draft);
    assert_ne!(
        run(dir.path(), &["profile", "check"]).status.code(),
        Some(2)
    );
    result["draft"] = json!(NORMALIZED);
    assert_fixture("profile-suggest.json", &normalized(dir.path(), result));
}

#[test]
fn config_with_every_new_key_parses() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(
        stable("config-check-profiles.yaml"),
        dir.path().join("permesh.yaml"),
    )
    .unwrap();
    write(dir.path(), "profiles/backend.yaml", BACKEND);
    let config = permesh_config::Config::load(&dir.path().join("permesh.yaml")).unwrap();
    let value = serde_json::to_value(&config).unwrap();
    assert_fixture("config-check-profiles.json", &value);
}

#[test]
fn policy_v2_fixture_is_applied() {
    let dir = workspace();
    fs::copy(stable("policy-v2.json"), dir.path().join("policy.json")).unwrap();
    let report = json(&run(
        dir.path(),
        &["profile", "check", "--policy", "policy.json", "--json"],
    ));
    assert_eq!(report["result"]["counts"]["excepted"], 1, "{report}");
}

#[test]
fn fixture_comparison_detects_renamed_field() {
    let mut value: Value =
        serde_json::from_slice(&fs::read(stable("profile-check.json")).unwrap()).unwrap();
    assert!(compare("profile-check.json", &value).is_ok());
    let counts = value["counts"].as_object_mut().unwrap();
    let covered = counts.remove("covered").unwrap();
    counts.insert("covered_count".into(), covered);
    assert!(compare("profile-check.json", &value).is_err());
}

fn snapshot(dir: &Path) {
    let output = run(dir, &["snapshot", "create", "--output", "s.json"]);
    assert!(output.status.success(), "{output:?}");
}
fn access(dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let mut all = vec!["access"];
    all.extend_from_slice(args);
    all.push("--json");
    let output = run(dir, &all);
    (output.status.code(), json(&output))
}

#[test]
fn access_plan_join_fixture() {
    let dir = workspace();
    snapshot(dir.path());
    let (code, report) = access(
        dir.path(),
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
    assert_eq!(code, Some(0), "{report}");
    assert_fixture(
        "access-plan-join.json",
        &normalized(dir.path(), read(dir.path(), "p.json")),
    );
    assert_fixture(
        "access-plan-result.json",
        &normalized(dir.path(), report["result"].clone()),
    );
}

#[test]
fn access_plan_leave_fixture() {
    let dir = workspace();
    snapshot(dir.path());
    let (code, report) = access(
        dir.path(),
        &[
            "leave",
            "alice@example.com",
            "--snapshot",
            "s.json",
            "--output",
            "p.json",
        ],
    );
    assert_eq!(code, Some(0), "{report}");
    assert_fixture(
        "access-plan-leave.json",
        &normalized(dir.path(), read(dir.path(), "p.json")),
    );
}

fn joined_and_verified(partial: bool) -> (Option<i32>, Value, tempfile::TempDir) {
    let dir = workspace();
    snapshot(dir.path());
    let (code, _) = access(
        dir.path(),
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
    assert_eq!(code, Some(0));
    if partial {
        let mut s = read(dir.path(), "s.json");
        let limitations = json!(["Fixture visibility incomplete"]);
        s["providers"][0]["state"] = "partial".into();
        s["providers"][0]["limitations"] = limitations.clone();
        s["providers"][0]["data"]["complete"] = false.into();
        s["providers"][0]["data"]["limitations"] = limitations;
        fs::write(dir.path().join("s.json"), s.to_string()).unwrap();
    }
    let (code, report) = access(dir.path(), &["verify", "p.json", "--snapshot", "s.json"]);
    (code, report, dir)
}

#[test]
fn access_verify_fixture() {
    let (code, report, dir) = joined_and_verified(false);
    assert_eq!(code, Some(6), "{report}");
    assert_fixture(
        "access-verify.json",
        &normalized(dir.path(), report["result"].clone()),
    );
}

#[test]
fn access_verify_partial_fixture() {
    let (code, report, dir) = joined_and_verified(true);
    assert_eq!(code, Some(4), "{report}");
    assert_eq!(report["complete"], false);
    assert_fixture(
        "access-verify-partial.json",
        &normalized(dir.path(), report["result"].clone()),
    );
}

#[test]
fn access_review_and_execution_fixtures() {
    let head = workspace();
    snapshot(head.path());
    let base = tempfile::tempdir().unwrap();
    fs::copy(
        head.path().join("permesh.yaml"),
        base.path().join("permesh.yaml"),
    )
    .unwrap();
    write(base.path(), "profiles/backend.yaml", BACKEND);
    write(base.path(), "profiles/base.yaml", BASE);
    write(
        base.path(),
        "people/alice.yaml",
        "identity: alice@example.com\nprofiles: [base]\n",
    );
    let base_dir = base.path().display().to_string();
    let review = |out: &str, extra: &[&str]| {
        let mut args = vec![
            "review",
            "--base",
            base_dir.as_str(),
            "--out",
            out,
            "--snapshot",
            "s.json",
        ];
        args.extend_from_slice(extra);
        access(head.path(), &args)
    };
    let (code, report) = review("r1", &[]);
    assert_eq!(code, Some(0), "{report}");
    assert_fixture(
        "access-review-index.json",
        &normalized(head.path(), read(head.path(), "r1/index.json")),
    );
    assert_fixture(
        "access-review-result.json",
        &normalized(head.path(), report["result"].clone()),
    );
    let (code, report) = review("r2", &["--reviewed", "r1"]);
    assert_eq!(code, Some(0), "{report}");
    assert_fixture(
        "access-execution-plan.json",
        &normalized(head.path(), read(head.path(), "r2/000.execution.json")),
    );
}
