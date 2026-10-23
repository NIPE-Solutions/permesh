// SPDX-License-Identifier: MIT
//! Check report version 1 (stable, ADR 0036). `summary.md` carries counts only.
use permesh_config::FindingKind;
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const ALL_KINDS: [FindingKind; 9] = [
    FindingKind::DepartedWithAccess,
    FindingKind::NewAdmin,
    FindingKind::RemovedAdmin,
    FindingKind::NoLongerObserved,
    FindingKind::PolicyFinding,
    FindingKind::UnexplainedAccess,
    FindingKind::MissingAccess,
    FindingKind::ProviderFailed,
    FindingKind::Incomplete,
];
/// Kinds that only exist when a run was compared with a baseline. They are absent
/// (shown as "not compared"), never zero, when no comparison happened.
pub const DIFF_KINDS: [FindingKind; 3] = [
    FindingKind::NewAdmin,
    FindingKind::RemovedAdmin,
    FindingKind::NoLongerObserved,
];
/// Kinds that only exist when profiles were compared; report-only unless enforced.
pub const PROFILE_KINDS: [FindingKind; 2] =
    [FindingKind::UnexplainedAccess, FindingKind::MissingAccess];
/// Kinds that ask a person to review something; they make the check exit 6.
/// Profile kinds join them only when `profiles.enforce` is set.
pub const REVIEW_KINDS: [FindingKind; 4] = [
    FindingKind::DepartedWithAccess,
    FindingKind::NewAdmin,
    FindingKind::NoLongerObserved,
    FindingKind::PolicyFinding,
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Changes {
    NoBaseline,
    Inconclusive {
        reasons: Vec<String>,
    },
    Compared {
        new_admins: Vec<Value>,
        removed_admins: Vec<Value>,
        no_longer_observed: Vec<Value>,
    },
}

/// Profile comparison state: `"not_configured"` or the `profile check` result.
#[derive(Clone, Debug)]
pub enum Profiles {
    NotConfigured,
    Compared {
        enforce: bool,
        complete: bool,
        value: Value,
    },
}
impl Serialize for Profiles {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::NotConfigured => serializer.serialize_str("not_configured"),
            Self::Compared { value, .. } => value.serialize(serializer),
        }
    }
}
impl Profiles {
    pub fn enforced(&self) -> bool {
        matches!(self, Self::Compared { enforce: true, .. })
    }
    /// Detail data: profile rows in state `finding`, one line each.
    pub fn findings(&self) -> Vec<Value> {
        let Self::Compared { value, .. } = self else {
            return vec![];
        };
        let mut rows = Vec::new();
        for identity in value["identities"].as_array().into_iter().flatten() {
            let id = &identity["identity"];
            let findings = |key: &str| {
                identity[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|r| r["state"] == "finding")
                    .cloned()
                    .collect::<Vec<_>>()
            };
            for row in findings("missing") {
                let entry = &row["entry"];
                let mut item = json!({"identity": id, "kind": "missing_access", "instance": entry["instance"], "via": []});
                for key in ["group", "resource", "role"] {
                    if entry[key].is_string() {
                        item[key] = entry[key].clone();
                    }
                }
                rows.push(item);
            }
            for row in findings("unexplained") {
                rows.push(json!({"identity": id, "kind": "unexplained_access", "instance": row["instance"], "resource": row["resource"], "role": row["role"], "via": row["via"]}));
            }
        }
        rows
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderLine {
    pub id: String,
    pub state: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Delivery {
    pub id: String,
    pub delivered: bool,
    pub skipped: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Files {
    pub snapshot: String,
    pub report: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub check_report_version: u32,
    pub complete: bool,
    pub departed_assessed: bool,
    pub counts: BTreeMap<FindingKind, usize>,
    pub providers: Vec<ProviderLine>,
    pub changes: Changes,
    /// Detail data: departed accounts with their access paths.
    pub departed: Vec<Value>,
    /// Detail data: policy checks in state `finding`, other than `inactive_access`.
    pub findings: Vec<Value>,
    pub policy: Value,
    pub profiles: Profiles,
    pub notifications: Vec<Delivery>,
    pub run_url: Option<String>,
    pub files: Files,
}

impl Changes {
    /// Non-identifying comparison state, safe for counts-only messages.
    pub fn state(&self) -> Value {
        match self {
            Self::NoBaseline => serde_json::json!({"state": "no_baseline", "reasons": []}),
            Self::Inconclusive { reasons } => {
                serde_json::json!({"state": "inconclusive", "reasons": reasons})
            }
            Self::Compared { .. } => serde_json::json!({"state": "compared", "reasons": []}),
        }
    }
}

impl CheckReport {
    pub fn count(&self, kind: FindingKind) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }
    pub fn review_items(&self) -> usize {
        let profiles = if self.profiles.enforced() {
            PROFILE_KINDS.iter().map(|k| self.count(*k)).sum()
        } else {
            0
        };
        REVIEW_KINDS.iter().map(|k| self.count(*k)).sum::<usize>() + profiles
    }
    pub fn status(&self) -> &'static str {
        if !self.complete {
            "incomplete"
        } else if self.review_items() > 0 {
            "findings"
        } else {
            "clean"
        }
    }
}

pub fn label(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::DepartedWithAccess => "Departed people with access",
        FindingKind::NewAdmin => "New admins",
        FindingKind::RemovedAdmin => "Removed admins",
        FindingKind::NoLongerObserved => "No longer observed (needs review)",
        FindingKind::PolicyFinding => "Policy findings",
        FindingKind::UnexplainedAccess => "Unexplained access",
        FindingKind::MissingAccess => "Missing expected access",
        FindingKind::ProviderFailed => "Providers that failed",
        FindingKind::Incomplete => "Incomplete collection",
    }
}

/// Report-only profile kinds say so wherever counts are shown.
fn kind_label(kind: FindingKind, report_only: bool) -> String {
    if PROFILE_KINDS.contains(&kind) && report_only {
        format!("{} (report-only)", label(kind))
    } else {
        label(kind).to_owned()
    }
}

fn changes_line(state: &Value) -> String {
    match state["state"].as_str() {
        Some("compared") => "compared with the previous run".into(),
        Some("inconclusive") => {
            let reasons: Vec<_> = state["reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            format!(
                "inconclusive; the previous run is not comparable ({})",
                reasons.join(", ")
            )
        }
        _ => "no previous run to compare with".into(),
    }
}
fn count_text(counts: &Value, kind: FindingKind) -> String {
    let key = serde_json::to_value(kind).unwrap_or_default();
    match counts
        .get(key.as_str().unwrap_or_default())
        .and_then(Value::as_u64)
    {
        Some(count) => count.to_string(),
        None if DIFF_KINDS.contains(&kind) || PROFILE_KINDS.contains(&kind) => {
            "not compared".into()
        }
        None => "0".into(),
    }
}

/// Counts-only Markdown, safe to post where access details must not appear.
pub fn summary_markdown(report: &CheckReport) -> String {
    let mut out = format!("# Permesh check\n\nStatus: {}\n\n", report.status());
    for kind in ALL_KINDS {
        if kind == FindingKind::Incomplete {
            continue;
        }
        let counts = serde_json::to_value(&report.counts).unwrap_or_default();
        out.push_str(&format!(
            "- {}: {}\n",
            kind_label(
                kind,
                matches!(report.profiles, Profiles::Compared { enforce: false, .. })
            ),
            count_text(&counts, kind)
        ));
    }
    out.push_str(&format!(
        "\nCollection complete: {}\n",
        if report.complete { "yes" } else { "no" }
    ));
    if !report.departed_assessed {
        out.push_str("Departures not assessed: no authoritative identity source.\n");
    }
    out.push_str(&format!(
        "Changes: {}\n",
        changes_line(&report.changes.state())
    ));
    if let Some(url) = &report.run_url {
        out.push_str(&format!("Run: {url}\n"));
    }
    out
}

pub fn write(out: &mut dyn std::io::Write, result: &Value) -> std::io::Result<()> {
    use crate::output::safe;
    writeln!(
        out,
        "Check\n  Status: {}",
        safe(result["status"].as_str().unwrap_or_default())
    )?;
    for kind in ALL_KINDS {
        if kind != FindingKind::Incomplete {
            writeln!(
                out,
                "  {}: {}",
                kind_label(kind, result["profiles"]["enforce"] == false),
                count_text(&result["counts"], kind)
            )?;
        }
    }
    writeln!(
        out,
        "  Changes: {}",
        safe(&changes_line(&result["changes_state"]))
    )?;
    if result["departed_assessed"] == false {
        writeln!(
            out,
            "  Departures not assessed: no authoritative identity source"
        )?;
    }
    if let Some(deliveries) = result["notifications"].as_array()
        && !deliveries.is_empty()
    {
        writeln!(out, "\nNotifications")?;
        for d in deliveries {
            let state = if d["delivered"] == true {
                "delivered".to_owned()
            } else if d["skipped"] == true {
                "skipped".to_owned()
            } else {
                format!("failed: {}", d["reason"].as_str().unwrap_or("unknown"))
            };
            writeln!(
                out,
                "  {}: {}",
                safe(d["id"].as_str().unwrap_or_default()),
                safe(&state)
            )?;
        }
    }
    writeln!(out, "\nFiles")?;
    for key in ["snapshot", "report", "summary"] {
        writeln!(
            out,
            "  {}",
            safe(result["files"][key].as_str().unwrap_or_default())
        )?;
    }
    writeln!(
        out,
        "\nThe snapshot and report contain sensitive access metadata; summary.md has counts only."
    )
}
