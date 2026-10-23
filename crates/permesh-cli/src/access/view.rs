// SPDX-License-Identifier: MIT
//! Self-contained HTML of the same validated plan returned as JSON. Every value is
//! escaped; the page loads nothing (CSP `default-src 'none'`).
use super::model::{AccessPlan, invalid};
use crate::error::AppError;
use serde_json::Value;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
pub(crate) fn page(title: &str, body: &str, document: &Value) -> Result<Vec<u8>, AppError> {
    let json = serde_json::to_string_pretty(document).map_err(|_| invalid())?;
    Ok(format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;max-width:80rem;margin:2rem auto;padding:1rem}}pre,td{{overflow-wrap:anywhere}}pre{{white-space:pre-wrap}}table{{border-collapse:collapse;width:100%;margin-bottom:2rem}}th,td{{text-align:left;border-bottom:1px solid #aaa;padding:.6rem;vertical-align:top}}.gap{{background:#fff0cc;padding:1rem}}</style>{body}<h2>Document</h2><pre>{}</pre></html>",
        escape(title),
        escape(&json)
    )
    .into_bytes())
}
pub(crate) fn plan_html(plan: &AccessPlan) -> Result<Vec<u8>, AppError> {
    plan.validate()?;
    let value = serde_json::to_value(plan).map_err(|_| invalid())?;
    let mut body = format!(
        "<h1>Access plan: {} for {}</h1><p>Created {}; fresh until {}.</p><p class=\"gap\">{} Plans are data: a person carries out each step in the provider. Verify afterwards with permesh access verify.</p><h2>Steps</h2><table><tr><th>Step</th><th>Justification</th><th>State</th><th>Prerequisites</th></tr>",
        escape(value["kind"].as_str().unwrap_or_default()),
        escape(&plan.identity),
        escape(&plan.created_at),
        escape(&plan.expires_at),
        if plan.complete {
            "Captured scope planned."
        } else {
            "INCOMPLETE — some access could not be observed."
        }
    );
    for step in value["steps"].as_array().into_iter().flatten() {
        body.push_str(&row(&[
            escape(&super::words(&step["operation"])),
            escape(&super::justification(&step["justification"])),
            escape(&format!(
                "{} ({})",
                step["state"].as_str().unwrap_or_default(),
                step["reason"].as_str().unwrap_or_default()
            )),
            step["prerequisites"]
                .as_array()
                .map_or(0, Vec::len)
                .to_string(),
        ]));
    }
    body.push_str("</table><h2>Kept</h2><table><tr><th>Entry</th><th>Because of</th></tr>");
    for kept in &plan.kept {
        body.push_str(&row(&[
            escape(&kept["entry"].to_string()),
            escape(&kept["because"].to_string()),
        ]));
    }
    body.push_str("</table>");
    page("Access plan", &body, &value)
}
