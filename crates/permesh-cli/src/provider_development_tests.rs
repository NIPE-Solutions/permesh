// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
#[test]
fn transcript_validation_preserves_incomplete_and_rejects_trailing_or_duplicate_frames() {
    let good = b"{\"protocol_version\":1,\"id\":\"handshake\",\"event\":\"handshake\",\"provider\":\"example\",\"capabilities\":[],\"operations\":[\"discover\"],\"draft\":true}\n{\"protocol_version\":1,\"id\":\"discover\",\"event\":\"complete\",\"count\":0,\"complete\":false,\"limitations\":[\"visibility_limited\"]}\n";
    let summary = validate_bytes(
        good,
        "example",
        "example-main",
        Operation::Discover,
        DiscoveryProtocol::NegotiatedV1,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(summary["complete"], false);
    let mut trailing = good.to_vec();
    trailing.extend_from_slice(b"SENTINEL_SECRET\n");
    let error = validate_bytes(
        &trailing,
        "example",
        "example-main",
        Operation::Discover,
        DiscoveryProtocol::NegotiatedV1,
        &Cancellation::new(),
    )
    .unwrap_err();
    assert!(!error.message.contains("SENTINEL_SECRET"));
    assert!(
        validate_bytes(
            b"{\"id\":1,\"id\":2}\n",
            "example",
            "example-main",
            Operation::Discover,
            DiscoveryProtocol::NegotiatedV1,
            &Cancellation::new()
        )
        .is_err()
    );
}
#[test]
fn scaffold_refuses_overwrite_bad_ids_and_preserves_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/native-runtime/src")).unwrap();
    std::fs::create_dir(dir.path().join("providers")).unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), b"[workspace]\nmembers=[]\n").unwrap();
    std::fs::write(
        dir.path().join("crates/native-runtime/Cargo.toml"),
        b"[package]\nname='permesh-native-runtime'\n",
    )
    .unwrap();
    let before = std::fs::read(dir.path().join("Cargo.toml")).unwrap();
    let result = scaffold(dir.path(), "example", &Cancellation::new()).unwrap();
    assert_eq!(result["executed"], false);
    assert!(dir.path().join("providers/example/src/main.rs").is_file());
    assert!(scaffold(dir.path(), "example", &Cancellation::new()).is_err());
    assert!(scaffold(dir.path(), "../escape", &Cancellation::new()).is_err());
    assert_eq!(
        std::fs::read(dir.path().join("Cargo.toml")).unwrap(),
        before
    );
    let cancelled = Cancellation::new();
    cancelled.cancel();
    assert!(scaffold(dir.path(), "cancelled", &cancelled).is_err());
    assert!(!dir.path().join("providers/cancelled").exists());
}
#[cfg(unix)]
#[test]
fn scaffold_rejects_symlink_provider_directory() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[workspace]").unwrap();
    std::fs::create_dir_all(root.path().join("crates/native-runtime")).unwrap();
    std::fs::write(
        root.path().join("crates/native-runtime/Cargo.toml"),
        "[package]",
    )
    .unwrap();
    std::os::unix::fs::symlink(other.path(), root.path().join("providers")).unwrap();
    assert!(scaffold(root.path(), "escape", &Cancellation::new()).is_err());
    assert!(!other.path().join("escape").exists());
}
#[test]
fn offline_validation_enforces_bounds_and_observes_cancellation() {
    let huge = vec![b'x'; protocol::MAX_FRAME_BYTES + 1];
    assert!(
        validate_bytes(
            &huge,
            "example",
            "work",
            Operation::Discover,
            DiscoveryProtocol::NegotiatedV1,
            &Cancellation::new()
        )
        .is_err()
    );
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        validate_bytes(
            b"",
            "example",
            "work",
            Operation::Discover,
            DiscoveryProtocol::NegotiatedV1,
            &cancel
        )
        .unwrap_err()
        .code,
        130
    );
    assert!(
        validate_bytes(
            b"{}",
            "example",
            "work",
            Operation::Discover,
            DiscoveryProtocol::NegotiatedV1,
            &Cancellation::new()
        )
        .is_err()
    );
}
#[cfg(unix)]
#[test]
fn transcript_open_recheck_rejects_replacement_with_same_length() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::write(&first, b"same").unwrap();
    std::fs::write(&second, b"size").unwrap();
    let before = std::fs::symlink_metadata(&first).unwrap();
    let original = std::fs::File::open(&first).unwrap().metadata().unwrap();
    let replacement = std::fs::File::open(&second).unwrap().metadata().unwrap();
    assert!(same_file(&before, &original).is_ok());
    assert!(same_file(&before, &replacement).is_err());
}
