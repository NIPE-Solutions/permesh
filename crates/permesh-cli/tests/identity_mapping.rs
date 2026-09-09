// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(path)
        .args(args)
        .output()
        .unwrap()
}
fn result(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}
#[test]
fn fresh_cli_mapping_proposal_apply_inspect_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let before = fs::read(&path).unwrap();
    let unresolved = result(run(dir.path(), &["identity", "unresolved", "--json"]));
    assert_eq!(unresolved["identity_review_version"], 1);
    let inspected = result(run(
        dir.path(),
        &["identity", "inspect", "demo", "101", "--json"],
    ));
    assert_eq!(inspected["accounts"][0]["account_id"], "101");
    let target = inspected["accounts"][0]["resolution"]["canonical_identity_id"]
        .as_str()
        .unwrap();
    let proposal = result(run(
        dir.path(),
        &[
            "identity",
            "map",
            "demo",
            "101",
            "--identity",
            target,
            "--json",
        ],
    ));
    assert_eq!(proposal["applied"], false);
    assert_eq!(fs::read(&path).unwrap(), before);
    let fp = proposal["fingerprint"].as_str().unwrap();
    let applied = result(run(
        dir.path(),
        &[
            "identity",
            "map",
            "demo",
            "101",
            "--identity",
            target,
            "--fingerprint",
            fp,
            "--json",
        ],
    ));
    assert_eq!(applied["applied"], true);
    let mapped = result(run(
        dir.path(),
        &["identity", "inspect", "demo", "101", "--json"],
    ));
    assert_eq!(
        mapped["explicit_mappings"]["canonical_identity_ids"],
        serde_json::json!([target])
    );
    let removal = result(run(
        dir.path(),
        &[
            "identity",
            "unmap",
            "demo",
            "101",
            "--identity",
            target,
            "--json",
        ],
    ));
    let fp = removal["fingerprint"].as_str().unwrap();
    result(run(
        dir.path(),
        &[
            "identity",
            "unmap",
            "demo",
            "101",
            "--identity",
            target,
            "--fingerprint",
            fp,
            "--json",
        ],
    ));
    assert!(
        permesh_config::Config::load(&path)
            .unwrap()
            .identity
            .aliases
            .is_empty()
    );
}
