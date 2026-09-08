// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::output::{field, safe};
use serde_json::Value;
use std::io::{self, Write};
pub fn write(out: &mut impl Write, command: &str, result: &Value) -> io::Result<()> {
    if let Some(storage) = result["storage"].as_str() {
        writeln!(out, "Local provider storage\n  {}\n", safe(storage))?;
    }
    match command {
        "external_inspect" => {
            writeln!(
                out,
                "Native executable\n  SHA-256: {}\n  Bytes: {}\n",
                safe(field(&result["inspection"], "sha256")),
                result["inspection"]["size"]
            )?;
        }
        "external_trust" => registration(out, &result["registration"])?,
        "external_list" => {
            writeln!(out, "Trusted native providers")?;
            if let Some(entries) = result["registrations"].as_array() {
                if entries.is_empty() {
                    writeln!(out, "  None registered")?;
                }
                for item in entries {
                    registration(out, item)?;
                }
            }
        }
        "external_discover" => {
            writeln!(out, "Observed snapshot")?;
            for kind in [
                "identities",
                "accounts",
                "resources",
                "groups",
                "memberships",
                "grants",
            ] {
                let count = result["snapshot"][kind].as_array().map_or(0, Vec::len);
                writeln!(out, "  {kind}: {count}")?;
            }
            writeln!(out, "\nUse --json for normalized records and provenance.")?;
        }
        _ => {}
    }
    Ok(())
}
fn registration(out: &mut impl Write, item: &Value) -> io::Result<()> {
    writeln!(
        out,
        "  {}\n    SHA-256: {}",
        safe(field(item, "id")),
        safe(field(item, "sha256"))
    )?;
    if let Some(caps) = item["capabilities"].as_array() {
        let labels: Vec<_> = caps.iter().filter_map(Value::as_str).map(safe).collect();
        writeln!(out, "    Capabilities: {}", labels.join(", "))?;
    }
    Ok(())
}
