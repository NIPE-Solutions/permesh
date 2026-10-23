// SPDX-License-Identifier: MIT
//! `permesh check`: one unattended review run (experimental report format).
//! Collect a snapshot, evaluate policy and departures, write files, then notify.
pub mod notify;
pub mod report;
use crate::{
    args::Cli,
    artifact::{self, Artifact, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    policy_command::LoadedPolicy,
    report::Outcome,
};
use permesh_config::FindingKind;
use permesh_config::profiles::ProfileSet;
use permesh_core::{self as core, OrphanReason};
use report::{ALL_KINDS, Changes, CheckReport, Files, Profiles, ProviderLine};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use time::OffsetDateTime;

const SNAPSHOT: &str = "snapshot.json";
const REPORT: &str = "check.json";
const SUMMARY: &str = "summary.md";

#[derive(clap::Args, Clone)]
pub struct CheckArgs {
    /// Directory for snapshot.json, check.json and summary.md.
    #[arg(long)]
    pub out: PathBuf,
    /// Snapshot from an earlier run to compare against.
    #[arg(long)]
    pub previous: Option<PathBuf>,
    /// Policy file; defaults to inactive_access, external_privileged and machine_owner.
    #[arg(long)]
    pub policy: Option<PathBuf>,
    /// Replace files from an earlier run in the output directory.
    #[arg(long)]
    pub overwrite: bool,
    /// Do not send configured notifications.
    #[arg(long)]
    pub no_notify: bool,
    /// Link to the CI run, included in summaries and notifications.
    #[arg(long, value_name = "URL")]
    pub run_url: Option<String>,
}

fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}

/// Reject output locations that would mix sensitive files into the workspace root.
fn check_output(cli: &Cli, out: &Path, overwrite: bool) -> Result<(), AppError> {
    let workspace = crate::workspace::path(cli)?
        .canonicalize()
        .map_err(|_| AppError::input("Cannot locate workspace"))?;
    if let Ok(dir) = out.canonicalize()
        && Some(dir.as_path()) == workspace.parent()
    {
        return Err(AppError::input(
            "Check output must be a separate directory, not the workspace directory",
        ));
    }
    for name in [SNAPSHOT, REPORT, SUMMARY] {
        artifact::check_destination(&out.join(name), overwrite)?;
    }
    Ok(())
}

/// Departed = an authoritative identity classified inactive or suspended, or an
/// identity whose people file says `status: left`, whose account still has
/// observed or derived access paths. A left identity whose accounts cannot be
/// attributed is skipped here; the ambiguous account already makes the run
/// incomplete through policy evaluation.
fn departures(
    artifact: &Artifact,
    profiles: Option<&ProfileSet>,
) -> Result<(bool, Vec<Value>), AppError> {
    let authorities = &artifact.identity.authorities;
    let left = profiles.map(|p| &p.left).filter(|l| !l.is_empty());
    if authorities.is_empty() && left.is_none() {
        return Ok((false, vec![]));
    }
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let mut departed = Vec::new();
    let mut seen = BTreeSet::new();
    if !authorities.is_empty() {
        let orphaned = core::query_orphaned(&snapshots, &aliases, authorities)?;
        let mut paths: BTreeMap<&core::EntityKey, usize> = BTreeMap::new();
        for path in &orphaned.access {
            *paths.entry(&path.account).or_default() += 1;
        }
        for a in orphaned.accounts.iter().filter(|a| {
            matches!(
                a.reason,
                OrphanReason::InactiveIdentity | OrphanReason::SuspendedIdentity
            )
        }) {
            if let Some(count) = paths.get(&a.account.key) {
                seen.insert(a.account.key.clone());
                departed.push(json!({
                    "instance": a.account.key.provider,
                    "account": a.account.key.id,
                    "login": a.account.login,
                    "reason": a.reason,
                    "access_paths": count,
                }));
            }
        }
    }
    for identity in left.into_iter().flatten() {
        let access = match core::query_user(&snapshots, &aliases, identity) {
            Ok(access) => access,
            Err(core::DomainError::NotFound | core::DomainError::Ambiguous) => continue,
            Err(error) => return Err(error.into()),
        };
        for account in &access.accounts {
            let count = access
                .access
                .iter()
                .filter(|p| p.account == account.key)
                .count();
            if count > 0 && seen.insert(account.key.clone()) {
                departed.push(json!({
                    "instance": account.key.provider,
                    "account": account.key.id,
                    "login": account.login,
                    "reason": "left_in_people_file",
                    "access_paths": count,
                }));
            }
        }
    }
    Ok((true, departed))
}

