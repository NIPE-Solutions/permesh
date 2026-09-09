// SPDX-License-Identifier: MIT
use super::*;

fn portable(config: &mut Config) -> TestResult<(&'static str, &'static str)> {
    let native = permesh_provider_sdk::target::native_target().ok_or("native target")?;
    let foreign = permesh_provider_sdk::target::TARGETS
        .iter()
        .copied()
        .find(|target| *target != native)
        .ok_or("foreign target")?;
    let external = config.providers[0].external.as_mut().ok_or("external")?;
    let digest = external.sha256.take().ok_or("scalar digest")?;
    external.sha256_by_target = Some(BTreeMap::from([
        (native.into(), digest),
        (foreign.into(), "a".repeat(64)),
    ]));
    Ok((native, foreign))
}
#[test]
fn portable_selection_ignores_mutable_registry_selection_and_reviews_full_map() -> TestResult {
    let (_temporary, access, mut config, path) = setup(&[])?;
    let (native, foreign) = portable(&mut config)?;
    let external = config.providers[0].external.as_ref().ok_or("external")?;
    let expected = external.digest_for_target(Some(native))?.to_owned();
    let other = path.with_file_name("other-native");
    std::fs::write(&other, b"\x7fELFnewer selected binary")?;
    let changed = inspect(&other)?.sha256;
    let registry = Registry::new(access.root.clone())?;
    registry.trust(&other, "fixture", &changed, &[])?;
    assert_eq!(registry.load("fixture")?.sha256, changed);
    let review = access
        .review(&config, &path, "instance")
        .map_err(|e| e.message)?;
    assert_eq!(review.report.result["registration"]["sha256"], expected);
    assert_eq!(
        review.report.result["target_pins"]["resolved_target"],
        native
    );
    assert_eq!(
        review.report.result["target_pins"]["resolved_sha256"],
        expected
    );
    assert_eq!(
        review.report.result["target_pins"]["sha256_by_target"][foreign],
        "a".repeat(64)
    );
    let fingerprint = review.report.result["fingerprint"]
        .as_str()
        .ok_or("fingerprint")?;
    access
        .approve(&config, &path, "instance", fingerprint, true)
        .map_err(|e| e.message)?;
    let (_, registration) = access
        .authorize(&config, &path, "instance")
        .map_err(|e| e.message)?;
    assert_eq!(registration.sha256, expected);
    assert!(
        access
            .prepare(&config, &path, &config.providers[0])
            .is_err_and(|e| e.message.contains("credential unavailable"))
    );
    let mut output = Vec::new();
    crate::external_output::write(&mut output, "external_review", &review.report.result)?;
    let output = String::from_utf8(output)?;
    assert!(output.contains(native) && output.contains(foreign) && output.contains(&expected));
    assert!(output.contains("Each target requires local binary trust"));
    Ok(())
}
#[test]
fn foreign_and_native_pin_changes_invalidate_approval_before_credentials() -> TestResult {
    let (_temporary, access, mut config, path) = setup(&[])?;
    let (native, foreign) = portable(&mut config)?;
    let other = path.with_file_name("other-native");
    std::fs::write(&other, b"\x7fELFsecond retained fixture")?;
    let changed = inspect(&other)?.sha256;
    Registry::new(access.root.clone())?.trust(&other, "fixture", &changed, &[])?;
    let (_, _, fingerprint) = access
        .reviewed(&config, &path, "instance")
        .map_err(|e| e.message)?;
    access
        .approve(&config, &path, "instance", &fingerprint, true)
        .map_err(|e| e.message)?;
    for target in [native, foreign] {
        let mut edited = config.clone();
        edited.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .sha256_by_target
            .as_mut()
            .ok_or("pins")?
            .insert(target.into(), changed.clone());
        assert!(
            access
                .prepare(&edited, &path, &edited.providers[0])
                .is_err_and(|e| e.message.contains("approval"))
        );
        assert!(
            access
                .approve(&edited, &path, "instance", &fingerprint, true)
                .is_err()
        );
        // Passing a previously selected provider cannot bypass changed map context.
        assert!(
            access
                .prepare(&edited, &path, &config.providers[0])
                .is_err_and(|e| e.message.contains("approval"))
        );
    }
    let mut missing = config.clone();
    missing.providers[0]
        .external
        .as_mut()
        .ok_or("external")?
        .sha256_by_target
        .as_mut()
        .ok_or("pins")?
        .remove(native);
    // Even a deliberately invalid registry root is not inspected before target selection.
    let invalid_access = WorkspaceAccess {
        root: PathBuf::from("relative-registry"),
    };
    let error = invalid_access
        .prepare(&missing, &path, &missing.providers[0])
        .err()
        .ok_or("missing native must fail")?;
    assert!(!error.message.contains("credential unavailable"));
    assert!(error.message.contains("target"));
    assert!(access.review(&missing, &path, "instance").is_err());
    let mut untrusted = config.clone();
    untrusted.providers[0]
        .external
        .as_mut()
        .ok_or("external")?
        .sha256_by_target
        .as_mut()
        .ok_or("pins")?
        .insert(native.into(), "f".repeat(64));
    assert!(
        access
            .prepare(&untrusted, &path, &untrusted.providers[0])
            .is_err_and(|e| !e.message.contains("credential unavailable"))
    );
    Ok(())
}
#[test]
fn scalar_approval_context_and_output_are_unchanged_and_map_order_is_canonical() -> TestResult {
    let (_temporary, access, mut config, path) = setup(&[])?;
    let (_, registration, actual) = access
        .reviewed(&config, &path, "instance")
        .map_err(|e| e.message)?;
    let original = serde_json::json!({"approval_context":"permesh-provider-context-v2", "workspace_schema":config.version, "provider":config.providers[0], "identity_sources":[], "identity_aliases":{}});
    assert_eq!(
        actual,
        fingerprint(&path, "instance", &original, &registration)?
    );
    access
        .approvals()
        .map_err(|e| e.message)?
        .approve(&path, "instance", &actual)?;
    access
        .authorize(&config, &path, "instance")
        .map_err(|e| e.message)?;
    let result = access
        .review(&config, &path, "instance")
        .map_err(|e| e.message)?
        .report
        .result;
    assert!(result.get("target_pins").is_none());
    let (native, foreign) = portable(&mut config)?;
    let (_, registration, first) = access
        .reviewed(&config, &path, "instance")
        .map_err(|e| e.message)?;
    let context = serde_json::json!({"approval_context":"permesh-provider-context-v2", "workspace_schema":config.version, "provider":config.providers[0], "identity_sources":[], "identity_aliases":{}, "resolved_target":native, "resolved_sha256":registration.sha256});
    assert_eq!(
        first,
        fingerprint(&path, "instance", &context, &registration)?
    );
    let pins = config.providers[0]
        .external
        .as_mut()
        .ok_or("external")?
        .sha256_by_target
        .as_mut()
        .ok_or("pins")?;
    let native_digest = pins.remove(native).ok_or("native pin")?;
    let foreign_digest = pins.remove(foreign).ok_or("foreign pin")?;
    pins.insert(foreign.into(), foreign_digest);
    pins.insert(native.into(), native_digest);
    let (_, _, reordered) = access
        .reviewed(&config, &path, "instance")
        .map_err(|e| e.message)?;
    assert_eq!(first, reordered);
    Ok(())
}

