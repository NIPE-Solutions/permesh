// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    time::Duration,
};
fn command(root: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_permesh"));
    c.current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2");
    c
}
fn run(root: &Path, args: &[&str]) -> Output {
    command(root).args(args).arg("--json").output().unwrap()
}
#[test]
fn browser_flags_are_explicit_and_do_not_create_credentials() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["auth", "login", "instance", "--no-open"],
        vec!["auth", "login", "instance", "--browser", "--token-stdin"],
        vec![
            "auth",
            "login",
            "instance",
            "--browser",
            "--credential",
            "token",
        ],
    ] {
        let out = run(root.path(), &args);
        assert_eq!(out.status.code(), Some(2));
    }
    assert!(!root.path().join("state").exists());
}
#[test]
fn approved_native_descriptor_drives_loopback_denial_without_credential_writes() {
    let root = tempfile::tempdir().unwrap();
    let canonical = root.path().canonicalize().unwrap();
    let root = canonical.as_path();
    let marker = root.join("executed");
    let source = root.join("fixture.rs");
    let binary = root.join(if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    });
    let code=r##"use std::io::{BufRead,Write};fn main(){std::fs::write(MARKER,"executed").unwrap();let mut lines=std::io::stdin().lock().lines();let first=lines.next().unwrap().unwrap();assert!(first.contains("\"protocol\":4"));println!("{}",r#"{"protocol":4,"id":"handshake","event":"handshake","provider":"fixture","capabilities":[],"draft":true}"#);std::io::stdout().flush().unwrap();assert!(lines.next().unwrap().unwrap().contains("describe_auth"));println!("{}",r#"{"protocol":4,"id":"describe_auth","event":"auth","spec":{"schema_version":1,"authorization_endpoint":"https://id.example.test/authorize","token_endpoint":"https://id.example.test/token","scopes":["read"],"client_id_field":"client_id","refresh_token_slot":"refresh_token","authorization_parameters":{}}}"#);std::io::stdout().flush().unwrap();assert!(lines.next().is_none());}"##.replace("MARKER",&format!("{marker:?}"));
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
    let digest = permesh_provider_external::trust::inspect(&binary)
        .unwrap()
        .sha256;
    permesh_provider_external::trust::Registry::new(root.join("state/providers"))
        .unwrap()
        .trust(&binary, "fixture", &digest, &[])
        .unwrap();
    assert!(run(root, &["init"]).status.success());
    assert!(
        run(
            root,
            &[
                "provider",
                "add",
                "external",
                "--id",
                "instance",
                "--provider",
                "fixture",
                "--sha256",
                &digest,
                "--setting",
                "client_id=client",
                "--credential",
                "refresh_token=keychain://instance/refresh_token"
            ]
        )
        .status
        .success()
    );
    let before = std::fs::read(root.join("permesh.yaml")).unwrap();
    let out = run(
        root,
        &["auth", "login", "instance", "--browser", "--no-open"],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(!marker.exists());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("https://"));
    let review = run(root, &["provider", "external", "review", "instance"]);
    let value: serde_json::Value = serde_json::from_slice(&review.stdout).unwrap();
    let fingerprint = value["result"]["fingerprint"].as_str().unwrap();
    let out = run(
        root,
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
    assert!(out.status.success(), "{out:?}");
    let mut child = command(root)
        .args([
            "auth",
            "login",
            "instance",
            "--browser",
            "--no-open",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let line = line.unwrap();
            if line.starts_with("https://") {
                tx.send(line).unwrap();
                break;
            }
        }
    });
    let authorization = match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(url) => url,
        Err(error) => {
            child.kill().unwrap();
            let out = child.wait_with_output().unwrap();
            panic!("URL unavailable: {error}; {out:?}")
        }
    };
    reader.join().unwrap();
    assert!(marker.exists());
    let url = url::Url::parse(&authorization).unwrap();
    let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(pairs["code_challenge_method"], "S256");
    assert_eq!(pairs["scope"], "read");
    let redirect = url::Url::parse(&pairs["redirect_uri"]).unwrap();
    let port = redirect.port().unwrap();
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(socket,"GET /oauth/callback?state={}&error=access_denied&error_description=NEVER_ECHO_SECRET HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n",pairs["state"]).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(!response.contains("NEVER_ECHO_SECRET"));
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("denied"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("NEVER_ECHO_SECRET"));
    assert_eq!(before, std::fs::read(root.join("permesh.yaml")).unwrap());
    #[cfg(unix)]
    {
        let mut child = command(root)
            .args([
                "auth",
                "login",
                "instance",
                "--browser",
                "--no-open",
                "--json",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let line = line.unwrap();
                if line.starts_with("https://") {
                    tx.send(line).unwrap();
                    break;
                }
            }
        });
        let url = rx.recv_timeout(Duration::from_secs(15)).unwrap();
        reader.join().unwrap();
        assert!(
            Command::new("/bin/kill")
                .args(["-INT", &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("cancelled CLI did not exit");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(130), "{out:?}");
        let url = url::Url::parse(&url).unwrap();
        let redirect = url
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .unwrap()
            .1
            .into_owned();
        let redirect = url::Url::parse(&redirect).unwrap();
        assert!(std::net::TcpStream::connect(("127.0.0.1", redirect.port().unwrap())).is_err());
        assert_eq!(before, std::fs::read(root.join("permesh.yaml")).unwrap());
    }
}
