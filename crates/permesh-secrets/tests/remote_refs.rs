// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_secrets::{SecretRef, SecretResolver};
#[test]
fn remote_references_are_only_nonsecret_local_names_and_never_resolve_directly() {
    for scheme in ["1password", "vault", "openbao"] {
        let value = format!("{scheme}://provider-token");
        let reference = SecretRef::parse(&value).unwrap();
        assert_eq!(reference.to_string(), value);
        assert!(SecretResolver.resolve(&reference).is_err());
        for suffix in [
            "",
            "other/secret",
            "a?b",
            "a#b",
            "a%2fb",
            "a@host",
            "a\\b",
            "a\n",
            "..",
        ] {
            assert!(SecretRef::parse(&format!("{scheme}://{suffix}")).is_err());
        }
    }
}
