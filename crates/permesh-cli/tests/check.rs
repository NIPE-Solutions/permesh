// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .args(args)
        .output()
        .unwrap()
}
fn demo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    dir
}
fn report(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn demo_check_reports_departed_access_and_exits_6() {
    let dir = demo();
    let output = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
    );
    assert_eq!(output.status.code(), Some(6));
    let report = report(&output);
    assert_eq!(report["command"], "check");
    let result = &report["result"];
    assert_eq!(result["check_report_version"], 1);
    assert_eq!(result["complete"], true);
    assert_eq!(result["counts"]["departed_with_access"], 1);
    assert_eq!(result["counts"]["policy_finding"], 0);
    assert_eq!(result["changes"], "no_baseline");
    for file in ["snapshot.json", "check.json", "summary.md"] {
        assert!(dir.path().join("out").join(file).is_file(), "{file}");
    }
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("out/check.json")).unwrap()).unwrap();
    assert_eq!(saved["counts"], result["counts"]);
    assert!(
        run(dir.path(), &["snapshot", "inspect", "out/snapshot.json"])
            .status
            .success()
    );
}

#[test]
fn summary_contains_counts_but_no_names() {
    let dir = demo();
    run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    let summary = fs::read_to_string(dir.path().join("out/summary.md")).unwrap();
    assert!(
        summary.contains("Departed people with access: 1"),
        "{summary}"
    );
    for name in ["former-dev", "alice", "bob", "@example.com", "acme/"] {
        assert!(!summary.contains(name), "{name} in {summary}");
    }
}

#[test]
fn human_output_lists_counts_and_files() {
    let dir = demo();
    let output = run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    assert_eq!(output.status.code(), Some(6));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Departed people with access: 1"), "{text}");
    assert!(text.contains("summary.md"), "{text}");
}

#[test]
fn existing_output_requires_overwrite() {
    let dir = demo();
    run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    let before = fs::read(dir.path().join("out/check.json")).unwrap();
    let again = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
    );
    assert_eq!(again.status.code(), Some(2));
    assert_eq!(fs::read(dir.path().join("out/check.json")).unwrap(), before);
    let forced = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--overwrite"],
    );
    assert_eq!(forced.status.code(), Some(6));
}

