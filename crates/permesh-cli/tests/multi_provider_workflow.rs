// SPDX-License-Identifier: MIT
//! Synthetic protocol peers, not live Google/GitHub API qualification.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let out = run(root, args);
    assert!(out.status.success(), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}
fn approve(root: &Path, id: &str) {
    let review = ok(root, &["provider", "external", "review", id]);
    ok(
        root,
        &[
            "provider",
            "external",
            "approve",
            id,
            "--fingerprint",
            review["result"]["fingerprint"].as_str().unwrap(),
            "--accept-risk",
        ],
    );
}
fn save(root: &Path, config: &Value) {
    std::fs::write(
        root.join("permesh.yaml"),
        serde_json::to_vec(config).unwrap(),
    )
    .unwrap();
}
fn key(provider: &str, id: &str) -> Value {
    json!({"provider":provider,"id":id})
}
fn account(
    provider: &str,
    id: &str,
    login: &str,
    kind: &str,
    status: &str,
    emails: Value,
) -> Value {
    json!({"kind":"account","data":{"key":key(provider,id),"login":login,"kind":kind,"affiliation":"unknown","status":status,"verified_emails":emails}})
}
fn grant(provider: &str, id: &str, subject: Value, role: &str, privilege: &str) -> Value {
    json!({"kind":"grant","data":{"id":id,"subject":subject,"resource":key(provider,"repository:20"),"role":role,"privilege":privilege,"certainty":"observed","evidence_kind": if id == "account-access" { "permission" } else { "assignment" },"provenance":{"method":"synthetic multi-provider fixture","observed_at":"2026-09-09T00:00:00Z"}}})
}
fn transcript(records: Vec<Value>) -> String {
    records
        .into_iter()
        .map(|record| {
            let mut value = json!({"protocol_version":1,"id":"discover","event":"record"});
            value
                .as_object_mut()
                .unwrap()
                .extend(record.as_object().unwrap().clone());
            format!("{value}\n")
        })
        .collect()
}
fn fixture(root: &Path) -> Value {
    let directory = transcript(vec![
        json!({"kind":"identity","data":{"id":"google:C123:1001","kind":"unknown","affiliation":"unknown","status":"inactive","verified_emails":["alice@example.test"]}}),
        account(
            "google-main",
            "1001",
            "alice@example.test",
            "unknown",
            "inactive",
            json!(["alice@example.test"]),
        ),
    ]);
    let access = transcript(vec![
        account(
            "github-main",
            "42",
            "alice-dev",
            "human",
            "unknown",
            json!([]),
        ),
        account(
            "github-main",
            "43",
            "automation-bot",
            "bot",
            "unknown",
            json!([]),
        ),
        account(
            "github-main",
            "44",
            "alice@example.test",
            "human",
            "unknown",
            json!([]),
        ),
        json!({"kind":"resource","data":{"key":key("github-main","repository:20"),"name":"acme/application","kind":"github.repository","parent":null}}),
        json!({"kind":"group","data":{"key":key("github-main","team:10"),"name":"maintainers"}}),
        json!({"kind":"membership","data":{"member":{"kind":"account","key":key("github-main","42")},"group":key("github-main","team:10"),"provenance":{"method":"synthetic team membership","observed_at":"2026-09-09T00:00:00Z"}}}),
        grant(
            "github-main",
            "team-access",
            json!({"kind":"group","key":key("github-main","team:10")}),
            "maintain",
            "elevated",
        ),
        grant(
            "github-main",
            "account-access",
            json!({"kind":"account","key":key("github-main","42")}),
            "admin",
            "admin",
        ),
    ]);
    // The real Google/GitHub adapters do not infer service kind. A separate
    // synthetic workload source verifies that the host preserves positive evidence.
    let workload = transcript(vec![account(
        "workloads",
        "svc-1",
        "release-worker",
        "service",
        "unknown",
        json!([]),
    )]);
    let source = format!(
        r##"
use std::io::{{BufRead, Write}};
fn main() {{
 let mut lines=std::io::stdin().lock().lines();
 let handshake=lines.next().unwrap().unwrap();
 assert!(handshake.contains("\"protocol_version\":1"));
 let check=handshake.contains("\"operation\":\"check\"");
 let (provider,records,count)=if handshake.contains("google-main") {{("google-fixture",{directory:?},2)}} else if handshake.contains("github-main") {{("github-fixture",{access:?},8)}} else {{("workload-fixture",{workload:?},1)}};
 println!("{{}}",format!(r#"{{{{"protocol_version":1,"id":"handshake","event":"handshake","provider":"{{}}","capabilities":["accounts","identities","resources","groups","memberships","grants"],"operations":["check","discover"],"draft":true}}}}"#,provider));
 std::io::stdout().flush().unwrap();
 let request=lines.next().unwrap().unwrap();
 assert!(request.contains("\"credentials\":{{}}"));
 if request.contains("\"fail\":true") {{ println!("{{}}",r#"{{"protocol_version":1,"id":"discover","event":"error","code":"unavailable"}}"#); }}
 else if check {{ println!("{{}}",r#"{{"protocol_version":1,"id":"check","event":"health","status":"ok","limitations":[]}}"#); }}
 else {{
  let output=if request.contains("\"renamed\":true") {{records.replace("alice-dev","alice-renamed")}} else {{records.to_string()}};
  print!("{{output}}");
  let partial=request.contains("\"partial\":true");
  println!("{{}}",format!(r#"{{{{"protocol_version":1,"id":"discover","event":"complete","count":{{}},"complete":{{}},"limitations":{{}}}}}}"#,count,!partial,if partial {{r#"["visibility_limited"]"#}} else {{"[]"}}));
 }}
 std::io::stdout().flush().unwrap();
 assert!(lines.next().is_none());
}}
"##
    );
    let file = root.join("peer.rs");
    let binary = root.join(if cfg!(windows) { "peer.exe" } else { "peer" });
    std::fs::write(&file, source).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let digest = permesh_provider_external::trust::inspect(&binary)
        .unwrap()
        .sha256;
    let mut providers = vec![];
    for (instance, provider) in [
        ("google-main", "google-fixture"),
        ("github-main", "github-fixture"),
        ("workloads", "workload-fixture"),
    ] {
        ok(
            root,
            &[
                "provider",
                "external",
                "trust",
                binary.to_str().unwrap(),
                "--id",
                provider,
                "--sha256",
                &digest,
                "--capability",
                "accounts",
                "--capability",
                "identities",
                "--capability",
                "resources",
                "--capability",
                "groups",
                "--capability",
                "memberships",
                "--capability",
                "grants",
                "--accept-risk",
            ],
        );
        providers.push(json!({"id":instance,"type":"external","external":{"provider":provider,"sha256":digest,"discovery_protocol":"negotiated_v1","configuration":{},"credentials":{}}}));
    }
    let config = json!({"version":1,"organization":{"name":"Synthetic cross-provider workflow"},"providers":providers,"identity":{"sources":[{"provider":"google-main","authoritative":true}],"aliases":{"google:C123:1001":{"github-main":["42"]}}}});
    save(root, &config);
    for id in ["google-main", "github-main", "workloads"] {
        approve(root, id);
    }
    config
}
fn query(root: &Path, args: &[&str], exit: i32) -> Value {
    let out = run(root, args);
    assert_eq!(out.status.code(), Some(exit), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn stable_mapping_preserves_access_across_rename_and_distinguishes_principal_evidence() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut config = fixture(&root);
    let first = query(&root, &["user", "google:C123:1001"], 0);
    assert_eq!(first["result"]["identity"]["status"], "inactive");
    assert_eq!(first["result"]["identity"]["kind"], "unknown");
    assert_eq!(first["result"]["access"].as_array().unwrap().len(), 2);
    assert!(
        first["result"]["access"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| !row["memberships"].as_array().unwrap().is_empty())
    );
    config["providers"][1]["external"]["configuration"] = json!({"renamed":true});
    save(&root, &config);
    approve(&root, "github-main");
    let renamed = query(&root, &["user", "google:C123:1001"], 0);
    assert_eq!(first["result"]["access"], renamed["result"]["access"]);
    assert!(
        renamed["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["key"] == key("github-main", "42") && a["login"] == "alice-renamed")
    );
    let review = query(&root, &["orphaned"], 0);
    let rows = review["result"]["accounts"].as_array().unwrap();
    for reason in [
        "inactive_identity",
        "bot",
        "service_account",
        "unknown_identity",
    ] {
        assert!(rows.iter().any(|a| a["reason"] == reason), "{review}");
    }
    let admins = query(&root, &["admins"], 0);
    assert!(!admins["result"]["access"].as_array().unwrap().is_empty());
}
#[test]
fn partial_or_failed_sources_never_erase_valid_access_or_claim_complete_authority() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut config = fixture(&root);
    config["providers"][0]["external"]["configuration"] = json!({"partial":true});
    save(&root, &config);
    approve(&root, "google-main");
    let partial = query(&root, &["user", "google:C123:1001"], 4);
    assert_eq!(partial["complete"], false);
    assert_eq!(partial["result"]["access"].as_array().unwrap().len(), 2);
    let orphaned = query(&root, &["orphaned"], 4);
    assert_eq!(orphaned["result"]["authority_complete"], false);
    assert!(
        orphaned["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["reason"] == "unassessed")
    );
    config["providers"][0]["external"]["configuration"] = json!({});
    config["providers"][2]["external"]["configuration"] = json!({"fail":true});
    save(&root, &config);
    approve(&root, "google-main");
    approve(&root, "workloads");
    let failed = query(&root, &["user", "google:C123:1001"], 4);
    assert_eq!(failed["complete"], false);
    assert_eq!(failed["result"]["access"].as_array().unwrap().len(), 2);
    assert!(
        failed["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "workloads" && p["state"] == "failed")
    );
}
