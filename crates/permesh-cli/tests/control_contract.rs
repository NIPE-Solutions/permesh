// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
#[path = "../src/schema1_control.rs"]
#[allow(dead_code)]
mod schema1_control;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn normalize(value: &mut Value, root: &str) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                match key.as_str() {
                    "started_at" | "completed_at" => {
                        time::OffsetDateTime::parse(
                            value.as_str().unwrap(),
                            &time::format_description::well_known::Rfc3339,
                        )
                        .unwrap();
                        *value = json!("<TIME>");
                    }
                    "fingerprint" => {
                        let digest = value.as_str().unwrap();
                        assert_eq!(digest.len(), 64);
                        assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
                        *value = json!("<FINGERPRINT>");
                    }
                    _ => normalize(value, root),
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(|value| normalize(value, root)),
        Value::String(value) if value.starts_with(root) => {
            *value = value.replacen(root, "<ROOT>", 1).replace('\\', "/");
        }
        _ => (),
    }
}
fn run(root: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .args(args)
        .arg("--json")
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&0x1b));
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn complete_control_reports_preserve_baseline_schema_one() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/control/reports.json")).unwrap();
    let check = |name: &str, mut report: Value| {
        normalize(&mut report, root.to_str().unwrap());
        assert_eq!(report, expected[name], "report: {name}");
    };
    run(&root, &["init", "--demo"]);
    check(
        "metadata",
        run(&root, &["provider", "capabilities", "demo"]),
    );
    check("doctor", run(&root, &["doctor"]));
    check("status", run(&root, &["provider", "status", "demo"]));
    let native = root.join("native");
    std::fs::write(&native, b"\x7fELFpermesh control fixture").unwrap();
    let inspection = run(
        &root,
        &["provider", "external", "inspect", native.to_str().unwrap()],
    );
    let digest = inspection["result"]["inspection"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    check("inspect", inspection);
    check("empty_list", run(&root, &["provider", "external", "list"]));
    let mut args = vec![
        "provider",
        "external",
        "trust",
        native.to_str().unwrap(),
        "--id",
        "fixture",
        "--sha256",
        &digest,
        "--accept-risk",
    ];
    for capability in [
        "accounts",
        "identities",
        "resources",
        "groups",
        "memberships",
        "grants",
    ] {
        args.extend(["--capability", capability]);
    }
    check("trust", run(&root, &args));
    check("list", run(&root, &["provider", "external", "list"]));
    let config = json!({"version":1,"organization":{"name":"Control fixture"},
        "providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":digest,
            "configuration":{"enabled":true,"regions":["eu"],"optional":null},"credentials":{"token":"env://PERMESH_CONTROL_ABSENT"}}}],
        "identity":{"sources":[{"provider":"instance","authoritative":true}],"aliases":{"person@example.test":{"instance":["native-id"]}}}});
    std::fs::write(
        root.join("permesh.yaml"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    check(
        "external_metadata",
        run(&root, &["provider", "capabilities", "instance"]),
    );
    let review = run(&root, &["provider", "external", "review", "instance"]);
    let fingerprint = review["result"]["fingerprint"].as_str().unwrap().to_owned();
    check("review", review);
    check(
        "approve",
        run(
            &root,
            &[
                "provider",
                "external",
                "approve",
                "instance",
                "--fingerprint",
                &fingerprint,
                "--accept-risk",
            ],
        ),
    );
    check(
        "approved_review",
        run(&root, &["provider", "external", "review", "instance"]),
    );
    check(
        "revoke",
        run(&root, &["provider", "external", "revoke", "instance"]),
    );
    check(
        "remove",
        run(&root, &["provider", "external", "remove", "fixture"]),
    );
}

#[test]
fn release_and_package_preserve_complete_baseline_objects_and_null() {
    use permesh_provider_external::{catalog::Release, packages::InstalledPackage};
    use permesh_provider_sdk::Capability;
    let release = Release {
        discovery_protocol: Default::default(),
        provider: "fixture".into(),
        version: "1.2.3".parse().unwrap(),
        target: "x86_64-unknown-linux-gnu".into(),
        capabilities: vec![
            Capability::Accounts,
            Capability::Identities,
            Capability::Resources,
            Capability::Groups,
            Capability::Memberships,
            Capability::Grants,
        ],
        protocols: vec![1, 2, 3, 4],
        archive_sha256: "a".repeat(64),
        executable_sha256: "b".repeat(64),
        archive_size: 12345,
    };
    let package = InstalledPackage {
        release: release.clone(),
        executable: "provider.exe".into(),
    };
    let actual = json!({"selected":schema1_control::Release::from(&release),"package":schema1_control::InstalledPackage::from(&package),"absent_package":Option::<schema1_control::InstalledPackage>::None});
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/control/distribution.json")).unwrap();
    assert_eq!(actual, expected);
}
