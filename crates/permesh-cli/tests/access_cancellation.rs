// SPDX-License-Identifier: MIT
#![cfg(unix)]
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_permesh"));
    command
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .env("NO_COLOR", "1");
    command
}
fn run(root: &Path, args: &[&str]) -> Value {
    let output = command(root).args(args).arg("--json").output().unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A real native provider speaks either legacy protocol, exposes a private
/// observation, then hangs after marking its PID. This synchronizes SIGINT with
/// active discovery rather than relying on startup timing.
fn fixture(root: &Path) -> String {
    let source = root.join("peer.rs");
    let binary = root.join("peer");
    let code = r##"
use std::io::{BufRead, Write};
fn main() {
    let mut lines = std::io::stdin().lock().lines();
    let handshake = lines.next().unwrap().unwrap();
    let version = if handshake.contains("\"protocol\":2") { 2 } else { 1 };
    println!("{}", r#"{"protocol":VERSION,"id":"handshake","event":"handshake","provider":"fixture","capabilities":["accounts"],"draft":true}"#.replace("VERSION", &version.to_string()));
    std::io::stdout().flush().unwrap();
    let discover = lines.next().unwrap().unwrap();
    assert!(discover.contains("\"method\":\"discover\""));
    println!("{}", r#"{"protocol":VERSION,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"instance","id":"guest"},"login":"CANCELLATION_PRIVATE_OBSERVATION","kind":"human","verified_emails":[]}}"#.replace("VERSION", &version.to_string()));
    std::io::stdout().flush().unwrap();
    eprintln!("CANCELLATION_PRIVATE_STDERR");
    let marker = std::path::Path::new(MARKER);
    let temporary = marker.with_extension("tmp");
    std::fs::write(&temporary, std::process::id().to_string()).unwrap();
    std::fs::rename(temporary, marker).unwrap();
    // Deliberately ignore cancellation; the host must terminate and reap us.
    loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
}
"##
    .replace("MARKER", &format!("{:?}", root.join("ready.pid")));
    std::fs::write(&source, code).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let inspect = run(
        root,
        &["provider", "external", "inspect", binary.to_str().unwrap()],
    );
    let digest = inspect["result"]["inspection"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
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
            "--accept-risk",
        ],
    );
    digest
}

fn cancel_discovery(root: &Path, args: &[&str]) -> Output {
    let mut child = command(root)
        .args(args)
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let marker = root.join("ready.pid");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!(
                "Discovery exited before ready: {:?}",
                child.wait_with_output().unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !marker.exists() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("Native discovery did not reach the cancellation marker");
    }
    let provider_pid: u32 = std::fs::read_to_string(marker).unwrap().parse().unwrap();
    assert!(provider_pid > 0, "Provider PID must be positive");
    let provider_pid = provider_pid.to_string();
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        let _ = Command::new("kill").args(["-KILL", &provider_pid]).status();
        panic!("CLI did not drain cancelled provider discovery");
    }
    let output = child.wait_with_output().unwrap();
    let alive = Command::new("kill")
        .args(["-0", &provider_pid])
        .output()
        .unwrap()
        .status
        .success();
    if alive {
        let _ = Command::new("kill").args(["-KILL", &provider_pid]).status();
    }
    assert!(
        !alive,
        "Cancelled provider child was not terminated and reaped"
    );
    output
}

fn assert_cancelled(output: Output) {
    assert_eq!(output.status.code(), Some(130), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value,
        json!({"schema_version":2,"error":{"code":130,"message":"Cancelled"}})
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("CANCELLATION_PRIVATE"));
}

#[test]
fn user_collection_sigint_reports_schema_two_and_reaps_provider() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let digest = fixture(&root);
    let config = json!({"version":1,"organization":{"name":"Cancellation fixture"},
        "providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":digest,"configuration":{},"credentials":{}}}]});
    std::fs::write(
        root.join("permesh.yaml"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let review = run(&root, &["provider", "external", "review", "instance"]);
    let fingerprint = review["result"]["fingerprint"].as_str().unwrap();
    run(
        &root,
        &[
            "provider",
            "external",
            "approve",
            "instance",
            "--fingerprint",
            fingerprint,
            "--accept-risk",
        ],
    );
    assert_cancelled(cancel_discovery(&root, &["user", "missing"]));
}

#[test]
fn standalone_external_sigint_reports_schema_two_and_reaps_provider() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    fixture(&root);
    assert_cancelled(cancel_discovery(
        &root,
        &[
            "provider",
            "external",
            "discover",
            "fixture",
            "--instance",
            "instance",
        ],
    ));
}
