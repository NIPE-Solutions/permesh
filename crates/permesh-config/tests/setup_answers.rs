// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
#[test]
fn setup_answers_accept_typed_yaml_and_json_without_duplicates_or_diagnostic_payloads() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("answers.yaml");
    std::fs::write(
        &path,
        "version: 1\nanswers: {tenant: acme, enabled: true, port: 443, regions: [eu, us]}\n",
    )
    .unwrap();
    let values = permesh_config::load_setup_answers(&path).unwrap();
    assert_eq!(values["port"], 443);
    assert_eq!(values["enabled"], true);
    for source in [
        "version: 1\nanswers: {tenant: one, tenant: two}",
        "version: 2\nanswers: {}",
        "version: 1\nanswers: {}\nextra: SENTINEL",
        "version: 1\nanswers: {tenant: [}",
    ] {
        std::fs::write(&path, source).unwrap();
        let error = permesh_config::load_setup_answers(&path)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("SENTINEL"));
    }
    std::fs::write(
        &path,
        format!("version: 1\nanswers: {{tenant: '{}'}}", "x".repeat(65_536)),
    )
    .unwrap();
    assert!(permesh_config::load_setup_answers(&path).is_err());
    assert!(permesh_config::load_setup_answers(root.path()).is_err());
}
