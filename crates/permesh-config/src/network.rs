// SPDX-License-Identifier: MIT
//! Explicit nonsecret network settings and workspace-relative CA pins.
use crate::{Result, invalid};
use permesh_provider_sdk::network::NetworkContext;
use serde::{Deserialize, Serialize};

pub(crate) fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaBundle {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub https_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_proxy: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub ca_bundle: Option<CaBundle>,
}

impl std::fmt::Debug for NetworkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NetworkConfig([REDACTED])")
    }
}

impl NetworkConfig {
    pub fn validate(&self) -> Result<()> {
        if self.https_proxy.is_none() && self.ca_bundle.is_none() {
            return Err(invalid(
                "external network requires an explicit proxy or CA bundle",
            ));
        }
        // Validate routing independently of loading the workspace-relative CA file.
        if self.https_proxy.is_some() || !self.no_proxy.is_empty() {
            NetworkContext {
                https_proxy: self.https_proxy.clone(),
                no_proxy: self.no_proxy.clone(),
                ca_bundle_pem: None,
            }
            .validate()
            .map_err(|_| invalid("invalid external network routing settings"))?;
        }
        if let Some(ca) = &self.ca_bundle
            && (ca.path.is_empty()
                || ca.path.len() > 1024
                || ca.path.starts_with('/')
                || ca.path.contains(['\\', ':'])
                || ca.path.chars().any(char::is_control)
                || ca
                    .path
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
                || ca.sha256.len() != 64
                || !ca
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err(invalid(
                "external CA bundle requires a relative file path and lowercase SHA-256 pin",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::Config;
    use serde_json::{Value, json};
    fn config(network: Value, negotiated: bool) -> Vec<u8> {
        let mut external = json!({"provider":"fixture","sha256":"a".repeat(64),"network":network});
        if negotiated {
            external["discovery_protocol"] = json!("negotiated_v1");
        }
        serde_json::to_vec(&json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":external}]})).unwrap()
    }
    #[test]
    fn network_requires_explicit_negotiated_bounded_nonsecret_settings() {
        let proxy = json!({"https_proxy":"http://proxy.example:8080","no_proxy":["example.test"]});
        assert!(Config::from_bytes(&config(proxy.clone(), true)).is_ok());
        assert!(Config::from_bytes(&config(proxy, false)).is_err());
        for bad in [
            Value::Null,
            json!({}),
            json!({"https_proxy":null}),
            json!({"ca_bundle":null}),
            json!({"no_proxy":null}),
            json!({"https_proxy":"http://user:password@proxy.test"}),
            json!({"https_proxy":"http://proxy.test/path"}),
            json!({"no_proxy":["example.test"]}),
            json!({"https_proxy":"http://proxy.test","extra":true}),
            json!({"https_proxy":"http://proxy.test","no_proxy":["a","a"]}),
        ] {
            assert!(Config::from_bytes(&config(bad, true)).is_err());
        }
        for path in [
            "../ca.pem",
            "/ca.pem",
            "C:/ca.pem",
            "a\\ca.pem",
            "a/../ca.pem",
            "a//ca.pem",
            "./ca.pem",
            "",
        ] {
            let ca = json!({"ca_bundle":{"path":path,"sha256":"b".repeat(64)}});
            assert!(Config::from_bytes(&config(ca, true)).is_err());
        }
        assert!(
            Config::from_bytes(&config(
                json!({"ca_bundle":{"path":"certs/ca.pem","sha256":"b".repeat(64)}}),
                true
            ))
            .is_ok()
        );
        let bytes =
            String::from_utf8(config(json!({"https_proxy":"http://proxy.test"}), true)).unwrap();
        let duplicate = bytes.replace(
            "\"https_proxy\":",
            "\"https_proxy\":\"http://other.test\",\"https_proxy\":",
        );
        assert!(Config::from_bytes(duplicate.as_bytes()).is_err());
    }
    #[test]
    fn invalid_proxy_is_not_exposed_by_debug_or_validation_errors() {
        let settings: super::NetworkConfig = serde_json::from_value(
            json!({"https_proxy":"http://user:SENTINEL_PRIVATE@proxy.test"}),
        )
        .unwrap();
        assert!(!format!("{settings:?}").contains("SENTINEL_PRIVATE"));
        assert!(
            !settings
                .validate()
                .unwrap_err()
                .to_string()
                .contains("SENTINEL_PRIVATE")
        );
    }
    #[test]
    fn absent_network_preserves_external_serialization() {
        let old = format!(
            r#"{{"provider":"fixture","sha256":"{}","configuration":{{}},"credentials":{{}}}}"#,
            "a".repeat(64)
        );
        let external: crate::ExternalConfig = serde_json::from_str(&old).unwrap();
        assert_eq!(serde_json::to_string(&external).unwrap(), old);
    }
}
