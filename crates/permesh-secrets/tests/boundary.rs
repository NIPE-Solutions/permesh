// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_secrets::{Secret, SecretRef, SecretResolver};
#[test]
fn reference_syntax_is_strict_and_errors_redacted() {
    for valid in [
        "env://TOKEN",
        "env://_TOKEN2",
        "keychain://github-main/token",
        "keychain://internal-main/client_secret",
    ] {
        assert_eq!(SecretRef::parse(valid).unwrap().to_string(), valid);
    }
    for invalid in [
        "sentinel-plaintext",
        "exec://echo",
        "env://",
        "env://2TOKEN",
        "env://A/B",
        "env://A?x",
        "keychain://../token",
        "keychain://gh/../password",
        "keychain://gh/token/",
        "keychain://gh%2f/token",
        "env://A\n",
    ] {
        let err = SecretRef::parse(invalid).unwrap_err();
        assert!(!format!("{err:?} {err}").contains("sentinel-plaintext"));
    }
}
#[test]
fn secrets_are_redacted_and_unset_env_is_safe() {
    let secret = Secret::new("sentinel-value".into());
    assert_eq!(secret.expose(), "sentinel-value");
    assert!(!format!("{secret:?}").contains("sentinel-value"));
    assert!(
        SecretResolver
            .resolve(&SecretRef::Env("PERMESH_TEST_UNSET_508943".into()))
            .is_err()
    );
}
#[test]
fn env_resolution_uses_subprocess_environment() {
    const NAME: &str = "PERMESH_TEST_SUBPROCESS_SECRET_93483";
    if std::env::var_os(NAME).is_some() {
        let secret = SecretResolver
            .resolve(&SecretRef::parse(&format!("env://{NAME}")).unwrap())
            .unwrap();
        assert_eq!(secret.expose(), "sentinel-env-value");
        assert!(!format!("{secret:?}").contains("sentinel-env-value"));
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "env_resolution_uses_subprocess_environment"])
        .env(NAME, "sentinel-env-value")
        .output()
        .unwrap();
    assert!(output.status.success());
}
#[test]
fn invalid_public_variants_never_reach_native_store() {
    let reference = SecretRef::Keychain {
        service: "sentinel/bad".into(),
        account: "token".into(),
    };
    assert!(SecretResolver.resolve(&reference).is_err());
    assert!(permesh_secrets::store(&reference, &Secret::new("value".into())).is_err());
    assert!(permesh_secrets::delete(&reference).is_err());
    assert!(!format!("{reference:?} {reference}").contains("sentinel"));
    assert!(permesh_secrets::delete(&SecretRef::Env("TOKEN".into())).is_err());
}
