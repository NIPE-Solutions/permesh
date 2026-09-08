// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("TOKIO_WORKER_THREADS", "2")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn scripts_are_deterministic_and_ignore_workspace_and_config_arguments() {
    let dir = tempfile::tempdir().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let clean = run(dir.path(), &["completion", shell]);
        assert!(
            clean.status.success(),
            "{}",
            String::from_utf8_lossy(&clean.stderr)
        );
        assert!(clean.stderr.is_empty());
        assert!(!clean.stdout.contains(&0x1b));
        let script = String::from_utf8(clean.stdout.clone()).unwrap();
        for name in ["permesh", "orphaned", "admins", "provider", "customer-id"] {
            assert!(script.contains(name), "{shell} missing {name}");
        }
        let path = dir.path().join("permesh.yaml");
        let hostile =
            "version: 999\nplugins: ['./do-not-run']\nprivate: SENTINEL_COMPLETION_PRIVATE\n";
        std::fs::write(&path, hostile).unwrap();
        let implicit = run(dir.path(), &["completion", shell]);
        assert!(implicit.status.success());
        assert_eq!(implicit.stdout, clean.stdout);
        let output = run(
            dir.path(),
            &[
                "--config",
                "nonexistent.yaml",
                "--color",
                "always",
                "-vv",
                "completion",
                shell,
            ],
        );
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, clean.stdout);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), hostile);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn invalid_completion_arguments_preserve_json_error_contract() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["completion", "bash", "--json"],
        vec!["--json", "completion", "bash"],
        vec!["completion", "SENTINEL_PRIVATE_SHELL", "--json"],
        vec!["completion", "--json"],
    ] {
        let output = run(dir.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["schema_version"], 1);
        assert!(error["error"].is_object());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SENTINEL_PRIVATE_SHELL"));
    }
}

#[cfg(unix)]
#[test]
fn bash_script_completes_nested_commands() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["completion", "bash"]);
    assert!(output.status.success());
    let path = dir.path().join("completion.bash");
    std::fs::write(&path, output.stdout).unwrap();
    let result = Command::new("bash").args(["--noprofile", "--norc", "-c", r#"source "$1"; COMP_WORDS=(permesh provider ""); COMP_CWORD=2; _permesh permesh "" provider; printf '%s\n' "${COMPREPLY[@]}""#, "completion-test"]).arg(path).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let values = String::from_utf8(result.stdout).unwrap();
    for command in ["add", "list", "status", "capabilities"] {
        assert!(
            values.lines().any(|value| value == command),
            "missing {command}: {values}"
        );
    }
}

#[cfg(windows)]
#[test]
fn powershell_script_registers_a_working_completer() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["completion", "powershell"]);
    assert!(output.status.success());
    let path = dir.path().join("completion.ps1");
    std::fs::write(&path, output.stdout).unwrap();
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_permesh")).parent().unwrap();
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let search_path = std::env::join_paths(
        std::iter::once(binary_dir.to_path_buf()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let result = Command::new("powershell.exe").env("PATH", search_path).args(["-NoProfile", "-NonInteractive", "-Command", r#"$ErrorActionPreference = 'Stop'; . $env:PERMESH_TEST_COMPLETION_SCRIPT; $line = 'permesh provider '; [System.Management.Automation.CommandCompletion]::CompleteInput($line, $line.Length, $null).CompletionMatches | ForEach-Object { $_.CompletionText }"#]).env("PERMESH_TEST_COMPLETION_SCRIPT", path).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let values = String::from_utf8(result.stdout).unwrap();
    for command in ["add", "list", "status", "capabilities"] {
        assert!(
            values.lines().any(|value| value == command),
            "missing {command}: {values}"
        );
    }
}

#[cfg(unix)]
#[test]
fn closed_completion_pipe_exits_without_panicking() {
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .args(["completion", "bash"])
        .env("TOKIO_WORKER_THREADS", "2")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "macos")]
#[test]
fn zsh_script_registers_the_completion_function() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["completion", "zsh"]);
    assert!(output.status.success());
    let path = dir.path().join("completion.zsh");
    std::fs::write(&path, output.stdout).unwrap();
    let result = Command::new("zsh").args(["-f", "-c", r#"autoload -Uz compinit; compinit -D; source "$1"; (( $+functions[_permesh] )) && [[ ${_comps[permesh]} == _permesh ]]"#, "completion-test"]).arg(path).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
