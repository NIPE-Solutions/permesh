// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
const PROFILE: &str = "[other]\ncredential_process = never-execute-me\n[review]\naws_access_key_id = ASIAEXAMPLE1234567890\naws_secret_access_key = synthetic-secret-value\naws_session_token = synthetic-session-value\n";
#[test]
fn selects_exact_named_temporary_profile_without_fallback() {
    let session = parse(PROFILE, "review").unwrap();
    assert_eq!(session.access_key_id.expose(), "ASIAEXAMPLE1234567890");
    assert_eq!(session.session_token.expose(), "synthetic-session-value");
    assert!(parse(PROFILE, "missing").is_err());
    assert!(parse(PROFILE, "other").is_err());
}
#[test]
fn rejects_static_credentials_ambiguous_or_executable_profile_features() {
    for text in [
        PROFILE.replace("aws_session_token = synthetic-session-value\n", ""),
        format!("{PROFILE}credential_process = command\n"),
        format!("{PROFILE}role_arn = arn:aws:iam::123456789012:role/reader\n"),
        format!("{PROFILE}sso_session = login\n"),
        format!("{PROFILE}aws_session_token = duplicate\n"),
        format!("{PROFILE}[review]\n"),
        PROFILE.replace("synthetic-session-value", "secret with spaces"),
        PROFILE.replace("ASIAEXAMPLE", "AKIAEXAMPLE"),
        PROFILE.replace("synthetic-session-value", "\"quoted-session\""),
    ] {
        assert!(parse(&text, "review").is_err());
    }
}
#[test]
fn errors_never_reflect_source_contents_or_profile_names() {
    let error = parse(
        "[sensitive-name]\ncredential_process = confidential-command",
        "sensitive-name",
    )
    .err()
    .unwrap();
    let rendered = format!("{error:?}");
    assert!(!rendered.contains("sensitive-name"));
    assert!(!rendered.contains("confidential-command"));
}
#[test]
fn bounds_hostile_input_and_keeps_the_session_together() {
    assert!(parse(&"x".repeat(MAX_BYTES + 1), "review").is_err());
    assert!(parse(&format!("{PROFILE}#{}", "x".repeat(MAX_LINE + 1)), "review").is_err());
    assert!(parse(PROFILE, "../review").is_err());
    let credentials = parse(PROFILE, "review").unwrap().credentials();
    assert_eq!(credentials.len(), 3);
    assert_eq!(
        credentials["secret_access_key"].expose(),
        "synthetic-secret-value"
    );
    assert!(!format!("{credentials:?}").contains("synthetic-secret-value"));
}
#[test]
fn source_requires_an_explicit_regular_private_file() {
    let directory = tempfile::tempdir().unwrap();
    let directory = directory.path().canonicalize().unwrap();
    let path = directory.join("credentials");
    std::fs::write(&path, PROFILE).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(load(&path, "review").is_ok());
    assert!(load(std::path::Path::new("credentials"), "review").is_err());
    assert!(load(&directory, "review").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let link = directory.join("link");
        symlink(&path, &link).unwrap();
        assert!(load(&link, "review").is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load(&path, "review").is_err());
    }
}
