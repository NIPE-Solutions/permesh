// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{fs, path::Path, process::Command};

const FILES: [&str; 10] = [
    "permesh.yaml",
    "README.md",
    ".gitignore",
    "ci/providers.txt",
    "ci/bootstrap.sh",
    "policies/review-policy.json",
    "profiles/README.md",
    "people/README.md",
    ".github/workflows/permesh-check.yml",
    ".gitlab-ci.yml",
];
fn example(file: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/starter")
            .join(file),
    )
    .unwrap()
}
fn init(dir: &Path, organization: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .args([
            "init",
            "--starter",
            "--organization",
            organization,
            "--json",
        ])
        .output()
        .unwrap()
}
fn yaml(file: &str) -> serde_json::Value {
    permesh_config::parse_setup_value(&String::from_utf8(example(file)).unwrap()).unwrap()
}

#[test]
fn init_starter_writes_layout_that_loads() {
    let dir = tempfile::tempdir().unwrap();
    let output = init(dir.path(), "Acme \"Ops\"");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    for file in FILES {
        assert!(dir.path().join(file).is_file(), "{file}");
    }
    let config = permesh_config::Config::load(&dir.path().join("permesh.yaml")).unwrap();
    assert_eq!(config.organization.name, "Acme \"Ops\"");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(dir.path().join("ci/bootstrap.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o100, 0, "bootstrap.sh is executable");
    }
}

#[test]
fn init_starter_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("ci")).unwrap();
    fs::write(dir.path().join("ci/providers.txt"), "mine\n").unwrap();
    let output = init(dir.path(), "Acme");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(dir.path().join("ci/providers.txt")).unwrap(),
        "mine\n"
    );
    assert!(!dir.path().join("permesh.yaml").exists());
}

#[test]
fn starter_files_match_examples() {
    let dir = tempfile::tempdir().unwrap();
    assert!(init(dir.path(), "Example organization").status.success());
    for file in FILES {
        assert_eq!(
            fs::read(dir.path().join(file)).unwrap(),
            example(file),
            "{file}"
        );
    }
}

#[test]
fn starter_policy_matches_default_rules() {
    let policy: serde_json::Value =
        serde_json::from_slice(&example("policies/review-policy.json")).unwrap();
    assert_eq!(
        policy["rules"],
        serde_json::json!(["inactive_access", "external_privileged", "machine_owner"])
    );
}

#[test]
fn starter_workflows_are_valid_yaml_and_gate_the_baseline() {
    let github = yaml(".github/workflows/permesh-check.yml");
    assert_eq!(
        github["permissions"],
        serde_json::json!({"contents": "read", "issues": "write", "actions": "read"})
    );
    let steps = github["jobs"]["check"]["steps"].as_array().unwrap();
    let baseline = steps
        .iter()
        .find(|s| s["name"] == "Keep the baseline for the next run")
        .unwrap();
    let condition = baseline["if"].as_str().unwrap();
    assert!(
        condition.contains("'0'") && condition.contains("'6'"),
        "{condition}"
    );
    for step in steps.iter().filter(|s| s["uses"].is_string()) {
        let uses = step["uses"].as_str().unwrap();
        let sha = uses.split('@').nth(1).unwrap();
        assert!(
            sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "{uses}"
        );
    }
    let gitlab = yaml(".gitlab-ci.yml");
    let script = gitlab["permesh-baseline"]["script"][0].as_str().unwrap();
    assert!(
        script.contains("\"0\"") && script.contains("\"6\""),
        "{script}"
    );
}

#[test]
fn github_workflow_passes_actionlint_when_available() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/starter/.github/workflows/permesh-check.yml");
    match Command::new("actionlint").arg(&file).output() {
        Ok(output) => assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        ),
        Err(_) => eprintln!("actionlint not installed; skipped"),
    }
    let github = yaml(".github/workflows/permesh-check.yml");
    assert!(!github["env"].to_string().contains("runner."));
}

#[test]
fn github_baseline_comes_only_from_this_workflow_on_the_default_branch() {
    let text = String::from_utf8(example(".github/workflows/permesh-check.yml")).unwrap();
    for needle in [
        "head_branch",
        "head_repository_id",
        ".github/workflows/permesh-check.yml",
        "gh attestation verify",
        "PERMESH_ARCHIVE_SHA256",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
}

#[test]
fn gitlab_findings_keep_baseline_and_issue_jobs_running() {
    let gitlab = yaml(".gitlab-ci.yml");
    assert_eq!(
        gitlab["permesh-check"]["allow_failure"]["exit_codes"],
        serde_json::json!([6])
    );
    assert_eq!(gitlab["permesh-check"]["artifacts"]["access"], "developer");
    assert_eq!(
        gitlab["permesh-baseline"]["artifacts"]["access"],
        "developer"
    );
    assert!(gitlab["permesh-issue"]["when"].is_null());
    assert_eq!(gitlab["permesh-issue"]["rules"][0]["when"], "always");
    let text = String::from_utf8(example(".gitlab-ci.yml")).unwrap();
    assert!(text.contains("PERMESH_ARCHIVE_SHA256"));
}

#[test]
fn starter_docs_require_a_private_repository() {
    let readme = String::from_utf8(example("README.md")).unwrap();
    assert!(readme.contains("private repository"), "{readme}");
}

#[test]
fn starter_ships_report_only_profiles() {
    let dir = tempfile::tempdir().unwrap();
    assert!(init(dir.path(), "Acme").status.success());
    let config = permesh_config::Config::load(&dir.path().join("permesh.yaml")).unwrap();
    let profiles = permesh_config::profiles::load_profiles(dir.path(), &config).unwrap();
    assert!(profiles.is_none_or(|p| !p.enforce));
    let template = String::from_utf8(example("permesh.yaml")).unwrap();
    assert!(template.contains("#   enforce: false"), "{template}");
}
