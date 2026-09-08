// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::output::{field, safe, write_path};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

const SECTIONS: &[(&str, &str)] = &[
    ("unassessed", "Unassessed accounts"),
    ("inactive_identity", "Inactive identities"),
    ("ambiguous_identity", "Ambiguous identities"),
    ("unknown_identity", "Unmatched accounts"),
    ("unknown_status", "Unknown identity status"),
    ("external_identity", "External identities"),
    ("service_account", "Service accounts"),
    ("bot", "Bots"),
];
fn key(value: &Value) -> (&str, &str) {
    (field(value, "provider"), field(value, "id"))
}
pub(crate) fn write_orphaned(out: &mut impl Write, result: &Value, dot: &str) -> io::Result<()> {
    writeln!(out, "Orphaned account review")?;
    let Some(accounts) = result["accounts"].as_array() else {
        return writeln!(
            out,
            "  Discovery unavailable; accounts could not be assessed."
        );
    };
    if result["authority_complete"] != true {
        writeln!(
            out,
            "  Identity sources are incomplete. Accounts are unassessed; no orphan conclusion is available."
        )?;
    }
    let mut paths: BTreeMap<_, Vec<&Value>> = BTreeMap::new();
    for path in result["access"].as_array().into_iter().flatten() {
        paths.entry(key(&path["account"])).or_default().push(path);
    }
    let mut review = 0;
    let mut separate = 0;
    let mut unassessed = 0;
    for (reason, heading) in SECTIONS {
        let selected: Vec<_> = accounts
            .iter()
            .filter(|a| field(a, "reason") == *reason)
            .collect();
        if selected.is_empty() {
            continue;
        }
        writeln!(out, "\n{heading}")?;
        match *reason {
            "unassessed" => unassessed += selected.len(),
            "service_account" | "bot" | "external_identity" => {
                separate += selected.len();
                writeln!(
                    out,
                    "  Listed separately; this classification does not establish approved access."
                )?;
            }
            _ => review += selected.len(),
        }
        for record in selected {
            let account = &record["account"];
            writeln!(
                out,
                "\n{} {dot} {}",
                safe(field(&account["key"], "provider")),
                safe(field(account, "login"))
            )?;
            let identity = &record["identity"];
            match field(identity, "state") {
                "resolved" => {
                    let person = &identity["identity"];
                    let label = person["verified_emails"]
                        .as_array()
                        .and_then(|v| v.first())
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| field(person, "id"));
                    writeln!(
                        out,
                        "  Identity: {} ({})",
                        safe(label),
                        safe(field(person, "status"))
                    )?;
                }
                "ambiguous" => {
                    let labels = identity["candidates"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(safe)
                        .collect::<Vec<_>>();
                    writeln!(out, "  Identity: ambiguous ({})", labels.join(", "))?;
                }
                _ => writeln!(out, "  Identity: no confident match")?,
            }
            if let Some(observed) = paths.get(&key(&account["key"])) {
                for path in observed {
                    write_path(out, path)?;
                }
            } else {
                writeln!(
                    out,
                    "  No access paths observed; this is not proof of no access."
                )?;
            }
        }
    }
    writeln!(
        out,
        "\nSummary\n  Accounts needing identity review: {review}\n  Service, bot or external accounts: {separate}\n  Unassessed accounts: {unassessed}"
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn terminal_controls_are_escaped_and_no_paths_do_not_imply_no_access() {
        let result = serde_json::json!({"authority_complete":true,"accounts":[{"account":{"key":{"provider":"source\u{1b}","id":"1"},"login":"account\n"},"identity":{"state":"ambiguous","candidates":["identity\u{202e}"]},"reason":"ambiguous_identity"}],"access":[]});
        let mut bytes = Vec::new();
        write_orphaned(&mut bytes, &result, "/").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
        assert!(text.contains("No access paths observed; this is not proof of no access."));
        assert!(text.contains("Accounts needing identity review: 1"));
    }
}
