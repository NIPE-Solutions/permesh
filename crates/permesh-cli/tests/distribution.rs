// SPDX-License-Identifier: MIT OR Apache-2.0
use std::process::Command;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn install_and_update_are_discoverable() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .args(["provider", "--help"])
        .output()?;
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout)?;
    assert!(help.contains("install"));
    assert!(help.contains("update"));
    Ok(())
}
#[test]
fn invalid_distribution_requests_and_missing_installations_are_inert() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    std::fs::write(root.join("permesh.yaml"), "hostile: invalid workspace")?;
    for args in [
        vec!["install", "github"],
        vec!["install", "../escape", "--version", "1.0.0"],
        vec!["install", "github", "--version", "SENTINEL_PRIVATE"],
        vec!["install", "github", "--version", "1.0.0-beta.1"],
        vec!["update", "github", "--check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
            .current_dir(&root)
            .env("PERMESH_DATA_DIR", root.join("state"))
            .env("TOKIO_WORKER_THREADS", "2")
            .arg("provider")
            .args(args)
            .arg("--json")
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(json["schema_version"], 1);
        assert!(!String::from_utf8(output.stdout)?.contains("SENTINEL_PRIVATE"));
        assert!(!root.join("state").exists());
    }
    Ok(())
}
