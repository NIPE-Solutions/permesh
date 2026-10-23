// SPDX-License-Identifier: MIT
//! Joiner, mover and leaver plans (experimental). Plans are data for a person to
//! carry out, never commands; nothing here changes provider access.
pub(crate) mod model;
mod verify;
mod view;
use crate::{
    args::Cli,
    artifact::{self, Artifact, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    offboard,
    output::safe,
    report::Outcome,
};
use model::{AccessPlan, Kind};
use permesh_config::profiles::ProfileSet;
use permesh_core as core;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, clap::Args)]
pub struct PlanArgs {
    /// Plan file to write (never overwritten).
    #[arg(long)]
    pub output: PathBuf,
    /// Read a saved snapshot offline instead of running fresh discovery.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
    /// Also write a self-contained HTML view of the plan.
    #[arg(long)]
    pub html: Option<PathBuf>,
    /// Hours the plan stays fresh, measured from the evidence start (1–168).
    #[arg(long, default_value_t = 24)]
    pub expires_in_hours: u32,
}

#[derive(Clone, clap::Subcommand)]
pub enum AccessCommand {
    /// Plan the access a person needs for added profiles.
    Join {
        person: String,
        /// Profile to add; repeat for several.
        #[arg(long = "profile", required = true)]
        profiles: Vec<String>,
        #[command(flatten)]
        common: PlanArgs,
    },
    /// Plan a profile change: remove access no remaining profile justifies.
    Move {
        person: String,
        /// Profile to take away; repeat for several.
        #[arg(long, required = true)]
        remove: Vec<String>,
        /// Profile to add; repeat for several.
        #[arg(long)]
        add: Vec<String>,
        #[command(flatten)]
        common: PlanArgs,
    },
    /// Plan the removal of all of a person's access, separating expected from unexplained.
    Leave {
        person: String,
        /// Owner annotations for machine accounts, as in `offboard`.
        #[arg(long)]
        annotations: Option<PathBuf>,
        #[command(flatten)]
        common: PlanArgs,
    },
    /// Check a plan's steps against current observations and flag extra access.
    Verify {
        plan: PathBuf,
        /// Read a saved snapshot offline instead of running fresh discovery.
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Also write a self-contained HTML view of the verification.
        #[arg(long)]
        html: Option<PathBuf>,
    },
}

fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}

/// New, distinct files in existing directories, checked before any capture.
fn check_destinations(paths: &[&Path]) -> Result<(), AppError> {
    let mut targets = BTreeSet::new();
    for path in paths {
        artifact::check_destination(path, false)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .map_err(|_| AppError::input("Report destination directory must exist"))?;
        let name = path
            .file_name()
            .ok_or_else(|| AppError::input("Report destination must name a file"))?;
        if !targets.insert(parent.join(name)) {
            return Err(AppError::input(
                "Plan and HTML outputs require different destinations",
            ));
        }
    }
    Ok(())
}

pub(crate) fn load_profiles(cli: &Cli) -> Result<ProfileSet, AppError> {
    let workspace = crate::workspace::path(cli)?;
    crate::profile_command::load(&workspace)?
        .ok_or_else(|| AppError::input(crate::profile_command::NOT_CONFIGURED))
}

fn check_names<'a>(
    set: &ProfileSet,
    names: impl IntoIterator<Item = &'a String>,
) -> Result<(), AppError> {
    for name in names {
        if !set.profiles.contains_key(name) {
            return Err(AppError::input(format!(
                "Unknown profile {}; profiles are named by the `profile` field in profiles/*.yaml",
                safe(name)
            )));
        }
    }
    Ok(())
}

/// What one plan request is about, before the person's current assignment is known.
pub(crate) struct Change {
    pub kind: Kind,
    pub person: String,
    pub add: BTreeSet<String>,
    pub remove: BTreeSet<String>,
}

