// SPDX-License-Identifier: MIT
//! Declared access profiles against captured access (stable report format, ADR 0036).
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

pub(crate) const NOT_CONFIGURED: &str = "No access profiles are configured; add profiles/ and people/ files or a profiles section (see docs/profiles.md)";

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
    /// Draft a profile from the access a group's members share; prints YAML to review.
    Suggest {
        /// Group as INSTANCE:GROUP_ID.
        #[arg(long)]
        group: String,
        /// Profile name for the draft.
        #[arg(long)]
        name: String,
        /// Include entries shared by at least this percentage of members.
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u8).range(1..=100))]
        min_percent: u8,
        /// Read a saved snapshot offline instead of running fresh discovery.
        #[arg(long)]
        snapshot: Option<PathBuf>,
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
    // Someone who left is expected to hold nothing: remaining access is unexplained,
    // and their former profiles are not missing.
    let mut input = input.clone();
    for identity in &set.left {
        input.people.insert(identity.clone(), Default::default());
    }
    let comparison = core::compare_profiles(&snapshots, &aliases, &input)?;
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
    let (snapshot, policy) = match command {
        ProfileCommand::Check { snapshot, policy } => (snapshot, policy),
        ProfileCommand::Suggest {
            group,
            name,
            min_percent,
            snapshot,
        } => {
            return suggest(
                cli,
                group,
                name,
                *min_percent,
                snapshot.as_deref(),
                pool,
                cancel,
            )
            .await;
        }
    };
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

fn parse_group(value: &str) -> Result<core::EntityKey, AppError> {
    match value.split_once(':') {
        Some((instance, id)) if !instance.is_empty() && !id.is_empty() => {
            Ok(core::EntityKey::new(instance, id))
        }
        _ => Err(AppError::input("--group must be INSTANCE:GROUP_ID")),
    }
}

/// YAML draft: entries as JSON flow mappings (valid YAML), labels only in comments.
fn draft(name: &str, suggestion: &core::Suggestion) -> Result<String, AppError> {
    use crate::output::safe;
    let group = safe(&suggestion.group.name);
    let mut text = format!(
        "# Draft from {} group {group}: {} members ({} unresolved, excluded). Review before committing.\nprofile: {name}\n",
        safe(&suggestion.group.key.provider),
        suggestion.members,
        suggestion.unresolved_members,
    );
    if suggestion.entries.is_empty() {
        text.push_str("grants: []\n");
        return Ok(text);
    }
    text.push_str("grants:\n");
    for suggested in &suggestion.entries {
        let entry = match suggested.entry.clone() {
            core::ExpectedEntry::Group { instance, group } => {
                ProfileEntry::Group { instance, group }
            }
            core::ExpectedEntry::Role {
                instance,
                role,
                resource,
            } => ProfileEntry::Role {
                instance,
                role,
                resource,
            },
        };
        let entry = serde_json::to_string(&entry)
            .map_err(|_| AppError::new(5, "Cannot encode profile draft"))?
            .chars()
            .map(|c| {
                // Keep IDs visible as reviewed: escape what the terminal would hide or reorder.
                if safe(&c.to_string()) == c.to_string() {
                    c.to_string()
                } else {
                    format!("\\u{:04x}", u32::from(c))
                }
            })
            .collect::<String>();
        text.push_str(&format!(
            "  - {entry}  # {}: {} of {} members\n",
            safe(&suggested.label),
            suggested.count,
            suggestion.members
        ));
    }
    Ok(text)
}

async fn suggest(
    cli: &Cli,
    group: &str,
    name: &str,
    min_percent: u8,
    snapshot: Option<&Path>,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    let group = parse_group(group)?;
    if !permesh_config::profiles::valid_name(name) {
        return Err(AppError::input(
            "--name must be letters, digits, '-' or '_' (at most 128)",
        ));
    }
    let name = name.to_owned();
    let (mut outcome, artifact) =
        crate::resource_command::input(cli, snapshot, pool, cancel).await?;
    cancelled(cancel)?;
    let complete = artifact.complete();
    let result = pool
        .run(move || -> Result<_, AppError> {
            let (snapshots, aliases) = crate::resource_command::domain(&artifact);
            let suggestion = core::suggest_profile(&snapshots, &aliases, &group, min_percent)?;
            if suggestion.members == 0 {
                return Err(AppError::input(
                    "The group has no members attributed to an identity; nothing to suggest",
                ));
            }
            Ok(json!({
                "profile_suggestion_version": 1,
                "group": {"instance": suggestion.group.key.provider, "id": suggestion.group.key.id, "name": suggestion.group.name},
                "members": suggestion.members,
                "unresolved_members": suggestion.unresolved_members,
                "entries": suggestion.entries,
                "draft": draft(&name, &suggestion)?,
            }))
        })
        .await??;
    outcome.report.command = "profile_suggest".into();
    outcome.report.complete = complete;
    outcome.report.result = result;
    outcome.code = if complete { 0 } else { 4 };
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
    // Uncompared evidence stays visible, never folded into a clean count.
    for (key, what) in [
        (
            "ambiguous_accounts",
            "accounts claimed by more than one identity (not attributed)",
        ),
        (
            "unresolved_members",
            "directory group members without one identity (not compared)",
        ),
    ] {
        if counts[key].as_u64().unwrap_or(0) > 0 {
            writeln!(out, "  {} {what}", counts[key])?;
        }
    }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn draft_escapes_invisible_and_bidi_characters_in_ids() {
        let suggestion = core::Suggestion {
            group: core::Group {
                key: core::EntityKey::new("demo", "team-10"),
                name: "team".into(),
            },
            members: 1,
            unresolved_members: 0,
            entries: vec![core::SuggestedEntry {
                entry: core::ExpectedEntry::Role {
                    instance: "demo".into(),
                    role: "Read\u{202e}X\u{9b}\u{7f}".into(),
                    resource: "repo-21".into(),
                },
                label: "infra".into(),
                count: 1,
            }],
        };
        let text = draft("backend", &suggestion).unwrap();
        assert!(
            text.contains(r#""role":"Read\u202eX\u009b\u007f""#),
            "{text}"
        );
        assert!(
            text.chars()
                .all(|c| c == '\n' || crate::output::safe(&c.to_string()) == c.to_string())
        );
        let parsed: serde_json::Value = permesh_config::parse_setup_value(&text).unwrap();
        assert_eq!(parsed["grants"][0]["role"], "Read\u{202e}X\u{9b}\u{7f}");
    }
}
