// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use super::*;
use std::path::PathBuf;
fn executable() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!(
            "permesh-test-external-peer{}",
            std::env::consts::EXE_SUFFIX
        ))
}
fn deadlines() -> Deadlines {
    Deadlines {
        handshake: Duration::from_secs(5),
        operation: Duration::from_secs(10),
        cancel: Duration::from_millis(200),
        cleanup: Duration::from_secs(2),
    }
}
async fn run(mode: &str) -> Result<Snapshot, ExternalError> {
    discover_with_deadlines(
        &executable(),
        "fixture",
        mode,
        &[],
        std::future::pending(),
        deadlines(),
    )
    .await
}
fn isolated_peer() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let peer = dir
        .path()
        .join(format!("peer{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(executable(), &peer).unwrap();
    (dir, peer)
}
#[tokio::test]
async fn rejects_peer_errors_without_echoing_data() {
    for mode in [
        "wrong-provider",
        "wrong-caps",
        "malformed",
        "truncated",
        "incomplete",
        "extra",
    ] {
        let error = run(mode).await.unwrap_err();
        assert!(
            matches!(error, ExternalError::Protocol),
            "{mode}: {error:?}"
        );
        assert!(!format!("{error:?} {error}").contains("SENTINEL"));
    }
    assert!(matches!(run("nonzero").await, Err(ExternalError::Exit)));
}
#[tokio::test]
async fn enforces_stream_limits() {
    assert!(matches!(
        run("stdout-flood").await,
        Err(ExternalError::Protocol)
    ));
    assert!(matches!(
        run("stderr-flood").await,
        Err(ExternalError::StderrLimit)
    ));
}
#[tokio::test]
async fn deadlines_do_not_reset_with_output_or_completion() {
    for mode in [
        "handshake-hang",
        "handshake-trickle",
        "operation-hang",
        "complete-hang",
    ] {
        let started = Instant::now();
        let limits = Deadlines {
            handshake: Duration::from_millis(500),
            operation: Duration::from_millis(1200),
            ..deadlines()
        };
        assert!(
            matches!(
                discover_with_deadlines(
                    &executable(),
                    "fixture",
                    mode,
                    &[],
                    std::future::pending(),
                    limits
                )
                .await,
                Err(ExternalError::Timeout)
            ),
            "{mode}"
        );
        let elapsed = started.elapsed();
        if mode.starts_with("handshake") {
            assert!(elapsed < Duration::from_millis(1100));
        } else {
            assert!(
                elapsed >= Duration::from_millis(1100),
                "handshake timer must stop after negotiation"
            );
        }
        assert!(elapsed < Duration::from_secs(4));
    }
}
#[tokio::test]
async fn cooperative_cancellation_is_sent_and_awaited() {
    let (dir, peer) = isolated_peer();
    let result = discover_with_deadlines(
        &peer,
        "fixture",
        "cancel",
        &[],
        wait_for_marker(dir.path().join("ready")),
        deadlines(),
    )
    .await;
    assert!(matches!(result, Err(ExternalError::Cancelled)));
    assert!(dir.path().join("cancelled").exists());
}
#[tokio::test]
async fn clears_environment_arguments_and_removes_private_working_directory() {
    let (dir, peer) = isolated_peer();
    discover_with_deadlines(
        &peer,
        "fixture",
        "environment",
        &[],
        std::future::pending(),
        deadlines(),
    )
    .await
    .unwrap();
    let cwd = std::fs::read_to_string(dir.path().join("working-directory")).unwrap();
    assert!(!Path::new(&cwd).exists());
}
#[tokio::test]
async fn remaining_descendants_stop_on_success_failure_timeout_and_cancellation() {
    for mode in [
        "descendant",
        "descendant-detached-pipes",
        "descendant-malformed",
        "descendant-nonzero",
        "descendant-hang",
    ] {
        let (dir, peer) = isolated_peer();
        let result = discover_with_deadlines(
            &peer,
            "fixture",
            mode,
            &[],
            std::future::pending(),
            deadlines(),
        )
        .await;
        if mode == "descendant-malformed" {
            assert!(matches!(result, Err(ExternalError::Protocol)));
        } else if mode == "descendant-nonzero" {
            assert!(matches!(result, Err(ExternalError::Exit)));
        } else if mode == "descendant-hang" {
            assert!(matches!(result, Err(ExternalError::Timeout)));
        } else {
            result.unwrap();
        }
        assert_stopped(dir.path()).await;
    }
    let (dir, peer) = isolated_peer();
    let result = discover_with_deadlines(
        &peer,
        "fixture",
        "descendant-hang",
        &[],
        wait_for_marker(dir.path().join("heartbeat")),
        deadlines(),
    )
    .await;
    assert!(matches!(result, Err(ExternalError::Cancelled)));
    assert_stopped(dir.path()).await;
}
async fn assert_stopped(directory: &Path) {
    tokio::time::sleep(Duration::from_millis(80)).await;
    let first = std::fs::read(directory.join("heartbeat")).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(first, std::fs::read(directory.join("heartbeat")).unwrap());
}

async fn wait_for_marker(path: PathBuf) {
    let until = Instant::now() + Duration::from_secs(8);
    while !path.exists() && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn rejects_cross_instance_domain_keys_and_accepts_exact_stderr_budget() {
    assert!(matches!(
        discover_with_deadlines(
            &executable(),
            "fixture",
            "wrong-instance",
            &[Capability::Accounts],
            std::future::pending(),
            deadlines()
        )
        .await,
        Err(ExternalError::Protocol)
    ));
    run("stderr-limit").await.unwrap();
}

#[tokio::test]
async fn retains_explicit_partial_discovery() {
    let snapshot = run("partial").await.unwrap();
    assert!(!snapshot.complete);
    assert_eq!(snapshot.limitations.len(), 1);
}