type AdminKey = (core::EntityKey, core::EntityKey);
/// Known privileged (account, resource) pairs, including access derived through groups.
fn admins(artifact: &Artifact) -> Result<BTreeMap<AdminKey, Value>, AppError> {
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let result = core::query_admins(&snapshots, &aliases)?;
    let logins: BTreeMap<_, _> = result
        .accounts
        .iter()
        .map(|a| (&a.account.key, a.account.login.as_str()))
        .collect();
    let mut admins = BTreeMap::new();
    for path in &result.access {
        admins
            .entry((path.account.clone(), path.resource.key.clone()))
            .or_insert_with(|| {
                json!({
                    "instance": path.account.provider,
                    "account": path.account.id,
                    "login": logins.get(&path.account).copied().unwrap_or_default(),
                    "resource": path.resource.key.id,
                    "resource_name": path.resource.name,
                    "role": path.grant.role,
                })
            });
    }
    Ok(admins)
}
fn only_in(a: &BTreeMap<AdminKey, Value>, b: &BTreeMap<AdminKey, Value>) -> Vec<Value> {
    a.iter()
        .filter(|(k, _)| !b.contains_key(*k))
        .map(|(_, v)| v.clone())
        .collect()
}
fn accounts(entries: &[Value]) -> usize {
    entries
        .iter()
        .map(|e| (e["instance"].to_string(), e["account"].to_string()))
        .collect::<BTreeSet<_>>()
        .len()
}
/// Compare with a baseline. Any inconclusive provider makes the whole comparison
/// inconclusive: partial "no changes" would read as a clean result.
fn changes(before: &Artifact, after: &Artifact) -> Result<Changes, AppError> {
    let comparison = artifact::diff::compare(before, after)?;
    if !comparison.inconclusive.is_empty() {
        let reasons: BTreeSet<_> = comparison
            .inconclusive
            .iter()
            .map(|g| g.reason.to_owned())
            .collect();
        return Ok(Changes::Inconclusive {
            reasons: reasons.into_iter().collect(),
        });
    }
    let (old, new) = (admins(before)?, admins(after)?);
    let no_longer_observed = comparison
        .changes
        .iter()
        .filter(|c| {
            c.entity_kind == "identities"
                && matches!(c.change, artifact::diff::ChangeKind::NoLongerObserved)
        })
        .map(|c| json!({"instance": c.instance, "identity": c.before.as_ref().map(|b| b["id"].clone())}))
        .collect();
    Ok(Changes::Compared {
        new_admins: only_in(&new, &old),
        removed_admins: only_in(&old, &new),
        no_longer_observed,
    })
}

fn exit_code(all_failed: bool, complete: bool, review_items: usize) -> u8 {
    if all_failed {
        3
    } else if !complete {
        4
    } else if review_items > 0 {
        6
    } else {
        0
    }
}

