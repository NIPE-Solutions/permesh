// SPDX-License-Identifier: MIT
//! Declared access profiles against captured access (experimental report format).
use crate::{
    args::Cli,
    artifact::{Artifact, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    policy_command::LoadedPolicy,
    report::Outcome,
};
use permesh_config::profiles::{ProfileEntry, ProfileSet, load_profiles};
use permesh_core::{self as core, EntryState};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};
use time::OffsetDateTime;

const NOT_CONFIGURED: &str = "No access profiles are configured; add profiles/ and people/ files or a profiles section (see docs/profiles.md)";

#[derive(Clone, clap::Subcommand)]
pub enum ProfileCommand {
    /// Compare people's declared profiles with observed and derived access.
    Check {
        /// Read a saved snapshot offline instead of running fresh discovery.
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Policy file whose `unexplained_access` exceptions apply.
        #[arg(long)]
        policy: Option<PathBuf>,
    },
}

/// A profile comparison ready for reports; `findings` excludes excepted and unknown rows.
pub(crate) struct ProfileReview {
    pub value: Value,
    pub findings: usize,
    pub complete: bool,
}

/// Profiles next to the workspace file; `None` when nothing is configured.
pub(crate) fn load(workspace: &Path) -> Result<Option<ProfileSet>, AppError> {
    let config = permesh_config::Config::load(workspace)?;
    let root = workspace.parent().unwrap_or(Path::new("."));
    Ok(load_profiles(root, &config)?)
}

/// Comparison input; only completely captured instances are observable.
pub(crate) fn input(set: &ProfileSet, artifact: &Artifact) -> core::ProfileInput {
    let entry = |e: &ProfileEntry| match e.clone() {
        ProfileEntry::Group { instance, group } => core::ExpectedEntry::Group { instance, group },
        ProfileEntry::Role {
            instance,
            role,
            resource,
        } => core::ExpectedEntry::Role {
            instance,
            role,
            resource,
        },
    };
    core::ProfileInput {
        profiles: set
            .profiles
            .iter()
            .map(|(name, entries)| (name.clone(), entries.iter().map(entry).collect()))
            .collect(),
        people: set.people.clone(),
        directory_groups: set
            .directory_groups
            .iter()
            .map(|d| core::DirectoryAssignment {
                instance: d.instance.clone(),
                group: d.group.clone(),
                profiles: d.profiles.clone(),
            })
            .collect(),
        observable: artifact
            .providers
            .iter()
            .filter(|p| p.state == State::Complete)
            .map(|p| (p.instance.clone(), p.capabilities.iter().cloned().collect()))
            .collect(),
    }
}

fn state(value: EntryState) -> &'static str {
    match value {
        EntryState::Finding => "finding",
        EntryState::CannotTell => "cannot_tell",
    }
}

pub(crate) fn review(
    set: &ProfileSet,
    input: &core::ProfileInput,
    artifact: &Artifact,
    policy: &LoadedPolicy,
    now: OffsetDateTime,
) -> Result<ProfileReview, AppError> {
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let comparison = core::compare_profiles(&snapshots, &aliases, input)?;
    let (mut missing, mut unexplained, mut cannot_tell, mut excepted, mut covered) =
        (0, 0, 0, 0, 0);
    let mut identities = Vec::new();
    for identity in &comparison.identities {
        covered += usize::from(identity.state == core::ComparisonState::Compared);
        let mut missing_rows = Vec::new();
        for entry in &identity.missing {
            match entry.state {
                EntryState::Finding => missing += 1,
                EntryState::CannotTell => cannot_tell += 1,
            }
            missing_rows.push(
                json!({"state":state(entry.state),"entry":entry.entry,"reason":entry.reason}),
            );
        }
        let mut unexplained_rows = Vec::new();
        for row in &identity.unexplained {
            let path = &row.path;
            // Exceptions accept known findings; they never settle unknown evidence.
            let exception = (row.state == EntryState::Finding)
                .then(|| policy.unexplained_exception(path, now))
                .flatten();
            let label = match (&exception, row.state) {
                (Some(_), _) => {
                    excepted += 1;
                    "excepted"
                }
                (None, EntryState::Finding) => {
                    unexplained += 1;
                    "finding"
                }
                (None, EntryState::CannotTell) => {
                    cannot_tell += 1;
                    "cannot_tell"
                }
            };
            let mut value = json!({
                "state": label,
                "instance": path.account.provider,
                "account": path.account.id,
                "resource": path.resource.key.id,
                "resource_name": path.resource.name,
                "role": path.grant.role,
                "grant_id": path.grant.id,
                "via": path.groups.iter().map(|g| g.key.id.as_str()).collect::<Vec<_>>(),
                "certainty": path.certainty(),
            });
            if let Some(exception) = exception {
                value["exception"] = exception;
            }
            unexplained_rows.push(value);
        }
        identities.push(json!({
            "identity": identity.identity,
            "state": identity.state,
            "profiles": identity.profiles,
            "accounts": identity.accounts.iter().map(|a| json!({"instance":a.provider,"account":a.id})).collect::<Vec<_>>(),
            "explained": identity.explained,
            "missing": missing_rows,
            "unexplained": unexplained_rows,
        }));
    }
    let complete = comparison.complete && artifact.complete();
    let value = json!({
        "profile_review_version": 1,
        "enforce": set.enforce,
        "complete": complete,
        "counts": {
            "covered": covered,
            "not_covered": comparison.not_covered,
            "unexplained": unexplained,
            "missing": missing,
            "cannot_tell": cannot_tell,
            "excepted": excepted,
            "ambiguous_accounts": comparison.ambiguous_accounts,
            "unresolved_members": comparison.unresolved_members,
        },
        "identities": identities,
        "unresolved_assignments": comparison.unresolved_assignments,
        "message": "Read-only comparison of declared profiles with captured access. Unexplained access is a review finding, not proof of wrong access; cannot_tell entries were not fully observed.",
    });
    Ok(ProfileReview {
        value,
        findings: missing + unexplained,
        complete,
    })
}

fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}

pub async fn run(
    cli: &Cli,
    command: &ProfileCommand,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    let ProfileCommand::Check { snapshot, policy } = command;
    cancelled(cancel)?;
    let workspace = crate::workspace::path(cli)?;
    let policy = policy.clone();
    let (set, policy) = pool
        .run(move || -> Result<_, AppError> {
            let set = load(&workspace)?.ok_or_else(|| AppError::input(NOT_CONFIGURED))?;
            Ok((set, LoadedPolicy::load(policy.as_deref())?))
        })
        .await??;
    let (mut outcome, artifact) =
        crate::resource_command::input(cli, snapshot.as_deref(), pool, cancel).await?;
    cancelled(cancel)?;
    let all_failed = artifact.providers.iter().all(|p| p.state == State::Failed);
    let review = pool
        .run(move || {
            let input = input(&set, &artifact);
            review(&set, &input, &artifact, &policy, OffsetDateTime::now_utc())
        })
        .await??;
    cancelled(cancel)?;
    outcome.report.command = "profile_check".into();
    outcome.report.complete = review.complete;
    outcome.code = if all_failed {
        3
    } else if !review.complete {
        4
    } else if review.findings > 0 {
        6
    } else {
        0
    };
    outcome.report.result = review.value;
    Ok(outcome)
}

pub fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    use crate::output::safe;
    let text = |v: &Value| safe(v.as_str().unwrap_or_default());
    let counts = &result["counts"];
    writeln!(
        out,
        "Access profiles{}\n  {} people compared; {} not covered by a profile\n  {} unexplained; {} missing; {} cannot tell; {} accepted exceptions",
        if result["enforce"] == true {
            ""
        } else {
            " (report-only)"
        },
        counts["covered"],
        counts["not_covered"],
        counts["unexplained"],
        counts["missing"],
        counts["cannot_tell"],
        counts["excepted"],
    )?;
    for identity in result["identities"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "\n{} ({})",
            text(&identity["identity"]),
            text(&identity["state"]).replace('_', " ")
        )?;
        for row in identity["missing"].as_array().into_iter().flatten() {
            let entry = &row["entry"];
            let target = if entry["kind"] == "group" {
                format!("member of {}", text(&entry["group"]))
            } else {
                format!("{} on {}", text(&entry["role"]), text(&entry["resource"]))
            };
            writeln!(
                out,
                "  missing ({}): {} in {} — {}",
                text(&row["state"]).replace('_', " "),
                target,
                text(&entry["instance"]),
                text(&row["reason"]).replace('_', " ")
            )?;
        }
        for row in identity["unexplained"].as_array().into_iter().flatten() {
            let via: Vec<String> = row["via"]
                .as_array()
                .into_iter()
                .flatten()
                .map(text)
                .collect();
            writeln!(
                out,
                "  unexplained ({}): {} on {} ({}) in {}, account {}, {}",
                text(&row["state"]).replace('_', " "),
                text(&row["role"]),
                text(&row["resource_name"]),
                text(&row["resource"]),
                text(&row["instance"]),
                text(&row["account"]),
                if via.is_empty() {
                    "direct".to_owned()
                } else {
                    format!("via {}", via.join(" > "))
                }
            )?;
            if let Some(exception) = row.get("exception").filter(|e| e.is_object()) {
                writeln!(
                    out,
                    "    exception {} / owner {} / expires {}",
                    text(&exception["id"]),
                    text(&exception["owner"]),
                    text(&exception["expires_at"])
                )?;
            }
        }
    }
    for row in result["unresolved_assignments"]
        .as_array()
        .into_iter()
        .flatten()
    {
        writeln!(
            out,
            "\nDirectory group {} in {} not resolved: {}",
            text(&row["group"]),
            text(&row["instance"]),
            text(&row["reason"]).replace('_', " ")
        )?;
    }
    Ok(())
}
