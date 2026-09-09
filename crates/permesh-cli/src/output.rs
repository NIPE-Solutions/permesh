// SPDX-License-Identifier: MIT
use crate::{args::Color, error::AppError, report::Report};
use std::io::{self, IsTerminal, Write};
/// Escape provider-controlled terminal characters, including bidi controls.
pub(crate) fn safe(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
pub(crate) fn field<'a>(value: &'a serde_json::Value, name: &str) -> &'a str {
    value[name].as_str().unwrap_or("")
}
pub fn write_report(report: &Report, json: bool, color: Color, verbose: u8) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, report)?;
        return writeln!(out);
    }
    let colors = !matches!(color, Color::Never)
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").ok().as_deref() != Some("dumb")
        && (matches!(color, Color::Always) || io::stdout().is_terminal());
    if colors {
        writeln!(out, "\x1b[1mPermesh\x1b[0m\n")?;
    } else {
        writeln!(out, "Permesh\n")?;
    }
    let dot = if std::env::var("TERM").ok().as_deref() == Some("dumb") {
        "/"
    } else {
        "·"
    };
    let result = &report.result;
    if report.command == "user" {
        let name = if result["identity"].is_object() {
            field(&result["identity"], "id")
        } else {
            "Provider account lookup"
        };
        writeln!(out, "Identity\n  {}", safe(name))?;
        if result["identity"].is_object() {
            write_classification(&mut out, "Identity", &result["identity"])?;
        }
        if let Some(accounts) = result["accounts"].as_array() {
            for a in accounts {
                writeln!(
                    out,
                    "  {} {dot} {}",
                    safe(field(&a["key"], "provider")),
                    safe(field(a, "login"))
                )?;
                write_classification(&mut out, "Account", a)?;
            }
        }
        writeln!(out, "\nAccess evidence")?;
        let mut count = 0;
        if let Some(access) = result["access"].as_array() {
            let mut current = String::new();
            for path in access {
                let provider = field(&path["account"], "provider");
                if current != provider {
                    writeln!(out, "\n{}", safe(provider))?;
                    current = provider.into();
                }
                write_path(&mut out, path)?;
                count += 1;
            }
        }
        writeln!(out, "\nSummary\n  {count} access evidence paths")?;
    } else if report.command == "provider_development" {
        crate::provider_development::write(&mut out, result)?;
    } else if report.command == "admins" {
        crate::admins_output::write_admins(&mut out, result, dot)?;
    } else if report.command == "orphaned" {
        crate::orphaned_output::write_orphaned(&mut out, result, dot)?;
    } else if report.command == "provider_list" {
        writeln!(out, "Providers")?;
        if let Some(providers) = result["providers"].as_array() {
            for p in providers {
                writeln!(
                    out,
                    "  {} {dot} {}",
                    safe(field(p, "id")),
                    safe(field(p, "type"))
                )?;
            }
        }
    } else if report.command == "provider_capabilities" {
        writeln!(out, "{}\n\nDiscovery", safe(field(result, "id")))?;
        if let Some(capabilities) = result["metadata"]["capabilities"].as_array() {
            for c in capabilities {
                writeln!(out, "  Supported: {}", safe(c.as_str().unwrap_or_default()))?;
            }
        }
        writeln!(out, "  Unsupported: sessions, credentials, mutations")?;
    } else if matches!(
        report.command.as_str(),
        "provider_install" | "provider_update"
    ) {
        crate::distribution_output::write(&mut out, result)?;
    } else if report.command.starts_with("external_") {
        crate::external_output::write(&mut out, &report.command, result)?;
    } else if report.command == "provider_setup_describe" {
        crate::setup_output::write(&mut out, result)?;
    } else if report.command == "version" {
        writeln!(
            out,
            "{}\nNo backend. No telemetry.",
            safe(field(result, "version"))
        )?;
    }
    if let Some(message) = result["message"].as_str() {
        writeln!(out, "{}", safe(message))?;
    }
    if let Some(file) = result["file"].as_str() {
        writeln!(out, "  {}", safe(file))?;
    }
    if let Some(next) = result["next"].as_str() {
        writeln!(out, "\nNext\n  {}", safe(next))?;
    }
    if !report.providers.is_empty() {
        writeln!(out, "\nProviders")?;
        for p in &report.providers {
            writeln!(
                out,
                "  {} {dot} {}: {}\n    {}",
                safe(&p.kind),
                safe(&p.id),
                safe(&p.state),
                safe(&p.message)
            )?;
            for limitation in &p.limitations {
                writeln!(out, "    Note: {}", safe(limitation))?;
            }
        }
    }
    if let Some(rows) = result["diagnostics"].as_array() {
        writeln!(out, "\nDiagnostic details")?;
        for row in rows {
            writeln!(
                out,
                "  {} [{} / {}]\n    {}\n    Next: {}",
                safe(field(row, "instance")),
                safe(field(row, "stage")),
                safe(field(row, "code")),
                safe(field(row, "message")),
                safe(field(row, "next"))
            )?;
        }
    }
    if !report.complete {
        writeln!(
            out,
            "\nWarning\n  Results are incomplete. Unavailable observations cannot prove absence of access."
        )?;
    }
    if verbose > 0 {
        writeln!(
            out,
            "\nObservation window (UTC)\n  {} to {}\n  Provider API reads are not transactional.",
            report.started_at, report.completed_at
        )?;
    }
    Ok(())
}
pub fn write_error(error: &AppError, json: bool, schema_version: u32) -> io::Result<()> {
    if json {
        let mut out = io::stdout().lock();
        serde_json::to_writer(
            &mut out,
            &serde_json::json!({"schema_version":schema_version,"error":error}),
        )?;
        writeln!(out)
    } else {
        writeln!(
            io::stderr().lock(),
            "Permesh\n\nError\n  {}",
            safe(&error.message)
        )
    }
}
/// Shared path presentation for user and privileged-access reports.
pub(crate) fn write_path(out: &mut impl Write, path: &serde_json::Value) -> io::Result<()> {
    writeln!(
        out,
        "  {}\n    {} ({})",
        safe(field(&path["resource"], "name")),
        safe(field(&path["grant"], "role")),
        safe(field(&path["grant"], "privilege"))
    )?;
    let groups = path["groups"]
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .map(|g| safe(field(g, "name")))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if groups.is_empty() {
        writeln!(
            out,
            "    via {}",
            safe(field(&path["grant"]["provenance"], "method"))
        )?;
    } else {
        writeln!(out, "    via {}", groups.join(" -> "))?;
    }
    write_grant_evidence(out, &path["grant"])?;
    writeln!(
        out,
        "    path certainty: {}",
        safe(field(path, "certainty"))
    )?;
    Ok(())
}

/// Keep independent classifications visible in every account-oriented report.
pub(crate) fn write_classification(
    out: &mut impl Write,
    label: &str,
    value: &serde_json::Value,
) -> io::Result<()> {
    writeln!(
        out,
        "  {label} classification: {} / {} / {}",
        safe(field(value, "kind")),
        safe(field(value, "status")),
        safe(field(value, "affiliation"))
    )
}
pub(crate) fn write_grant_evidence(
    out: &mut impl Write,
    grant: &serde_json::Value,
) -> io::Result<()> {
    writeln!(
        out,
        "    evidence: {}\n    grant certainty: {}",
        safe(field(grant, "evidence_kind")),
        safe(field(grant, "certainty"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_terminal_text_is_escaped() {
        assert_eq!(safe("a\x1b[2J\n"), "a\\u{1b}[2J\\n");
        assert!(!safe("a\u{202e}txt").contains('\u{202e}'));
    }
}