/// Plan one change against one capture. Current profiles come from people files
/// and directory group mappings; unresolved directory assignment makes it incomplete.
pub(crate) fn plan_one(
    set: &ProfileSet,
    artifact: &Artifact,
    change: &Change,
    annotations: Option<&offboard::Annotations>,
    mode: offboard::Mode,
    hours: u32,
    now: time::OffsetDateTime,
) -> Result<AccessPlan, AppError> {
    let input = crate::profile_command::input(set, artifact);
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let assignments = core::assign_profiles(&snapshots, &aliases, &input)?;
    let current = assignments
        .profiles
        .get(&change.person)
        .cloned()
        .unwrap_or_default();
    let request = match change.kind {
        Kind::Join | Kind::Move => core::PlanRequest {
            identity: change.person.clone(),
            after: current
                .union(&change.add)
                .filter(|p| !change.remove.contains(*p))
                .cloned()
                .collect(),
            removed: change.remove.clone(),
            leave: false,
        },
        Kind::Leave => core::PlanRequest {
            identity: change.person.clone(),
            after: BTreeSet::new(),
            removed: current,
            leave: true,
        },
    };
    let mut result = core::plan_access(&snapshots, &aliases, &input, &request)?;
    result.complete &= assignments.complete();
    if change.kind == Kind::Leave && !result.accounts.is_empty() {
        match offboard::review_steps(artifact, &change.person, annotations) {
            Ok(reviews) => {
                for (operation, evidence) in reviews {
                    result.steps.push(core::PlannedStep {
                        operation,
                        state: core::EntryState::Finding,
                        reason: "manual_review",
                        justification: core::Justification::Unexplained,
                        evidence,
                        needs_account: None,
                    });
                }
                result.steps.sort_by(|a, b| a.operation.cmp(&b.operation));
            }
            // Annotations that cannot be applied are an input error, as in offboard.
            Err(error) if annotations.is_some() => return Err(error),
            // Without an established authoritative target, ownership is unreviewed.
            Err(_) => result.complete = false,
        }
    }
    model::build(
        change.kind,
        &result,
        &request,
        artifact,
        &input,
        mode,
        hours,
        now,
    )
}

pub(crate) fn plan_value(plan: &AccessPlan) -> Result<Value, AppError> {
    let count = |f: &dyn Fn(&model::Step) -> bool| plan.steps.iter().filter(|s| f(s)).count();
    Ok(json!({
        "access_plan": plan,
        "plan_sha256": model::plan_sha256(plan)?,
        "counts": {
            "steps": plan.steps.len(),
            "cannot_tell": count(&|s| s.state == "cannot_tell"),
            "account_needed": count(&|s| matches!(s.operation, core::Operation::AccountNeeded { .. })),
            "kept": plan.kept.len(),
        },
    }))
}

fn mode(snapshot: Option<&Path>) -> offboard::Mode {
    if snapshot.is_some() {
        offboard::Mode::SnapshotReplay
    } else {
        offboard::Mode::FreshDiscovery
    }
}

async fn plan(
    cli: &Cli,
    change: Change,
    annotations: Option<&Path>,
    common: &PlanArgs,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancel)?;
    let hours = common.expires_in_hours;
    if hours == 0 || hours > 168 {
        return Err(AppError::input(
            "Plan freshness must be 1 through 168 hours",
        ));
    }
    if change.person.is_empty() {
        return Err(AppError::input("A person's canonical identity is required"));
    }
    let mut destinations = vec![common.output.as_path()];
    destinations.extend(common.html.as_deref());
    check_destinations(&destinations)?;
    let owned_cli = cli.clone();
    let annotations = annotations.map(Path::to_owned);
    let (set, annotations) = pool
        .run(move || -> Result<_, AppError> {
            let set = load_profiles(&owned_cli)?;
            let annotations = annotations
                .map(|p| artifact::load_document::<offboard::Annotations>(&p))
                .transpose()?;
            Ok((set, annotations))
        })
        .await??;
    check_names(&set, change.add.iter().chain(&change.remove))?;
    let (mut outcome, artifact) =
        crate::resource_command::input(cli, common.snapshot.as_deref(), pool, cancel).await?;
    cancelled(cancel)?;
    if !artifact.providers.is_empty() && artifact.providers.iter().all(|p| p.state == State::Failed)
    {
        return Err(AppError::new(
            3,
            "No provider returned usable observations; no plan was written",
        ));
    }
    let mode = mode(common.snapshot.as_deref());
    let html_requested = common.html.is_some();
    let (plan, html) = pool
        .run(move || -> Result<_, AppError> {
            let plan = plan_one(
                &set,
                &artifact,
                &change,
                annotations.as_ref(),
                mode,
                hours,
                time::OffsetDateTime::now_utc(),
            )?;
            let html = html_requested.then(|| view::plan_html(&plan)).transpose()?;
            Ok((plan, html))
        })
        .await??;
    cancelled(cancel)?;
    let (output, saved, token) = (common.output.clone(), plan.clone(), cancel.clone());
    pool.run(move || artifact::write_document(&output, &saved, &token))
        .await??;
    if let (Some(path), Some(bytes)) = (common.html.clone(), html) {
        let token = cancel.clone();
        pool.run(move || artifact::write_bytes(&path, &bytes, &token))
            .await??;
    }
    let mut value = plan_value(&plan)?;
    value["files"] = json!({
        "plan": common.output.display().to_string(),
        "html": common.html.as_ref().map(|p| p.display().to_string()),
    });
    outcome.report.command = "access_plan".into();
    outcome.report.complete = plan.complete;
    outcome.report.result = value;
    outcome.code = if plan.complete { 0 } else { 4 };
    Ok(outcome)
}

