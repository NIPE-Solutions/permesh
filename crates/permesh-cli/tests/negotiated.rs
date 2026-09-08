// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root)
        .args(args)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .env("NO_COLOR", "1")
        .env_remove("PERMESH_NEGOTIATED_ABSENT")
        .output()
        .unwrap()
}
fn success(root: &Path, args: &[&str]) -> Value {
    let out = run(root, args);
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}
fn write_config(root: &Path, config: &Value) {
    std::fs::write(
        root.join("permesh.yaml"),
        serde_json::to_vec(config).unwrap(),
    )
    .unwrap();
}
fn approve(root: &Path) -> Value {
    let review = success(
        root,
        &["provider", "external", "review", "instance", "--json"],
    );
    success(
        root,
        &[
            "provider",
            "external",
            "approve",
            "instance",
            "--fingerprint",
            review["result"]["fingerprint"].as_str().unwrap(),
            "--accept-risk",
            "--json",
        ],
    );
    review
}
fn fixture(root: &Path) -> Value {
    // A small native peer exercises the real trust path on all supported platforms.
    // Trusting the CLI test executable itself can exceed the executable size bound.
    let source = root.join("peer.rs");
    let binary = root.join(if cfg!(windows) { "peer.exe" } else { "peer" });
    std::fs::write(&source, r##"
use std::io::{BufRead, Write};
fn main() {
    let mut lines = std::io::stdin().lock().lines();
    let request = lines.next().unwrap().unwrap();
    assert!(request.contains("\"protocol_version\":1"));
    let check = request.contains("\"operation\":\"check\"");
    assert!(check || request.contains("\"operation\":\"discover\""));
    println!("{}", r#"{"protocol_version":1,"id":"handshake","event":"handshake","provider":"fixture","capabilities":["accounts","identities","resources","grants"],"operations":["check","discover"],"draft":true}"#);
    std::io::stdout().flush().unwrap();
    let request = lines.next().unwrap().unwrap();
    assert!(request.contains("\"protocol_version\":1"));
    assert!(request.contains("\"configuration\":{}"));
    assert!(request.contains("\"credentials\":{}"));
    if check {
        assert!(request.contains("\"method\":\"check\""));
        println!("{}", r#"{"protocol_version":1,"id":"check","event":"health","status":"ok","limitations":[]}"#);
    } else {
        assert!(request.contains("\"method\":\"discover\""));
        for record in [
            r#"{"protocol_version":1,"id":"discover","event":"record","kind":"identity","data":{"id":"robot@example.com","kind":"service","affiliation":"external","status":"inactive","verified_emails":["robot@example.com"]}}"#,
            r#"{"protocol_version":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"instance","id":"robot"},"login":"robot","kind":"service","affiliation":"external","status":"inactive","verified_emails":["robot@example.com"]}}"#,
            r#"{"protocol_version":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"instance","id":"org"},"name":"Organization","kind":"fixture.organization","parent":null}}"#,
            r#"{"protocol_version":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"instance","id":"repo"},"name":"Repository","kind":"fixture.repository","parent":{"provider":"instance","id":"org"}}}"#,
            r#"{"protocol_version":1,"id":"discover","event":"record","kind":"grant","data":{"id":"policy","subject":{"kind":"account","key":{"provider":"instance","id":"robot"}},"resource":{"provider":"instance","id":"repo"},"role":"Reader","privilege":"standard","certainty":"derived","evidence_kind":"policy_attachment","provenance":{"method":"synthetic fixture","observed_at":"2026-01-01T00:00:00Z"}}}"#,
            r#"{"protocol_version":1,"id":"discover","event":"complete","count":5,"complete":true,"limitations":[]}"#,
        ] { println!("{record}"); }
    }
    std::io::stdout().flush().unwrap();
    assert!(lines.next().is_none());
}
"##).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let digest = Sha256::digest(std::fs::read(&binary).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    success(
        root,
        &[
            "provider",
            "external",
            "trust",
            binary.to_str().unwrap(),
            "--id",
            "fixture",
            "--sha256",
            &digest,
            "--capability",
            "accounts",
            "--capability",
            "identities",
            "--capability",
            "resources",
            "--capability",
            "grants",
            "--accept-risk",
            "--json",
        ],
    );
    json!({"version":1,"organization":{"name":"Negotiated fixture"},
        "providers":[{"id":"instance","type":"external","external":{
            "provider":"fixture","sha256":digest,"configuration":{},"credentials":{}}}],
        "identity":{"sources":[{"provider":"instance","authoritative":true}],"aliases":{}}})
}

#[test]
fn approved_negotiated_workspace_preserves_rich_schema_two_and_schema_one_health() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut config = fixture(&root);
    config["providers"][0]["external"]["discovery_protocol"] = json!("negotiated_v1");
    write_config(&root, &config);
    let review = approve(&root);
    assert_eq!(review["result"]["discovery_protocol"], "negotiated_v1");
    let human = run(&root, &["provider", "external", "review", "instance"]);
    assert!(human.status.success(), "{human:?}");
    assert!(String::from_utf8_lossy(&human.stdout).contains("negotiated_v1"));
    let report = success(&root, &["user", "robot@example.com", "--json"]);
    assert_eq!(report["schema_version"], 2);
    assert_eq!(report["complete"], true);
    let result = &report["result"];
    for entity in [&result["identity"], &result["accounts"][0]] {
        assert_eq!(entity["kind"], "service");
        assert_eq!(entity["affiliation"], "external");
        assert_eq!(entity["status"], "inactive");
    }
    let access = &result["access"][0];
    assert_eq!(access["resource"]["kind"], "fixture.repository");
    assert_eq!(
        access["resource"]["parent"],
        json!({"provider":"instance","id":"org"})
    );
    assert_eq!(access["grant"]["evidence_kind"], "policy_attachment");
    assert_eq!(access["grant"]["certainty"], "derived");
    assert_eq!(access["certainty"], "derived");
    let health = success(&root, &["doctor", "--json"]);
    assert_eq!(health["schema_version"], 1);
    assert_eq!(health["complete"], true);
}

#[test]
fn changing_protocol_invalidates_approval_before_credentials_are_resolved() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut config = fixture(&root);
    config["providers"][0]["external"]["credentials"] =
        json!({"token":"env://PERMESH_NEGOTIATED_ABSENT"});
    write_config(&root, &config);
    let legacy = approve(&root);
    assert!(legacy["result"].get("discovery_protocol").is_none());
    let human = run(&root, &["provider", "external", "review", "instance"]);
    assert!(human.status.success());
    assert!(!String::from_utf8_lossy(&human.stdout).contains("Discovery protocol"));
    config["providers"][0]["external"]["discovery_protocol"] = json!("negotiated_v1");
    write_config(&root, &config);
    let review = success(
        &root,
        &["provider", "external", "review", "instance", "--json"],
    );
    assert_ne!(
        review["result"]["fingerprint"],
        legacy["result"]["fingerprint"]
    );
    assert_eq!(review["result"]["approved"], false);
    for args in [
        vec!["user", "robot@example.com", "--json"],
        vec!["doctor", "--json"],
    ] {
        let out = run(&root, &args);
        assert!(!out.status.success(), "{out:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("approval is missing or stale"), "{out:?}");
        assert!(!text.contains("credential unavailable"), "{out:?}");
    }
    approve(&root);
    let out = run(&root, &["user", "robot@example.com", "--json"]);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("credential unavailable"),
        "{out:?}"
    );
}
