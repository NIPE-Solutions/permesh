// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_external::host::discover;
#[tokio::test]
async fn native_peer_completes_discovery() {
    let snapshot = discover(
        std::path::Path::new(env!("CARGO_BIN_EXE_permesh-test-external-peer")),
        "fixture",
        "normal",
        &[],
        std::future::pending(),
    )
    .await
    .unwrap();
    assert!(snapshot.complete);
    assert_eq!(snapshot.provider, "normal");
}
