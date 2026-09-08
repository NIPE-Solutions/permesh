// SPDX-License-Identifier: MIT
use crate::output::{field, safe, write_path};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

type AccountKey<'a> = (&'a str, &'a str);
fn key(value: &Value) -> AccountKey<'_> {
    (field(value, "provider"), field(value, "id"))
}

pub(crate) fn write_admins(out: &mut impl Write, result: &Value, dot: &str) -> io::Result<()> {
    writeln!(out, "Privileged access")?;
    if !result["access"].is_array() {
        return writeln!(
            out,
            "  Discovery unavailable; privileged access could not be evaluated."
        );
    }
    let accounts: BTreeMap<_, _> = result["accounts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|record| (key(&record["account"]["key"]), record))
        .collect();
    let known = result["access"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let unknown = result["unknown_access"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if known.is_empty() {
        writeln!(
            out,
            "  No known privileged paths in the available observations."
        )?;
    }
    write_paths(out, known, &accounts, dot)?;
    if !unknown.is_empty() {
        writeln!(
            out,
            "\nUnknown privilege\n  These roles could not be classified; review their native permissions."
        )?;
        write_paths(out, unknown, &accounts, dot)?;
    }
    let unresolved = result["unresolved_grants"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !unresolved.is_empty() {
        writeln!(
            out,
            "\nGrants without an observed account path\n  Missing membership observations do not prove a grant is unused."
        )?;
        for record in unresolved {
            let grant = &record["grant"];
            writeln!(
                out,
                "\n{} {dot} {}",
                safe(field(&grant["resource"], "provider")),
                safe(field(&record["group"], "name"))
            )?;
            writeln!(
                out,
                "  {}\n    {} ({})\n    certainty: {}\n    observed via {}",
                safe(field(&record["resource"], "name")),
                safe(field(grant, "role")),
                safe(field(grant, "privilege")),
                safe(field(grant, "certainty")),
                safe(field(&grant["provenance"], "method"))
            )?;
        }
    }
    let privileged_accounts: BTreeSet<_> = known.iter().map(|path| key(&path["account"])).collect();
    writeln!(
        out,
        "\nSummary\n  {} accounts with known privileged paths\n  {} known privileged paths\n  {} paths with unknown privilege\n  {} grants without an observed account path",
        privileged_accounts.len(),
        known.len(),
        unknown.len(),
        unresolved.len()
    )
}
fn write_paths(
    out: &mut impl Write,
    paths: &[Value],
    accounts: &BTreeMap<AccountKey<'_>, &Value>,
    dot: &str,
) -> io::Result<()> {
    let mut current = None;
    for path in paths {
        let account_key = key(&path["account"]);
        if current != Some(account_key) {
            current = Some(account_key);
            let record = accounts.get(&account_key).copied().unwrap_or(&Value::Null);
            writeln!(
                out,
                "\n{} {dot} {}",
                safe(account_key.0),
                safe(field(&record["account"], "login"))
            )?;
            match field(&record["identity"], "state") {
                "resolved" => writeln!(
                    out,
                    "  Identity: {}",
                    safe(field(&record["identity"]["identity"], "id"))
                )?,
                "ambiguous" => {
                    let candidates = record["identity"]["candidates"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(safe)
                        .collect::<Vec<_>>();
                    writeln!(out, "  Identity: ambiguous ({})", candidates.join(", "))?;
                }
                _ => writeln!(out, "  Identity: unmapped")?,
            }
        }
        write_path(out, path)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_and_unresolved_sections_preserve_uncertainty_and_escape_controls() {
        let result = json!({
            "accounts": [{"account": {"key":{"provider":"tenant\u{1b}","id":"1"},"login":"build\u{202e}"}, "identity":{"state":"ambiguous","candidates":["one@example.com","two\n@example.com"]}}],
            "access": [],
            "unknown_access": [{"account":{"provider":"tenant\u{1b}","id":"1"},"groups":[{"name":"team\nname"}],"resource":{"name":"repo\u{1b}[2J"},"grant":{"role":"custom-owner","privilege":"unknown","certainty":"observed","provenance":{"method":"API"}}}],
            "unresolved_grants": [{"group":{"name":"unseen\u{202e}"},"resource":{"name":"other\nrepo"},"grant":{"resource":{"provider":"tenant\u{1b}","id":"r"},"role":"Admin","privilege":"admin","certainty":"inferred","provenance":{"method":"API\u{1b}"}}}]
        });
        let mut bytes = Vec::new();
        write_admins(&mut bytes, &result, "/").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("Unknown privilege"));
        assert!(text.contains("custom-owner (unknown)"));
        assert!(text.contains("Grants without an observed account path"));
        assert!(text.contains("Missing membership observations do not prove a grant is unused."));
        assert!(text.contains("Identity: ambiguous (one@example.com, two\\n@example.com)"));
        assert!(text.contains("0 accounts with known privileged paths"));
        assert!(text.contains("1 paths with unknown privilege"));
        assert!(text.contains("1 grants without an observed account path"));
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
        assert!(text.contains("team\\nname"));
    }

    #[test]
    fn multiple_paths_do_not_inflate_the_privileged_account_count() {
        let path = json!({"account":{"provider":"demo","id":"1"},"groups":[],"resource":{"name":"repo"},"grant":{"role":"Admin","privilege":"admin","certainty":"observed","provenance":{"method":"API"}}});
        let result = json!({"accounts":[{"account":{"key":{"provider":"demo","id":"1"},"login":"alice"},"identity":{"state":"unmapped"}}],"access":[path.clone(),path],"unknown_access":[],"unresolved_grants":[]});
        let mut bytes = Vec::new();
        write_admins(&mut bytes, &result, "/").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("1 accounts with known privileged paths"));
        assert!(text.contains("2 known privileged paths"));
        assert!(text.contains("Identity: unmapped"));
    }
}
