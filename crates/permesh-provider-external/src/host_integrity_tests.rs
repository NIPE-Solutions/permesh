// SPDX-License-Identifier: MIT
use super::*;
use crate::trust::{Registry, inspect};

async fn launch(
    operation: usize,
    peer: &Path,
    digest: &str,
    cancelled: bool,
) -> Result<(), ExternalError> {
    let cancellation = async move {
        if !cancelled {
            std::future::pending::<()>().await;
        }
    };
    let invocation = configured("normal").with_executable_pin(digest).unwrap();
    match operation {
        0 => discover_pinned(peer, "fixture", "normal", &[], digest, cancellation)
            .await
            .map(|_| ()),
        1 => discover_configured(
            peer,
            "fixture",
            "normal",
            &all_capabilities(),
            &invocation,
            cancellation,
        )
        .await
        .map(|_| ()),
        2 => check_configured(
            peer,
            "fixture",
            "normal",
            &all_capabilities(),
            &invocation,
            cancellation,
        )
        .await
        .map(|_| ()),
        3 => discover_negotiated(
            peer,
            "fixture",
            "normal",
            &all_capabilities(),
            &invocation,
            cancellation,
        )
        .await
        .map(|_| ()),
        4 => check_negotiated(
            peer,
            "fixture",
            "normal",
            &all_capabilities(),
            &invocation,
            cancellation,
        )
        .await
        .map(|_| ()),
        5 => describe_pinned(
            peer,
            "fixture",
            "normal",
            &all_capabilities(),
            digest,
            cancellation,
        )
        .await
        .map(|_| ()),
        6 => describe_auth_pinned(peer, "fixture", "normal", &[], digest, cancellation)
            .await
            .map(|_| ()),
        7 => discover(peer, "fixture", "normal", &[], cancellation)
            .await
            .map(|_| ()),
        8 => describe(peer, "fixture", "normal", &all_capabilities(), cancellation)
            .await
            .map(|_| ()),
        9 => describe_auth(peer, "fixture", "normal", &[], cancellation)
            .await
            .map(|_| ()),
        _ => unreachable!(),
    }
}
fn trusted_peer() -> (tempfile::TempDir, PathBuf, String) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let registry = Registry::new(root.join("providers")).unwrap();
    let digest = inspect(&executable()).unwrap().sha256;
    let registration = registry
        .trust(&executable(), "fixture", &digest, &all_capabilities())
        .unwrap();
    let peer = registry.verify(&registration).unwrap();
    std::fs::write(peer.parent().unwrap().join("record-startup"), b"enabled").unwrap();
    (temp, peer, digest)
}
#[tokio::test]
async fn precancelled_operations_do_not_start_native_code_or_inspect_missing_executable() {
    let (_temp, peer, digest) = trusted_peer();
    let missing = peer.with_file_name("missing-provider");
    for operation in 0..10 {
        for path in [&peer, &missing] {
            assert!(
                matches!(
                    launch(operation, path, &digest, true).await,
                    Err(ExternalError::Cancelled)
                ),
                "operation {operation}"
            );
        }
        assert!(!peer.parent().unwrap().join("started").exists());
    }
}
#[tokio::test]
async fn changed_managed_native_bytes_reject_before_startup_for_every_operation() {
    for truncation in [false, true] {
        let (_temp, peer, digest) = trusted_peer();
        if truncation {
            std::fs::write(&peer, b"truncated").unwrap();
        } else {
            let mut bytes = std::fs::read(&peer).unwrap();
            // Keep an executable native file, with a different digest.
            bytes.extend_from_slice(b"deterministic replacement");
            std::fs::write(&peer, bytes).unwrap();
        }
        for operation in 0..7 {
            assert!(
                matches!(
                    launch(operation, &peer, &digest, false).await,
                    Err(ExternalError::Trust)
                ),
                "operation {operation}"
            );
            assert!(!peer.parent().unwrap().join("started").exists());
        }
    }
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_replacement_rejects_even_when_target_digest_matches() {
    let (_temp, peer, digest) = trusted_peer();
    let retained = peer.with_file_name("retained.exe");
    std::fs::rename(&peer, &retained).unwrap();
    std::os::unix::fs::symlink(&retained, &peer).unwrap();
    assert!(matches!(
        launch(3, &peer, &digest, false).await,
        Err(ExternalError::Trust)
    ));
    assert!(!peer.parent().unwrap().join("started").exists());
}

#[tokio::test]
async fn cancellation_observed_at_final_poll_prevents_native_startup() {
    let (_temp, peer, digest) = trusted_peer();
    let invocation = configured("normal").with_executable_pin(&digest).unwrap();
    let mut polls = 0;
    let cancel = std::future::poll_fn(|_| {
        polls += 1;
        if polls == 1 {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    });
    assert!(matches!(
        discover_negotiated(
            &peer,
            "fixture",
            "normal",
            &all_capabilities(),
            &invocation,
            cancel
        )
        .await,
        Err(ExternalError::Cancelled)
    ));
    assert_eq!(polls, 2);
    assert!(!peer.parent().unwrap().join("started").exists());
}
#[tokio::test]
async fn unchanged_pinned_native_peer_can_still_complete_discovery_and_setup() {
    let (_temp, peer, digest) = trusted_peer();
    for operation in 0..6 {
        launch(operation, &peer, &digest, false).await.unwrap();
    }
    assert!(peer.parent().unwrap().join("started").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn changed_executable_or_parent_permissions_reject_before_native_startup() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, peer, digest) = trusted_peer();
    for path in [&peer, peer.parent().unwrap()] {
        let original = std::fs::metadata(path).unwrap().permissions();
        let unsafe_permissions = std::fs::Permissions::from_mode(original.mode() | 0o022);
        std::fs::set_permissions(path, unsafe_permissions).unwrap();
        let result = launch(3, &peer, &digest, false).await;
        std::fs::set_permissions(path, original).unwrap();
        assert!(matches!(result, Err(ExternalError::Trust)));
        assert!(!peer.parent().unwrap().join("started").exists());
    }
}
