// SPDX-License-Identifier: MIT
//! Check report version 1 (experimental). `summary.md` carries counts only.
use permesh_config::FindingKind;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const ALL_KINDS: [FindingKind; 7] = [
    FindingKind::DepartedWithAccess,
    FindingKind::NewAdmin,
    FindingKind::RemovedAdmin,
    FindingKind::NoLongerObserved,
    FindingKind::PolicyFinding,
    FindingKind::ProviderFailed,
    FindingKind::Incomplete,
];
/// Kinds that ask a person to review something; they make the check exit 6.
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
    pub notifications: Vec<Delivery>,
    pub run_url: Option<String>,
    pub files: Files,
}

impl CheckReport {
    pub fn count(&self, kind: FindingKind) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }
    pub fn review_items(&self) -> usize {
        REVIEW_KINDS.iter().map(|k| self.count(*k)).sum()
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
        FindingKind::ProviderFailed => "Providers that failed",
        FindingKind::Incomplete => "Incomplete collection",
    }
}

fn changes_line(changes: &Changes) -> &'static str {
    match changes {
        Changes::NoBaseline => "no previous run to compare with",
        Changes::Inconclusive { .. } => "inconclusive; the previous run is not comparable",
        Changes::Compared { .. } => "compared with the previous run",
    }
}

/// Counts-only Markdown, safe to post where access details must not appear.
pub fn summary_markdown(report: &CheckReport) -> String {
    let mut out = format!("# Permesh check\n\nStatus: {}\n\n", report.status());
    for kind in ALL_KINDS {
        if kind == FindingKind::Incomplete {
            continue;
        }
        out.push_str(&format!("- {}: {}\n", label(kind), report.count(kind)));
    }
    out.push_str(&format!(
        "\nCollection complete: {}\n",
        if report.complete { "yes" } else { "no" }
    ));
    if !report.departed_assessed {
        out.push_str("Departures not assessed: no authoritative identity source.\n");
    }
    out.push_str(&format!("Changes: {}\n", changes_line(&report.changes)));
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
        let key = serde_json::to_value(kind).unwrap_or_default();
        let count = result["counts"][key.as_str().unwrap_or_default()]
            .as_u64()
            .unwrap_or(0);
        if kind != FindingKind::Incomplete {
            writeln!(out, "  {}: {count}", label(kind))?;
        }
    }
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
