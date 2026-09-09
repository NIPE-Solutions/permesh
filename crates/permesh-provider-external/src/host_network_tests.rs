// SPDX-License-Identifier: MIT
use super::*;
use permesh_provider_sdk::network::NetworkContext;

fn network_invocation() -> Invocation {
    configured("normal")
        .with_network(NetworkContext {
            https_proxy: Some("http://127.0.0.1:3128".into()),
            no_proxy: vec!["example.invalid".into()],
            ca_bundle_pem: None,
        })
        .unwrap()
}

#[tokio::test]
async fn network_feature_rejection_withholds_invocations_for_both_operations() {
    for mode in [
        "network-unsupported",
        "network-null",
        "network-empty",
        "network-duplicate",
        "network-unknown",
    ] {
        for check in [false, true] {
            let (directory, peer) = isolated_peer();
            let invocation = network_invocation();
            let result = if check {
                check_negotiated(
                    &peer,
                    "fixture",
                    mode,
                    &all_capabilities(),
                    &invocation,
                    std::future::pending(),
                )
                .await
                .map(|_| ())
            } else {
                discover_negotiated(
                    &peer,
                    "fixture",
                    mode,
                    &all_capabilities(),
                    &invocation,
                    std::future::pending(),
                )
                .await
                .map(|_| ())
            };
            assert!(
                matches!(result, Err(ExternalError::NetworkNegotiation)),
                "{mode}: {result:?}"
            );
            assert!(!directory.path().join("delivered").exists());
        }
    }
}

#[tokio::test]
async fn negotiated_network_context_reaches_only_feature_aware_peers() {
    let (directory, peer) = isolated_peer();
    let invocation = network_invocation();
    assert!(
        discover_negotiated(
            &peer,
            "fixture",
            "network-ok",
            &all_capabilities(),
            &invocation,
            std::future::pending()
        )
        .await
        .unwrap()
        .complete
    );
    assert!(
        check_negotiated(
            &peer,
            "fixture",
            "network-ok",
            &all_capabilities(),
            &invocation,
            std::future::pending()
        )
        .await
        .unwrap()
        .limitations
        .is_empty()
    );
    assert!(directory.path().join("delivered").exists());
}

#[tokio::test]
async fn legacy_network_context_rejects_before_attempting_to_spawn() {
    let invocation = network_invocation();
    // A missing executable would yield a spawn failure if the contract gate ran too late.
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("missing-peer");
    assert!(matches!(
        discover_configured(
            &executable,
            "fixture",
            "legacy",
            &all_capabilities(),
            &invocation,
            std::future::pending()
        )
        .await,
        Err(ExternalError::Input)
    ));
    assert!(matches!(
        check_configured(
            &executable,
            "fixture",
            "legacy",
            &all_capabilities(),
            &invocation,
            std::future::pending()
        )
        .await,
        Err(ExternalError::Input)
    ));
}

#[tokio::test]
async fn rejected_network_feature_writes_only_the_public_handshake() {
    for operation in [
        negotiated::Operation::Discover,
        negotiated::Operation::Check,
    ] {
        let mut child = Command::new(executable())
            .arg("--capture-input")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take();
        let features = &[negotiated::Feature::NetworkV1];
        let handshake =
            negotiated::handshake_request_with_features("network-main", operation, features)
                .unwrap();
        let response = format!(
            "{}\n",
            serde_json::json!({
                "protocol_version":1,"id":"handshake","event":"handshake","provider":"fixture",
                "capabilities":[],"operations":["discover","check"],"draft":true,
            })
        );
        let invocation = network_invocation();
        let mut total = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = match operation {
            negotiated::Operation::Discover => exchange(
                &mut stdin,
                response.as_bytes(),
                &mut total,
                Exchange {
                    decoder: negotiated::DiscoveryDecoder::with_required_features(
                        "fixture",
                        "network-main",
                        Some(&[]),
                        features,
                    )
                    .unwrap(),
                    handshake: handshake.clone(),
                    invocation: Some(&invocation),
                    method: "discover",
                    contract: Contract::Negotiated,
                },
                deadline,
            )
            .await
            .map(|_| ()),
            negotiated::Operation::Check => exchange(
                &mut stdin,
                response.as_bytes(),
                &mut total,
                Exchange {
                    decoder: negotiated::HealthDecoder::with_required_features(
                        "fixture",
                        "network-main",
                        Some(&[]),
                        features,
                    )
                    .unwrap(),
                    handshake: handshake.clone(),
                    invocation: Some(&invocation),
                    method: "check",
                    contract: Contract::Negotiated,
                },
                deadline,
            )
            .await
            .map(|_| ()),
        };
        stdin.take();
        let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(output.status.success());
        assert!(matches!(result, Err(ExternalError::NetworkNegotiation)));
        assert_eq!(
            output.stdout, handshake,
            "unsupported network policy must withhold all invocation bytes"
        );
    }
}
