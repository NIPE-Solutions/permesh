// SPDX-License-Identifier: MIT
//! Strict portable digest alternatives; selection performs no host or filesystem access.
use crate::{ExternalConfig, Result, invalid};
use permesh_provider_sdk::target::TARGETS;
use serde::de::{MapAccess, Visitor};
use std::{collections::BTreeMap, fmt};

pub(crate) fn target_map<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<BTreeMap<String, String>>, D::Error> {
    struct Targets;
    impl<'de> Visitor<'de> for Targets {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded target-to-digest map with unique keys")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((target, digest)) = map.next_entry::<String, String>()? {
                if values.len() == TARGETS.len() || values.insert(target, digest).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate or excessive provider target pins",
                    ));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Targets).map(Some)
}
fn digest_valid(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl ExternalConfig {
    pub(crate) fn validate_pins(&self) -> Result<()> {
        match (&self.sha256, &self.sha256_by_target) {
            (Some(digest), None) if digest_valid(digest) => Ok(()),
            (None, Some(targets))
                if !targets.is_empty()
                    && targets.len() <= TARGETS.len()
                    && targets.iter().all(|(target, digest)| {
                        TARGETS.contains(&target.as_str()) && digest_valid(digest)
                    }) =>
            {
                Ok(())
            }
            _ => Err(invalid(
                "external requires exactly one valid sha256 or nonempty sha256_by_target map with supported targets and lowercase SHA-256 digests",
            )),
        }
    }
    /// Resolve only the requested target. Single pins retain their historical
    /// behavior on hosts outside the official target list; maps never fall back.
    pub fn digest_for_target(&self, target: Option<&str>) -> Result<&str> {
        self.validate_pins()?;
        if let Some(digest) = &self.sha256 {
            return Ok(digest);
        }
        let target = target
            .filter(|target| TARGETS.contains(target))
            .ok_or_else(|| invalid("portable provider pins do not support this native target"))?;
        self.sha256_by_target.as_ref().and_then(|targets| targets.get(target)).map(String::as_str)
            .ok_or_else(|| invalid("portable provider pin is missing for this native target; explicitly add its reviewed target digest"))
    }
}
