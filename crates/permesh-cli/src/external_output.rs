// SPDX-License-Identifier: MIT
use crate::output::{field, safe};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

/// Review boundary owned by the CLI, independent of workspace storage serde.
#[derive(serde::Serialize)]
pub(crate) struct TargetPinsReview<'a> {
    pub sha256_by_target: &'a BTreeMap<String, String>,
    pub resolved_target: &'a str,
    pub resolved_sha256: &'a str,
}

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
            if let Some(pins) = result["target_pins"].as_object() {
                writeln!(
                    out,
                    "\nPortable executable pins\n  Selected target: {}\n  Selected SHA-256: {}",
                    safe(field(&result["target_pins"], "resolved_target")),
                    safe(field(&result["target_pins"], "resolved_sha256"))
                )?;
                if let Some(targets) = pins.get("sha256_by_target").and_then(Value::as_object) {
                    for (target, digest) in targets {
                        writeln!(
                            out,
                            "  {}: {}",
                            safe(target),
                            safe(digest.as_str().unwrap_or_default())
                        )?;
                    }
                }
                writeln!(
                    out,
                    "  Each target requires local binary trust and workspace approval."
                )?;
            }
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
            if result["aws_profile"].is_object() {
                writeln!(out, "Temporary AWS profile")?;
                for (key, label) in [
                    ("credentials_file", "Credentials file"),
                    ("profile", "Named profile"),
                ] {
                    writeln!(
                        out,
                        "  {label}: {}",
                        safe(field(&result["aws_profile"], key))
                    )?;
                }
                writeln!(
                    out,
                    "  Delivers: access_key_id, secret_access_key, session_token\n  Resolves only after approval; no AWS credential chain or helper execution.\n"
                )?;
            }
            if let Some(resolvers) = result["credential_resolvers"].as_object() {
                writeln!(out, "Remote credential resolvers")?;
                for (name, resolver) in resolvers {
                    writeln!(out, "  {}", safe(name))?;
                    for (key, label) in [
                        ("backend", "Backend"),
                        ("origin", "HTTPS origin"),
                        ("vault", "Vault ID"),
                        ("item", "Item ID"),
                        ("mount", "KV mount"),
                        ("path", "KV path"),
                        ("field", "Exact field"),
                        ("bootstrap_reference", "Bootstrap reference"),
                    ] {
                        if let Some(value) = resolver[key].as_str() {
                            writeln!(out, "    {label}: {}", safe(value))?;
                        }
                    }
                    if let Some(version) = resolver["secret_version"].as_u64() {
                        writeln!(out, "    Secret version: {version}")?;
                    }
                    if let Some(proxy) = resolver["network"]["https_proxy"].as_str() {
                        writeln!(out, "    HTTPS proxy: {}", safe(proxy))?;
                    }
                    if let Some(entries) = resolver["network"]["no_proxy"].as_array() {
                        for entry in entries {
                            if let Some(value) = entry.as_str() {
                                writeln!(out, "    Proxy bypass: {}", safe(value))?;
                            }
                        }
                    }
                    for (key, label) in [("path", "CA file"), ("sha256", "CA SHA-256")] {
                        if let Some(value) = resolver["network"]["ca_bundle"][key].as_str() {
                            writeln!(out, "    {label}: {}", safe(value))?;
                        }
                    }
                }
                writeln!(
                    out,
                    "  Whole item or KV object is fetched; only the exact field is delivered.\n"
                )?;
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