#[test]
fn portable_approval_binds_security_context_and_ignores_unrelated_providers() -> TestResult {
    let (_temporary, access, mut config, path) = setup(&[Capability::Identities])?;
    portable(&mut config)?;
    let (_, _, fingerprint) = access
        .reviewed(&config, &path, "instance")
        .map_err(|e| e.message)?;
    access
        .approve(&config, &path, "instance", &fingerprint, true)
        .map_err(|e| e.message)?;
    config.organization.name = "Unrelated rename".into();
    config.providers.push(ProviderConfig {
        inventory: None,
        id: "demo".into(),
        kind: ProviderKind::Demo,
        organizations: vec![],
        customer_id: None,
        auth: None,
        external: None,
    });
    access
        .authorize(&config, &path, "instance")
        .map_err(|e| e.message)?;
    for change in ["selector", "authority", "alias", "network"] {
        let mut edited = config.clone();
        match change {
            "selector" => {
                edited.providers[0]
                    .external
                    .as_mut()
                    .ok_or("external")?
                    .discovery_protocol = permesh_config::DiscoveryProtocol::NegotiatedV1
            }
            "authority" => edited
                .identity
                .sources
                .push(permesh_config::IdentitySource {
                    provider: "instance".into(),
                    authoritative: true,
                }),
            "alias" => {
                edited.identity.aliases.insert(
                    "person@example.com".into(),
                    BTreeMap::from([("instance".into(), vec!["native-id".into()])]),
                );
            }
            "network" => {
                let external = edited.providers[0].external.as_mut().ok_or("external")?;
                external.discovery_protocol = permesh_config::DiscoveryProtocol::NegotiatedV1;
                external.network = Some(permesh_config::NetworkConfig {
                    https_proxy: Some("http://proxy.example:8080".into()),
                    no_proxy: vec![],
                    ca_bundle: None,
                });
            }
            _ => unreachable!(),
        }
        assert!(
            access
                .prepare(&edited, &path, &edited.providers[0])
                .is_err_and(|e| e.message.contains("approval")),
            "{change}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn prepared_portable_invocation_rechecks_the_retained_pin_before_launch() -> TestResult {
    let (_temporary, access, mut config, path) = setup(&[])?;
    let (native, _) = portable(&mut config)?;
    let external = config.providers[0].external.as_mut().ok_or("external")?;
    external.credentials.clear();
    let expected = external.digest_for_target(Some(native))?.to_owned();
    let other = path.with_file_name("other-native");
    std::fs::write(&other, b"\x7fELFnew selected executable")?;
    let registry = Registry::new(access.root.clone())?;
    registry.trust(&other, "fixture", &inspect(&other)?.sha256, &[])?;
    let (_, _, fingerprint) = access.reviewed(&config, &path, "instance").map_err(|error| error.message)?;
    access.approve(&config, &path, "instance", &fingerprint, true).map_err(|error| error.message)?;
    let (executable, registration, invocation) = access.prepare(&config, &path, &config.providers[0]).map_err(|error| error.message)?;
    assert_eq!(registration.sha256, expected);
    std::fs::write(&executable, b"\x7fELFsubstituted after preparation")?;
    let result = permesh_provider_external::host::check_configured(&executable, "fixture", "instance", &[], &invocation, std::future::pending()).await;
    assert!(matches!(result, Err(permesh_provider_external::ExternalError::Trust)));
    Ok(())
}