async fn verify(
    cli: &Cli,
    path: &Path,
    snapshot: Option<&Path>,
    html: Option<&Path>,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancel)?;
    check_destinations(&html.into_iter().collect::<Vec<_>>())?;
    let (path, workspace) = (path.to_owned(), crate::workspace::path(cli)?);
    // A plan that fails validation is refused before any provider is contacted.
    let (plan, set) = pool
        .run(move || -> Result<_, AppError> {
            let plan: AccessPlan = artifact::load_document(&path)?;
            plan.validate()?;
            Ok((plan, crate::profile_command::load(&workspace)?))
        })
        .await??;
    let (mut outcome, artifact) =
        crate::resource_command::input(cli, snapshot, pool, cancel).await?;
    cancelled(cancel)?;
    let all_failed = !artifact.providers.is_empty()
        && artifact.providers.iter().all(|p| p.state == State::Failed);
    let html_requested = html.is_some();
    let (value, bytes) = pool
        .run(move || -> Result<_, AppError> {
            let input = set
                .as_ref()
                .map(|s| crate::profile_command::input(s, &artifact));
            let value = verify::verify(&plan, &artifact, input.as_ref())?;
            let bytes = html_requested
                .then(|| view::verify_html(&value))
                .transpose()?;
            Ok((value, bytes))
        })
        .await??;
    cancelled(cancel)?;
    if let (Some(path), Some(bytes)) = (html.map(Path::to_owned), bytes) {
        let token = cancel.clone();
        pool.run(move || artifact::write_bytes(&path, &bytes, &token))
            .await??;
    }
    let complete = value["complete"] == true;
    outcome.code = if all_failed {
        3
    } else if !complete {
        4
    } else if value["counts"]["not_done"].as_u64().unwrap_or(0) > 0 {
        6
    } else {
        0
    };
    outcome.report.command = "access_verify".into();
    outcome.report.complete = complete;
    outcome.report.result = value;
    Ok(outcome)
}

pub async fn run(
    cli: &Cli,
    command: &AccessCommand,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    let set = |names: &[String]| names.iter().cloned().collect::<BTreeSet<_>>();
    match command {
        AccessCommand::Join {
            person,
            profiles,
            common,
        } => {
            let change = Change {
                kind: Kind::Join,
                person: person.clone(),
                add: set(profiles),
                remove: BTreeSet::new(),
            };
            plan(cli, change, None, common, pool, cancel).await
        }
        AccessCommand::Move {
            person,
            remove,
            add,
            common,
        } => {
            let change = Change {
                kind: Kind::Move,
                person: person.clone(),
                add: set(add),
                remove: set(remove),
            };
            plan(cli, change, None, common, pool, cancel).await
        }
        AccessCommand::Leave {
            person,
            annotations,
            common,
        } => {
            let change = Change {
                kind: Kind::Leave,
                person: person.clone(),
                add: BTreeSet::new(),
                remove: BTreeSet::new(),
            };
            plan(cli, change, annotations.as_deref(), common, pool, cancel).await
        }
        AccessCommand::Verify {
            plan,
            snapshot,
            html,
        } => {
            verify(
                cli,
                plan,
                snapshot.as_deref(),
                html.as_deref(),
                pool,
                cancel,
            )
            .await
        }
    }
}

/// Plain words for an operation; every provider value goes through `safe`.
pub(crate) fn words(operation: &Value) -> String {
    let f = |name: &str| safe(operation[name].as_str().unwrap_or_default());
    let instance = f("instance");
    match operation["type"].as_str().unwrap_or_default() {
        "account_needed" => format!("account needed in {instance} (create or invite)"),
        "add_membership" => format!("add membership {} in {instance}", f("group")),
        "add_role" => format!("add role {} on {} in {instance}", f("role"), f("resource")),
        "remove_membership" => format!(
            "remove membership {} in {instance} (account {})",
            f("group"),
            f("account")
        ),
        "remove_role" => format!(
            "remove role {} on {} in {instance} (account {}, grant {})",
            f("role"),
            f("resource"),
            f("account"),
            f("grant_id")
        ),
        "review_account_lifecycle" => {
            format!("review account {} lifecycle in {instance}", f("account"))
        }
        "review_ownership" => format!(
            "review ownership of {} by account {} in {instance}",
            f("resource"),
            f("account")
        ),
        "review_machine_dependency" => format!(
            "review machine account {} dependency in {instance}",
            f("account")
        ),
        other => safe(other),
    }
}

