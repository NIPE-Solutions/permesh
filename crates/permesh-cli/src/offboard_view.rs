// SPDX-License-Identifier: MIT
//! Local self-contained rendering of the same validated model returned as JSON.
use super::model::{Report, invalid};
use crate::error::AppError;
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn label<T: serde::Serialize>(value: &T) -> Result<String, AppError> {
    Ok(escape(
        serde_json::to_value(value)
            .map_err(|_| invalid())?
            .as_str()
            .ok_or_else(invalid)?,
    ))
}
fn row(values: &[String]) -> String {
    format!(
        "<tr>{}</tr>",
        values
            .iter()
            .map(|s| format!("<td>{s}</td>"))
            .collect::<String>()
    )
}
pub fn html(report: &Report) -> Result<Vec<u8>, AppError> {
    report.validate()?;
    let json = serde_json::to_string_pretty(report).map_err(|_| invalid())?;
    let mut out = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>Departure review</title><style>body{{font:16px system-ui;max-width:80rem;margin:2rem auto;padding:1rem}}pre,td{{overflow-wrap:anywhere}}pre{{white-space:pre-wrap}}table{{border-collapse:collapse;width:100%;margin-bottom:2rem}}th,td{{text-align:left;border-bottom:1px solid #aaa;padding:.6rem;vertical-align:top}}.gap{{background:#fff0cc;padding:1rem}}small{{color:#444}}</style><h1>Departure review: {}</h1><p>Report generated {}. Evidence mode: {}. Assessment source: {}.</p><p class=\"gap\">{} Advisory only; absence does not prove denied access. Unsupported and manual checks require follow-up.</p>",
        escape(&report.assessment.target.id),
        escape(&report.generated_at),
        label(&report.mode)?,
        label(&report.assessment_origin)?,
        if report.complete {
            "Recorded scope reviewed."
        } else {
            "INCOMPLETE — unresolved evidence gaps remain."
        }
    );
    out.push_str("<h2>Coverage gaps and unsupported checks</h2><ul>");
    for gap in &report.gaps {
        out.push_str(&format!(
            "<li>{}: {}</li>",
            escape(gap.provider.as_deref().unwrap_or("Overall")),
            label(&gap.reason)?
        ));
    }
    for check in &report.assessment.unsupported_checks {
        out.push_str(&format!("<li>Unsupported: {}</li>", label(check)?));
    }
    out.push_str("</ul><h2>Connected scope and collection times</h2><table><tr><th>Provider</th><th>Collection</th><th>Scope and limitations</th></tr>");
    for s in if report.verification_sources.is_empty() {
        &report.assessment.sources
    } else {
        &report.verification_sources
    } {
        out.push_str(&row(&[
            escape(&s.capture.instance),
            format!(
                "{}<br>{} → {}",
                label(&s.capture.state)?,
                escape(&s.capture.started_at),
                escape(&s.capture.completed_at)
            ),
            escape(
                &serde_json::to_string(&(
                    &s.capture.configured_scope,
                    &s.capture.source_observation,
                    &s.capture.limitations,
                ))
                .map_err(|_| invalid())?,
            ),
        ]));
    }
    out.push_str("</table><h2>Exact associated accounts</h2><table><tr><th>Stable account</th><th>Label</th><th>Kind / lifecycle</th></tr>");
    for account in &report.assessment.accounts {
        out.push_str(&row(&[
            format!(
                "{} / {}",
                escape(&account.key.provider),
                escape(&account.key.id)
            ),
            escape(&account.login),
            format!("{} / {}", label(&account.kind)?, label(&account.status)?),
        ]));
    }
    out.push_str("</table><h2>Observed assignment paths</h2><p>Use your browser’s Find command to inspect resource names and IDs. Derived paths preserve the grant’s original evidence; they are not an effective authorization decision.</p><table><tr><th>Account and path</th><th>Resource</th><th>Native role / evidence</th></tr>");
    for p in &report.assessment.paths {
        let mut path = vec![escape(&p.account.id)];
        path.extend(p.groups.iter().map(|g| escape(&g.name)));
        out.push_str(&row(&[
            path.join(" → "),
            format!(
                "{}<br><small>{} / {}</small>",
                escape(&p.resource.name),
                escape(&p.resource.key.provider),
                escape(&p.resource.key.id)
            ),
            format!(
                "{}<br>{} / {}",
                escape(&p.grant.role),
                label(&p.grant.evidence_kind)?,
                label(&p.certainty)?
            ),
        ]));
    }
    out.push_str("</table><h2>Ownership dependencies</h2><p>Machine ownership is an explicit organizational assertion, not a provider fact. Retained plan assertions require manual revalidation.</p><ul>");
    for owner in &report.assessment.ownership {
        out.push_str(&format!(
            "<li>{} / {} ({}) — responsibility asserted for {}</li>",
            escape(&owner.account.key.provider),
            escape(&owner.account.key.id),
            escape(&owner.account.login),
            escape(&owner.identity)
        ));
    }
    for finding in &report.assessment.ownership_findings {
        out.push_str(&format!("<li>{} / {}: {} observed owner account(s), within complete visible GitHub organization membership only.</li>",escape(&finding.resource.provider),escape(&finding.resource.id),finding.observed_owner_accounts));
    }
    out.push_str("</ul><h2>Advisory review steps</h2><p>Review ownership transfer and machine dependencies before disruptive membership or lifecycle changes. These are structured review items, not executable instructions.</p><table><tr><th>Review</th><th>Stable account / resource</th><th>Reason / verification</th></tr>");
    for r in &report.assessment.recommendations {
        out.push_str(&row(&[
            format!("{}<br><small>{}</small>", label(&r.kind)?, escape(&r.id)),
            format!(
                "{} / {}<br>{}",
                escape(&r.account.provider),
                escape(&r.account.id),
                r.resource
                    .as_ref()
                    .map(|k| escape(&k.id))
                    .unwrap_or_default()
            ),
            format!(
                "{}<br>{}<br><small>{} prerequisite review(s)</small>",
                label(&r.reason)?,
                label(&r.verification)?,
                r.prerequisites.len()
            ),
        ]));
    }
    out.push_str("</table><h2>Verification checks</h2><table><tr><th>Provider / native ID</th><th>Observation result</th></tr>");
    for c in &report.checks {
        out.push_str(&row(&[
            format!("{} / {}", escape(&c.provider), escape(&c.native_id)),
            label(&c.state)?,
        ]));
    }
    out.push_str("</table><h2>Limitations</h2><ul>");
    for limitation in &report.limitations {
        out.push_str(&format!("<li>{}</li>", escape(limitation)));
    }
    out.push_str(&format!("</ul><details><summary>Full validated report and evidence identifiers</summary><pre>{}</pre></details></html>",escape(&json)));
    Ok(out.into_bytes())
}
pub fn write(out: &mut dyn std::io::Write, result: &serde_json::Value) -> std::io::Result<()> {
    writeln!(
        out,
        "Read-only departure review\n  Target: {}\n  Evidence: {}\n  Complete within recorded scope: {}",
        crate::output::safe(
            result["assessment"]["target"]["id"]
                .as_str()
                .unwrap_or_default()
        ),
        crate::output::safe(result["mode"].as_str().unwrap_or_default()),
        result["complete"]
    )?;
    for line in serde_json::to_string_pretty(result)
        .map_err(std::io::Error::other)?
        .lines()
    {
        writeln!(out, "{}", crate::output::safe(line))?;
    }
    Ok(())
}
