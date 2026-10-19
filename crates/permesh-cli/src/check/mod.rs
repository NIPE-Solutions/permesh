// SPDX-License-Identifier: MIT
//! `permesh check`: one unattended review run (experimental report format).
//! Collect a snapshot, evaluate policy and departures, write files, then notify.
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
use permesh_core::{self as core, OrphanReason};
use report::{ALL_KINDS, Changes, CheckReport, Files, ProviderLine};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
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

/// Departed = an authoritative identity classified inactive or suspended whose
/// account still has observed or derived access paths.
fn departures(artifact: &Artifact) -> Result<(bool, Vec<Value>), AppError> {
    let authorities = &artifact.identity.authorities;
    if authorities.is_empty() {
        return Ok((false, vec![]));
    }
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let orphaned = core::query_orphaned(&snapshots, &aliases, authorities)?;
    let mut paths: BTreeMap<&core::EntityKey, usize> = BTreeMap::new();
    for path in &orphaned.access {
        *paths.entry(&path.account).or_default() += 1;
    }
    let departed = orphaned
        .accounts
        .iter()
        .filter(|a| {
            matches!(
                a.reason,
                OrphanReason::InactiveIdentity | OrphanReason::SuspendedIdentity
            )
        })
        .filter_map(|a| {
            paths.get(&a.account.key).map(|count| {
                json!({
                    "instance": a.account.key.provider,
                    "account": a.account.key.id,
                    "login": a.account.login,
                    "reason": a.reason,
                    "access_paths": count,
                })
            })
        })
        .collect();
    Ok((true, departed))
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
    policy: &LoadedPolicy,
    collection_complete: bool,
    args: &CheckArgs,
) -> Result<(CheckReport, bool), AppError> {
    let (policy_result, _, evaluation_complete) =
        policy.evaluate(artifact, OffsetDateTime::now_utc())?;
    let findings: Vec<Value> = policy_result["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["state"] == "finding" && c["rule"] != "inactive_access")
        .cloned()
        .collect();
    let (departed_assessed, departed) = departures(artifact)?;
    let failed = artifact
        .providers
        .iter()
        .filter(|p| p.state == State::Failed)
        .count();
    let all_failed = !artifact.providers.is_empty() && failed == artifact.providers.len();
    let complete = collection_complete && artifact.complete() && evaluation_complete;
    let mut counts: BTreeMap<FindingKind, usize> = ALL_KINDS.iter().map(|k| (*k, 0)).collect();
    counts.insert(FindingKind::DepartedWithAccess, departed.len());
    counts.insert(FindingKind::PolicyFinding, findings.len());
    counts.insert(FindingKind::ProviderFailed, failed);
    counts.insert(FindingKind::Incomplete, usize::from(!complete));
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
        changes: Changes::NoBaseline,
        departed,
        findings,
        policy: json!({"rules": policy.rules()}),
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
    let policy_path = args.policy.clone();
    let policy = pool
        .run(move || LoadedPolicy::load(policy_path.as_deref()))
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
            let (report, all_failed) = build(&artifact, &policy, collection_complete, &owned)?;
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
    outcome.report.command = "check".into();
    outcome.report.complete = report.complete;
    outcome.report.result = result_value(&report)?;
    outcome.code = code;
    Ok(outcome)
}