#[test]
fn out_cannot_be_workspace_directory() {
    let dir = demo();
    let before = fs::read(dir.path().join("permesh.yaml")).unwrap();
    let output = run(
        dir.path(),
        &["check", "--out", ".", "--no-notify", "--json"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(dir.path().join("permesh.yaml")).unwrap(), before);
    assert!(!dir.path().join("check.json").exists());
}

#[test]
fn explicit_policy_file_is_used_and_departed_is_independent_of_it() {
    let dir = demo();
    fs::write(
        dir.path().join("policy.json"),
        r#"{"version":1,"rules":["external_privileged"]}"#,
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "check",
            "--out",
            "out",
            "--policy",
            "policy.json",
            "--no-notify",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(6));
    let result = &report(&output)["result"];
    assert_eq!(result["counts"]["policy_finding"], 0);
    assert_eq!(result["counts"]["departed_with_access"], 1);
    assert_eq!(
        result["policy"]["rules"],
        serde_json::json!(["external_privileged"])
    );
}

/// Run once into `a/`, then return the saved snapshot for editing as a baseline.
fn baseline(dir: &Path) -> serde_json::Value {
    run(dir, &["check", "--out", "a", "--no-notify"]);
    serde_json::from_slice(&fs::read(dir.join("a/snapshot.json")).unwrap()).unwrap()
}
fn save(dir: &Path, name: &str, snapshot: &serde_json::Value) {
    fs::write(dir.join(name), serde_json::to_vec_pretty(snapshot).unwrap()).unwrap();
}
fn check_against(dir: &Path, previous: &str) -> (Option<i32>, serde_json::Value) {
    let output = run(
        dir,
        &[
            "check",
            "--out",
            "b",
            "--previous",
            previous,
            "--no-notify",
            "--json",
        ],
    );
    (output.status.code(), report(&output)["result"].clone())
}

#[test]
fn identical_baseline_reports_no_admin_changes() {
    let dir = demo();
    baseline(dir.path());
    let (code, result) = check_against(dir.path(), "a/snapshot.json");
    assert_eq!(code, Some(6));
    let compared = &result["changes"]["compared"];
    assert_eq!(compared["new_admins"], serde_json::json!([]));
    assert_eq!(compared["removed_admins"], serde_json::json!([]));
    assert_eq!(result["counts"]["new_admin"], 0);
}

#[test]
fn new_admin_grant_is_reported() {
    let dir = demo();
    let mut snapshot = baseline(dir.path());
    let grants = snapshot["providers"][0]["data"]["grants"]
        .as_array_mut()
        .unwrap();
    let before = grants.len();
    grants.retain(|g| g["privilege"] != "admin" && g["privilege"] != "owner");
    assert!(grants.len() < before, "demo has an admin grant");
    save(dir.path(), "previous.json", &snapshot);
    let (code, result) = check_against(dir.path(), "previous.json");
    assert_eq!(code, Some(6));
    assert_eq!(result["counts"]["new_admin"], 1);
    let added = &result["changes"]["compared"]["new_admins"][0];
    assert_eq!(added["login"], "bob-admin");
}

#[test]
fn vanished_identity_is_needs_review_not_departed() {
    let dir = demo();
    let mut snapshot = baseline(dir.path());
    let identities = snapshot["providers"][0]["data"]["identities"]
        .as_array_mut()
        .unwrap();
    let mut extra = identities[0].clone();
    extra["id"] = "vanished@example.com".into();
    extra["verified_emails"] = serde_json::json!(["vanished@example.com"]);
    identities.push(extra);
    save(dir.path(), "previous.json", &snapshot);
    let (_, result) = check_against(dir.path(), "previous.json");
    assert_eq!(result["counts"]["no_longer_observed"], 1);
    assert_eq!(result["counts"]["departed_with_access"], 1);
}

#[test]
fn different_provider_set_is_inconclusive_not_unchanged() {
    let dir = demo();
    let mut snapshot = baseline(dir.path());
    let text = serde_json::to_string(&snapshot)
        .unwrap()
        .replace("\"demo\"", "\"other\"");
    snapshot = serde_json::from_str(&text).unwrap();
    save(dir.path(), "previous.json", &snapshot);
    let (code, result) = check_against(dir.path(), "previous.json");
    assert_eq!(code, Some(4));
    assert_eq!(result["complete"], false);
    let reasons = result["changes"]["inconclusive"]["reasons"]
        .as_array()
        .unwrap();
    assert!(
        reasons.iter().any(|r| r == "provider_set_changed"),
        "{reasons:?}"
    );
    let summary = fs::read_to_string(dir.path().join("b/summary.md")).unwrap();
    assert!(summary.contains("inconclusive"), "{summary}");
}

#[test]
fn missing_previous_file_is_input_error() {
    let dir = demo();
    let output = run(
        dir.path(),
        &[
            "check",
            "--out",
            "b",
            "--previous",
            "nope.json",
            "--no-notify",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!dir.path().join("b").exists());
}

fn with_hook(dir: &Path, extra: &str) {
    let path = dir.join("permesh.yaml");
    let mut config = fs::read_to_string(&path).unwrap();
    config.push_str(&format!(
        "check:\n  notifications:\n    - {{id: hook, type: webhook, url: 'env://PERMESH_TEST_HOOK'{extra}}}\n"
    ));
    fs::write(path, config).unwrap();
}
/// Loopback listener: answers 200 (or never, when `hang`) and returns the request body.
fn listener(hang: bool) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/hook", server.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        server.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let stream = loop {
            match server.accept() {
                Ok((stream, _)) => break stream,
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(_) => return String::new(),
            }
        };
        stream.set_nonblocking(false).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap();
            }
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        if hang {
            std::thread::sleep(std::time::Duration::from_secs(15));
        } else {
            let mut stream = stream;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
        }
        String::from_utf8(body).unwrap()
    });
    (url, handle)
}
fn run_env(dir: &Path, args: &[&str], hook: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_permesh"));
    command
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .env_remove("PERMESH_TEST_HOOK")
        .args(args);
    if let Some(hook) = hook {
        command.env("PERMESH_TEST_HOOK", hook);
    }
    command.output().unwrap()
}

