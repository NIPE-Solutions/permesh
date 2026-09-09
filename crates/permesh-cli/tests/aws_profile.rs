// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    fs,
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
fn report(root: &Path, args: &[&str], code: i32) -> Value {
    let out = run(root, args);
    assert_eq!(out.status.code(), Some(code), "{out:?}");
    for secret in [
        "ASIAEXAMPLE1234567890",
        "synthetic-secret-value",
        "synthetic-session-one",
        "synthetic-session-two",
    ] {
        assert!(!String::from_utf8_lossy(&out.stdout).contains(secret));
        assert!(!String::from_utf8_lossy(&out.stderr).contains(secret));
    }
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn approved_profile_delivers_one_session_without_exposing_source_or_loading_fallbacks() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let source = root.join("peer.rs");
    let binary = root.join(if cfg!(windows) { "peer.exe" } else { "peer" });
    fs::write(&source,r##"
use std::io::{BufRead,Write};
fn main(){
 let mut input=std::io::stdin().lock().lines(); let handshake=input.next().unwrap().unwrap();
 assert!(handshake.contains("\"operation\":\"check\""));
 println!("{}",r#"{"protocol_version":1,"id":"handshake","event":"handshake","provider":"aws-fixture","capabilities":["accounts"],"operations":["check","discover"],"draft":true}"#);std::io::stdout().flush().unwrap();
 let request=input.next().unwrap().unwrap();
 assert!(request.contains("\"access_key_id\":\"ASIAEXAMPLE1234567890\""));
 assert!(request.contains("\"secret_access_key\":\"synthetic-secret-value\""));
 let generation=std::fs::read_to_string(EXPECTATION_PATH).unwrap();
 assert!(request.contains(&format!("\"session_token\":\"synthetic-session-{}\"",generation)));
 assert!(!request.contains("credentials_file"));assert!(!request.contains("aws_profile"));assert!(!request.contains("UNSELECTED"));
 assert!(std::env::var("AWS_PROFILE").is_err());assert!(std::env::var("AWS_SECRET_ACCESS_KEY").is_err());
 println!("{}",r#"{"protocol_version":1,"id":"check","event":"health","status":"ok","limitations":[]}"#);std::io::stdout().flush().unwrap();
 assert!(input.next().is_none());
}
"##.replace("EXPECTATION_PATH", &format!("{:?}",root.join("expected-generation")))).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let digest = permesh_provider_external::trust::inspect(&binary)
        .unwrap()
        .sha256;
    report(
        &root,
        &[
            "provider",
            "external",
            "trust",
            binary.to_str().unwrap(),
            "--id",
            "aws-fixture",
            "--sha256",
            &digest,
            "--capability",
            "accounts",
            "--accept-risk",
        ],
        0,
    );
    let credentials = root.join("credentials");
    let mut config = json!({"version":1,"organization":{"name":"Synthetic AWS profile"},"providers":[{"id":"aws-main","type":"external","external":{"provider":"aws-fixture","sha256":digest,"discovery_protocol":"negotiated_v1","configuration":{"account_id":"123456789012","region":"eu-west-1","caller_role":"Reader"},"aws_profile":{"version":1,"credentials_file":credentials,"profile":"review"}}}]});
    fs::write(root.join("permesh.yaml"), config.to_string()).unwrap();
    assert_eq!(
        report(&root, &["auth", "status"], 0)["providers"][0]["state"],
        "configured"
    );
    report(&root, &["auth", "login", "aws-main", "--token-stdin"], 2);
    report(&root, &["auth", "logout", "aws-main"], 2);
    let blocked = report(&root, &["doctor", "--details"], 3);
    assert!(blocked.to_string().contains("approval"));
    let review = report(&root, &["provider", "external", "review", "aws-main"], 0);
    assert_eq!(review["result"]["aws_profile"]["profile"], "review");
    report(
        &root,
        &[
            "provider",
            "external",
            "approve",
            "aws-main",
            "--fingerprint",
            review["result"]["fingerprint"].as_str().unwrap(),
            "--accept-risk",
        ],
        0,
    );
    let unavailable = report(&root, &["doctor", "--details"], 3);
    assert_eq!(
        unavailable["result"]["diagnostics"][0]["stage"],
        "credentials"
    );
    assert_eq!(
        unavailable["result"]["diagnostics"][0]["code"],
        "credential_unavailable"
    );
    for token in ["synthetic-session-one", "synthetic-session-two"] {
        fs::write(
            root.join("expected-generation"),
            token.strip_prefix("synthetic-session-").unwrap(),
        )
        .unwrap();
        fs::write(&credentials,format!("[unselected]\ncredential_process = UNSELECTED\n[review]\naws_access_key_id = ASIAEXAMPLE1234567890\naws_secret_access_key = synthetic-secret-value\naws_session_token = {token}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&credentials, fs::Permissions::from_mode(0o600)).unwrap();
        }
        report(&root, &["doctor"], 0);
    }
    config["providers"][0]["external"]["configuration"]["caller_role"] = json!("Changed");
    fs::write(root.join("permesh.yaml"), config.to_string()).unwrap();
    fs::remove_file(credentials).unwrap();
    let blocked = report(&root, &["doctor", "--details"], 3);
    assert!(blocked.to_string().contains("approval"));
}
