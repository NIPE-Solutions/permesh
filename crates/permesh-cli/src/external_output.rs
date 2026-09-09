// SPDX-License-Identifier: MIT
use crate::output::{field, safe};
use serde_json::Value;
use std::io::{self, Write};
pub fn write(out: &mut impl Write, command: &str, result: &Value) -> io::Result<()> {
    if let Some(storage) = result["storage"].as_str() {
        writeln!(out, "Local provider storage\n  {}\n", safe(storage))?;
    }
    match command {
        "external_review" => {
            writeln!(
                out,
                "Workspace\n  {}\n\nInstance\n  {}\n",
                safe(field(result, "workspace")),
                safe(field(result, "instance"))
            )?;
            registration(out, &result["registration"])?;
            if result["discovery_protocol"] == "negotiated_v1" {
                writeln!(
                    out,
                    "\nDiscovery protocol\n  negotiated_v1 (negotiated protocol 1; health and discovery)\n"
                )?;
            }
            writeln!(
                out,
                "\nApproval\n  {}\n  Fingerprint: {}\n",
                if result["approved"] == true {
                    "Current"
                } else {
                    "Required"
                },
                safe(field(result, "fingerprint"))
            )?;
            for (key, title) in [
                ("configuration", "Provider configuration"),
                ("credential_references", "Credential references"),
                ("identity", "Identity configuration"),
            ] {
                writeln!(out, "{title}")?;
                let formatted =
                    serde_json::to_string_pretty(&result[key]).map_err(io::Error::other)?;
                for line in formatted.lines() {
                    writeln!(out, "  {}", safe(line))?;
                }
                writeln!(out)?;
            }
            if !result["network"].is_null() {
                writeln!(out, "Network settings")?;
                let formatted =
                    serde_json::to_string_pretty(&result["network"]).map_err(io::Error::other)?;
                for line in formatted.lines() {
                    writeln!(out, "  {}", safe(line))?;
                }
                writeln!(out)?;
            }
            writeln!(
                out,
                "To approve after review (use the same --config FILE if supplied):\n  permesh provider external approve {} --fingerprint {} --accept-risk\n",
                safe(field(result, "instance")),
                safe(field(result, "fingerprint"))
            )?;
        }
        "external_approve" => {
            writeln!(
                out,
                "Workspace\n  {}\n\nApproved instance\n  {}\n",
                safe(field(&result["approval"], "workspace")),
                safe(field(&result["approval"], "instance"))
            )?;
        }
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