#[test]
fn delivers_summary_to_configured_webhook() {
    let dir = demo();
    with_hook(dir.path(), "");
    let (url, handle) = listener(false);
    let output = run_env(dir.path(), &["check", "--out", "out", "--json"], Some(&url));
    assert_eq!(output.status.code(), Some(6));
    let body: serde_json::Value = serde_json::from_str(&handle.join().unwrap()).unwrap();
    assert_eq!(body["counts"]["departed_with_access"], 1);
    assert!(!body.to_string().contains("former-dev"));
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("out/check.json")).unwrap()).unwrap();
    assert_eq!(saved["notifications"][0]["delivered"], true);
    assert_eq!(
        report(&output)["result"]["notifications"][0]["delivered"],
        true
    );
}

#[test]
fn unset_webhook_reference_fails_softly() {
    let dir = demo();
    with_hook(dir.path(), "");
    let output = run_env(dir.path(), &["check", "--out", "out", "--json"], None);
    assert_eq!(output.status.code(), Some(6));
    let delivery = &report(&output)["result"]["notifications"][0];
    assert_eq!(delivery["delivered"], false);
    assert_eq!(delivery["skipped"], false);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.contains("PERMESH_TEST_HOOK"), "{text}");
    assert!(!text.contains("127.0.0.1"), "{text}");
}

#[test]
fn hanging_endpoint_times_out() {
    let dir = demo();
    with_hook(dir.path(), "");
    let (url, _handle) = listener(true);
    let started = std::time::Instant::now();
    let output = run_env(dir.path(), &["check", "--out", "out", "--json"], Some(&url));
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(output.status.code(), Some(6));
    let delivery = &report(&output)["result"]["notifications"][0];
    assert_eq!(delivery["delivered"], false);
    assert!(
        delivery["reason"].as_str().unwrap().contains("timed out"),
        "{delivery}"
    );
}

#[test]
fn no_notify_skips_all_channels() {
    let dir = demo();
    with_hook(dir.path(), "");
    let output = run_env(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
        Some("http://127.0.0.1:9/never"),
    );
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(
        report(&output)["result"]["notifications"][0]["skipped"],
        true
    );
}

#[test]
fn on_filter_skips_unmatched_channel() {
    let dir = demo();
    with_hook(dir.path(), ", on: [new_admin]");
    let output = run_env(
        dir.path(),
        &["check", "--out", "out", "--json"],
        Some("http://127.0.0.1:9/never"),
    );
    assert_eq!(
        report(&output)["result"]["notifications"][0]["skipped"],
        true
    );
}

#[test]
fn without_baseline_change_counts_are_not_compared_rather_than_zero() {
    let dir = demo();
    let output = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
    );
    let counts = &report(&output)["result"]["counts"];
    for kind in ["new_admin", "removed_admin", "no_longer_observed"] {
        assert!(counts.get(kind).is_none(), "{kind} present: {counts}");
    }
    let summary = fs::read_to_string(dir.path().join("out/summary.md")).unwrap();
    assert!(summary.contains("- New admins: not compared"), "{summary}");
    assert!(!summary.contains("New admins: 0"), "{summary}");
    let human = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--overwrite"],
    );
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("New admins: not compared"), "{text}");
    assert!(
        text.contains("Changes: no previous run to compare with"),
        "{text}"
    );
}

#[test]
fn inconclusive_comparison_explains_itself_in_human_output() {
    let dir = demo();
    let snapshot = baseline(dir.path());
    let text = serde_json::to_string(&snapshot)
        .unwrap()
        .replace("\"demo\"", "\"other\"");
    fs::write(dir.path().join("previous.json"), text).unwrap();
    let output = run(
        dir.path(),
        &[
            "check",
            "--out",
            "b",
            "--previous",
            "previous.json",
            "--no-notify",
        ],
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("New admins: not compared"), "{text}");
    assert!(text.contains("provider_set_changed"), "{text}");
}
