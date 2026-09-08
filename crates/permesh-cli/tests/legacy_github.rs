// SPDX-License-Identifier: MIT
use std::{fs, process::Command};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn legacy_commands_require_migration_before_credentials_or_network() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let path = root.join("permesh.yaml");
    let config = "version: 1\norganization: {name: Acme}\nproviders:\n- id: legacy\n  type: github\n  organizations: [acme]\n  auth: {token: env://PERMESH_REMOVAL_MISSING_TOKEN}\n";
    fs::write(&path, config)?;
    for (args, code) in [
        (vec!["doctor"], 3),
        (vec!["user", "someone"], 3),
        (vec!["admins"], 3),
        (vec!["provider", "capabilities", "legacy"], 2),
        (vec!["auth", "status", "legacy"], 3),
        (vec!["auth", "login", "legacy", "--token-stdin"], 2),
        (vec!["auth", "logout", "legacy"], 2),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_permesh"))
            .current_dir(&root)
            .env("PERMESH_DATA_DIR", root.join("state"))
            .env_remove("PERMESH_REMOVAL_MISSING_TOKEN")
            .args(args)
            .arg("--json")
            .output()?;
        assert_eq!(result.status.code(), Some(code));
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout)?;
        assert!(text.contains("provider migrate legacy --sha256"), "{text}");
        assert!(!text.contains("Authentication unavailable"));
        assert_eq!(fs::read_to_string(&path)?, config);
        assert!(!root.join("state").exists());
    }
    Ok(())
}
#[test]
fn legacy_add_github_flags_are_inert_and_point_to_guided_setup() -> TestResult {
    let temp = tempfile::tempdir()?;
    let result = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(temp.path())
        .args([
            "provider",
            "add",
            "github",
            "--organization",
            "acme",
            "--json",
        ])
        .output()?;
    assert_eq!(result.status.code(), Some(2));
    let text = String::from_utf8(result.stdout)?;
    for word in ["install", "trust", "setup"] {
        assert!(text.contains(word), "{text}");
    }
    assert_eq!(fs::read_dir(temp.path())?.count(), 0);
    Ok(())
}
