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
    let source = executable();
    // Keep links on the same filesystem. Concurrent executable copies can leave
    // writable descriptors briefly inherited by another spawn before exec closes
    // CLOEXEC descriptors; Linux then rejects execution with ETXTBSY.
    #[cfg(target_os = "linux")]
    let dir = tempfile::tempdir_in(source.parent().unwrap()).unwrap();
    #[cfg(not(target_os = "linux"))]
    let dir = tempfile::tempdir().unwrap();
    let peer = dir
        .path()
        .join(format!("peer{}", std::env::consts::EXE_SUFFIX));
    #[cfg(target_os = "linux")]
    std::fs::hard_link(source, &peer).unwrap();
    #[cfg(not(target_os = "linux"))]
    std::fs::copy(source, &peer).unwrap();
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
    assert!(
        matches!(result, Err(ExternalError::Cancelled)),
        "{result:?}"
    );
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
            assert!(result.is_ok(), "{mode}: {result:?}");
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
    assert!(
        matches!(result, Err(ExternalError::Cancelled)),
        "{result:?}"
    );
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

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn isolated_fixtures_can_spawn_concurrently() {
    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..8 {
        workers.spawn(async {
            for _ in 0..20 {
                let (_directory, peer) = isolated_peer();
                discover_with_deadlines(
                    &peer,
                    "fixture",
                    "normal",
                    &[],
                    std::future::pending(),
                    deadlines(),
                )
                .await
                .unwrap();
            }
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
}

fn all_capabilities() -> [Capability; 6] {
    [
        Capability::Accounts,
        Capability::Identities,
        Capability::Resources,
        Capability::Groups,
        Capability::Memberships,
        Capability::Grants,
    ]
}
fn configured(mode: &str) -> Invocation {
    use std::collections::BTreeMap;
    Invocation::new(
        BTreeMap::from([("mode".into(), serde_json::json!(mode))]),
        BTreeMap::from([(
            "token".into(),
            permesh_secrets::Secret::new("synthetic-fixture-token".into()),
        )]),
    )
    .unwrap()
}
#[tokio::test]
async fn configured_discovery_and_health_deliver_private_named_credentials() {
    let invocation = configured("normal");
    let snapshot = discover_configured(
        &executable(),
        "fixture",
        "first-main",
        &all_capabilities(),
        &invocation,
        std::future::pending(),
    )
    .await
    .unwrap();
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(snapshot.grants.len(), 2);
    assert_eq!(snapshot.accounts[0].key.provider, "first-main");
    let health = check_configured(
        &executable(),
        "fixture",
        "first-main",
        &all_capabilities(),
        &invocation,
        std::future::pending(),
    )
    .await
    .unwrap();
    assert!(health.limitations.is_empty());
    assert!(!format!("{snapshot:?} {health:?} {invocation:?}").contains("synthetic-fixture-token"));
}

#[tokio::test]
async fn configured_responses_reject_plain_and_escaped_secret_reflection() {
    for mode in ["echo", "echo-escaped", "echo-key"] {
        let error = discover_configured(
            &executable(),
            "fixture",
            "safe-main",
            &all_capabilities(),
            &configured(mode),
            std::future::pending(),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, ExternalError::Protocol),
            "{mode}: {error:?}"
        );
        assert!(!format!("{error:?} {error}").contains("synthetic-fixture-token"));
    }
    let snapshot = discover_configured(
        &executable(),
        "fixture",
        "safe-main",
        &all_capabilities(),
        &configured("echo-stderr"),
        std::future::pending(),
    )
    .await
    .unwrap();
    assert!(!format!("{snapshot:?}").contains("synthetic-fixture-token"));
}
#[tokio::test]
async fn configured_handshake_rejection_never_delivers_an_operation() {
    for mode in ["wrong-provider", "wrong-caps", "downgrade"] {
        let (directory, peer) = isolated_peer();
        let error = discover_configured(
            &peer,
            "fixture",
            mode,
            &all_capabilities(),
            &configured("normal"),
            std::future::pending(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ExternalError::Protocol));
        assert!(!directory.path().join("delivered").exists());
    }
}
#[tokio::test]
async fn configured_partial_health_errors_and_instance_credentials_are_isolated() {
    use std::collections::BTreeMap;
    let partial = discover_configured(
        &executable(),
        "fixture",
        "first-main",
        &all_capabilities(),
        &configured("partial"),
        std::future::pending(),
    )
    .await
    .unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.accounts.len(), 2);
    let health = check_configured(
        &executable(),
        "fixture",
        "first-main",
        &all_capabilities(),
        &configured("partial"),
        std::future::pending(),
    )
    .await
    .unwrap();
    assert_eq!(health.limitations.len(), 1);
    assert!(
        check_configured(
            &executable(),
            "fixture",
            "first-main",
            &all_capabilities(),
            &configured("checkfail"),
            std::future::pending()
        )
        .await
        .is_err()
    );
    for (instance, secret) in [
        ("two-first", "first-private-slot"),
        ("two-second", "second-private-slot"),
    ] {
        let invocation = Invocation::new(
            BTreeMap::new(),
            BTreeMap::from([(
                "client_secret".into(),
                permesh_secrets::Secret::new(secret.into()),
            )]),
        )
        .unwrap();
        let snapshot = discover_configured(
            &executable(),
            "fixture",
            instance,
            &all_capabilities(),
            &invocation,
            std::future::pending(),
        )
        .await
        .unwrap();
        assert!(
            snapshot
                .accounts
                .iter()
                .all(|account| account.key.provider == instance)
        );
        assert!(!format!("{snapshot:?} {invocation:?}").contains(secret));
    }
}
#[tokio::test]
async fn configured_cancellation_and_timeout_wait_for_cleanup() {
    let (directory, peer) = isolated_peer();
    let result = configured_discovery(
        &peer,
        "fixture",
        "safe-main",
        &all_capabilities(),
        &configured("test-hang"),
        wait_for_marker(directory.path().join("ready")),
        deadlines(),
    )
    .await;
    assert!(
        matches!(result, Err(ExternalError::Cancelled)),
        "{result:?}"
    );
    let result = configured_discovery(
        &executable(),
        "fixture",
        "safe-main",
        &all_capabilities(),
        &configured("hang"),
        std::future::pending(),
        Deadlines {
            operation: Duration::from_millis(800),
            handshake: Duration::from_millis(500),
            ..deadlines()
        },
    )
    .await;
    assert!(matches!(result, Err(ExternalError::Timeout)));
}

#[tokio::test]
async fn dropping_configured_future_terminates_the_peer() {
    let (directory, peer) = isolated_peer();
    let task = tokio::spawn(async move {
        discover_configured(
            &peer,
            "fixture",
            "safe-main",
            &all_capabilities(),
            &configured("test-hang"),
            std::future::pending(),
        )
        .await
    });
    wait_for_marker(directory.path().join("ready")).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_stopped(directory.path()).await;
}

#[tokio::test]
async fn setup_description_is_validated_without_delivering_configuration() {
    let spec = describe(
        &executable(),
        "fixture",
        "normal",
        &all_capabilities(),
        std::future::pending(),
    )
    .await
    .unwrap();
    spec.validate().unwrap();
    assert_eq!(spec.title, "Fixture setup");
    assert_eq!(spec.steps.len(), 4);
    for mode in [
        "describe-wrong-version",
        "wrong-provider",
        "wrong-caps",
        "describe-error",
        "describe-malformed",
        "describe-invalid",
        "describe-extra",
        "describe-nonzero",
    ] {
        let result = describe_with_deadlines(
            &executable(),
            "fixture",
            mode,
            &all_capabilities(),
            std::future::pending(),
            deadlines(),
        )
        .await;
        assert!(result.is_err(), "{mode}: {result:?}");
        assert!(!result.unwrap_err().to_string().contains("SENTINEL"));
    }
}
#[tokio::test]
async fn setup_deadline_includes_terminal_eof_and_cancel_uses_draft_three() {
    for mode in ["describe-timeout", "describe-no-eof"] {
        let mut limits = deadlines();
        limits.operation = Duration::from_millis(300);
        let result = describe_with_deadlines(
            &executable(),
            "fixture",
            mode,
            &all_capabilities(),
            std::future::pending(),
            limits,
        )
        .await;
        assert!(
            matches!(result, Err(ExternalError::Timeout)),
            "{mode}: {result:?}"
        );
    }
    let (dir, peer) = isolated_peer();
    let result = describe_with_deadlines(
        &peer,
        "fixture",
        "describe-cancel",
        &all_capabilities(),
        wait_for_marker(dir.path().join("ready")),
        deadlines(),
    )
    .await;
    assert!(
        matches!(result, Err(ExternalError::Cancelled)),
        "{result:?}"
    );
    assert!(dir.path().join("cancelled").exists());
}