fn build(
    artifact: &Artifact,
    previous: Option<&Artifact>,
    policy: &LoadedPolicy,
    profiles: Option<&ProfileSet>,
    collection_complete: bool,
    args: &CheckArgs,
) -> Result<(CheckReport, bool), AppError> {
    let now = OffsetDateTime::now_utc();
    let profiles_set = profiles;
    let (policy_result, _, evaluation_complete) = policy.evaluate(artifact, now)?;
    let profiles = match profiles {
        Some(set) => {
            let input = crate::profile_command::input(set, artifact);
            let review = crate::profile_command::review(set, &input, artifact, policy, now)?;
            Profiles::Compared {
                enforce: set.enforce,
                complete: review.complete,
                value: review.value,
            }
        }
        None => Profiles::NotConfigured,
    };
    let findings: Vec<Value> = policy_result["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["state"] == "finding" && c["rule"] != "inactive_access")
        .cloned()
        .collect();
    let (departed_assessed, departed) = departures(artifact, profiles_set)?;
    let failed = artifact
        .providers
        .iter()
        .filter(|p| p.state == State::Failed)
        .count();
    let all_failed = !artifact.providers.is_empty() && failed == artifact.providers.len();
    let changes = match previous {
        Some(previous) => changes(previous, artifact)?,
        None => Changes::NoBaseline,
    };
    let comparable = !matches!(changes, Changes::Inconclusive { .. });
    // Report-only profiles never make a run incomplete; enforced ones do.
    let profiles_complete = match &profiles {
        Profiles::Compared {
            enforce: true,
            complete,
            ..
        } => *complete,
        _ => true,
    };
    let complete = collection_complete
        && artifact.complete()
        && evaluation_complete
        && comparable
        && profiles_complete;
    let mut counts: BTreeMap<FindingKind, usize> = ALL_KINDS
        .iter()
        .filter(|k| !report::DIFF_KINDS.contains(k) && !report::PROFILE_KINDS.contains(k))
        .map(|k| (*k, 0))
        .collect();
    counts.insert(FindingKind::DepartedWithAccess, departed.len());
    counts.insert(FindingKind::PolicyFinding, findings.len());
    counts.insert(FindingKind::ProviderFailed, failed);
    counts.insert(FindingKind::Incomplete, usize::from(!complete));
    if let Profiles::Compared { value, .. } = &profiles {
        let count = |key: &str| value["counts"][key].as_u64().unwrap_or(0) as usize;
        counts.insert(FindingKind::UnexplainedAccess, count("unexplained"));
        counts.insert(FindingKind::MissingAccess, count("missing"));
    }
    if let Changes::Compared {
        new_admins,
        removed_admins,
        no_longer_observed,
    } = &changes
    {
        counts.insert(FindingKind::NewAdmin, accounts(new_admins));
        counts.insert(FindingKind::RemovedAdmin, accounts(removed_admins));
        counts.insert(FindingKind::NoLongerObserved, no_longer_observed.len());
    }
    let report = CheckReport {
        check_report_version: 1,
        complete,
        departed_assessed,
        counts,
        providers: artifact
            .providers
            .iter()
            .map(|p| ProviderLine {
                id: p.instance.clone(),
                state: match p.state {
                    State::Complete => "complete",
                    State::Partial => "partial",
                    State::Failed => "failed",
                }
                .into(),
                limitations: p.limitations.clone(),
            })
            .collect(),
        changes,
        departed,
        findings,
        policy: json!({"rules": policy.rules()}),
        profiles,
        notifications: vec![],
        run_url: args.run_url.clone(),
        files: Files {
            snapshot: args.out.join(SNAPSHOT).display().to_string(),
            report: args.out.join(REPORT).display().to_string(),
            summary: args.out.join(SUMMARY).display().to_string(),
        },
    };
    Ok((report, all_failed))
}

fn result_value(report: &CheckReport) -> Result<Value, AppError> {
    let mut value =
        serde_json::to_value(report).map_err(|_| AppError::new(5, "Cannot format check report"))?;
    value["status"] = json!(report.status());
    value["changes_state"] = report.changes.state();
    Ok(value)
}

fn write_report(
    out: &Path,
    report: &CheckReport,
    overwrite: bool,
    cancel: &Cancellation,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(&result_value(report)?)
        .map_err(|_| AppError::new(5, "Cannot format check report"))?;
    artifact::persist_bytes(&out.join(REPORT), &bytes, overwrite, cancel)
}

pub async fn run(
    cli: &Cli,
    args: &CheckArgs,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancel)?;
    check_output(cli, &args.out, args.overwrite)?;
    let workspace = crate::workspace::path(cli)?;
    let notifications = permesh_config::Config::load(&workspace)?
        .check
        .map(|c| c.notifications)
        .unwrap_or_default();
    let policy_path = args.policy.clone();
    let previous_path = args.previous.clone();
    let (policy, previous, profiles) = pool
        .run(move || -> Result<_, AppError> {
            let previous = previous_path.as_deref().map(artifact::load).transpose()?;
            let profiles = crate::profile_command::load(&workspace)?;
            Ok((
                LoadedPolicy::load(policy_path.as_deref())?,
                previous,
                profiles,
            ))
        })
        .await??;
    let (mut outcome, artifact) = crate::snapshot_command::capture(cli, pool, cancel).await?;
    cancelled(cancel)?;
    std::fs::create_dir_all(&args.out)
        .map_err(|_| AppError::new(5, "Cannot create check output directory"))?;
    let collection_complete = outcome.report.complete;
    let owned = args.clone();
    let token = cancel.clone();
    let (report, code) = pool
        .run(move || -> Result<(CheckReport, u8), AppError> {
            artifact::write(
                &owned.out.join(SNAPSHOT),
                &artifact,
                owned.overwrite,
                &token,
            )?;
            let (report, all_failed) = build(
                &artifact,
                previous.as_ref(),
                &policy,
                profiles.as_ref(),
                collection_complete,
                &owned,
            )?;
            let code = exit_code(all_failed, report.complete, report.review_items());
            write_report(&owned.out, &report, owned.overwrite, &token)?;
            let summary = report::summary_markdown(&report);
            artifact::persist_bytes(
                &owned.out.join(SUMMARY),
                summary.as_bytes(),
                owned.overwrite,
                &token,
            )?;
            Ok((report, code))
        })
        .await??;
    // The access result and its files are fixed; delivery can only add its own status.
    let mut report = report;
    if !notifications.is_empty() {
        report.notifications =
            notify::deliver_all(&notifications, &report, !args.no_notify, pool, cancel).await;
        let (out, token, saved) = (args.out.clone(), cancel.clone(), report.clone());
        pool.run(move || write_report(&out, &saved, true, &token))
            .await??;
    }
    outcome.report.command = "check".into();
    outcome.report.complete = report.complete;
    outcome.report.result = result_value(&report)?;
    outcome.code = code;
    Ok(outcome)
}