pub(crate) fn justification(value: &Value) -> String {
    match value.get("profiles").and_then(Value::as_array) {
        Some(names) => format!(
            "profiles {}",
            names
                .iter()
                .map(|n| safe(n.as_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        None => "unexplained".into(),
    }
}

fn write_plan(out: &mut impl Write, result: &Value) -> io::Result<()> {
    let plan = &result["access_plan"];
    let text = |v: &Value| safe(v.as_str().unwrap_or_default());
    writeln!(
        out,
        "Access plan: {} for {}{}",
        text(&plan["kind"]),
        text(&plan["identity"]),
        if plan["complete"] == true {
            ""
        } else {
            " (INCOMPLETE: some access could not be observed)"
        }
    )?;
    let counts = &result["counts"];
    writeln!(
        out,
        "  {} steps; {} cannot tell; {} accounts needed; {} kept",
        counts["steps"], counts["cannot_tell"], counts["account_needed"], counts["kept"]
    )?;
    for step in plan["steps"].as_array().into_iter().flatten() {
        let cannot = if step["state"] == "cannot_tell" {
            format!(
                " [cannot tell: {}]",
                text(&step["reason"]).replace('_', " ")
            )
        } else {
            String::new()
        };
        writeln!(
            out,
            "  - {} — {}{cannot}",
            words(&step["operation"]),
            justification(&step["justification"])
        )?;
    }
    for kept in plan["kept"].as_array().into_iter().flatten() {
        let entry = &kept["entry"];
        let what = if entry["kind"] == "group" {
            format!("membership {}", text(&entry["group"]))
        } else {
            format!(
                "role {} on {}",
                text(&entry["role"]),
                text(&entry["resource"])
            )
        };
        let because: Vec<String> = kept["because"]
            .as_array()
            .into_iter()
            .flatten()
            .map(text)
            .collect();
        writeln!(
            out,
            "  kept: {what} in {} — because of {}",
            text(&entry["instance"]),
            because.join(", ")
        )?;
    }
    writeln!(out, "\nPlan file: {}", text(&result["files"]["plan"]))?;
    if result["files"]["html"].is_string() {
        writeln!(out, "HTML: {}", text(&result["files"]["html"]))?;
    }
    writeln!(
        out,
        "Plans are data: a person carries out each step. Run permesh access verify afterwards."
    )
}

fn write_verify(out: &mut impl Write, result: &Value) -> io::Result<()> {
    let text = |v: &Value| safe(v.as_str().unwrap_or_default());
    let counts = &result["counts"];
    writeln!(
        out,
        "Access verification: {} for {}{}\n  {} done; {} not done; {} cannot verify ({} manual review); {} extra",
        text(&result["kind"]),
        text(&result["identity"]),
        if result["complete"] == true {
            ""
        } else {
            " (INCOMPLETE)"
        },
        counts["done"],
        counts["not_done"],
        counts["cannot_verify"],
        counts["manual_review"],
        counts["extra"],
    )?;
    for check in result["checks"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "  - {}: {} ({})",
            words(&check["operation"]),
            text(&check["state"]).replace('_', " "),
            text(&check["reason"]).replace('_', " ")
        )?;
    }
    for row in result["extra"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "  extra ({}): {} on {} ({}) in {}, account {}",
            text(&row["state"]).replace('_', " "),
            text(&row["role"]),
            text(&row["resource_name"]),
            text(&row["resource"]),
            text(&row["instance"]),
            text(&row["account"]),
        )?;
    }
    for gap in result["gaps"].as_array().into_iter().flatten() {
        writeln!(out, "  gap: {}", text(gap).replace('_', " "))?;
    }
    Ok(())
}

pub fn write(out: &mut impl Write, command: &str, result: &Value) -> io::Result<()> {
    match command {
        "access_plan" => write_plan(out, result),
        "access_verify" => write_verify(out, result),
        _ => writeln!(out, "{}", safe(&result.to_string())),
    }
}
