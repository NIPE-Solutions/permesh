// SPDX-License-Identifier: MIT
//! Nonsecret, allowlisted configured tenant selectors. Unknown configuration is not rendered.
use permesh_config::ProviderConfig;
use std::collections::BTreeMap;
pub(crate) fn configured(provider: &ProviderConfig) -> BTreeMap<String, Vec<String>> {
    let Some(external) = &provider.external else {
        return BTreeMap::new();
    };
    let fields: &[&str] = match external.provider.as_str() {
        "github" => &["organizations"],
        "google" => &["customer_id"],
        "cloudflare" => &["account_id"],
        "aws" => &["account_id", "region"],
        _ => &[],
    };
    let mut result = BTreeMap::new();
    for field in fields {
        let Some(value) = external.configuration.get(*field) else {
            continue;
        };
        let values: Vec<&str> = if let Some(v) = value.as_str() {
            vec![v]
        } else if let Some(v) = value.as_array() {
            v.iter().filter_map(|s| s.as_str()).collect()
        } else {
            vec![]
        };
        if !values.is_empty()
            && values.len() <= 128
            && values.iter().all(|v| {
                !v.is_empty()
                    && v.len() <= 256
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
        {
            let mut values: Vec<_> = values.into_iter().map(str::to_owned).collect();
            values.sort();
            values.dedup();
            result.insert((*field).to_owned(), values);
        }
    }
    result
}
