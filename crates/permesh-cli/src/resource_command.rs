// SPDX-License-Identifier: MIT
//! Resource-first CLI DTOs and shared fresh/offline review input.
use crate::{
    args::Cli,
    artifact::{self, Artifact, records},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    report::{Outcome, ProviderStatus},
};
use permesh_core as core;
use serde::Serialize;
use serde_json::json;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, clap::Args)]
pub struct ResourceArgs {
    /// Exact resource display label. Ambiguity returns stable candidates for review.
    pub query: Option<String>,
    #[arg(long)]
    pub instance: Option<String>,
    #[arg(long, requires = "instance", conflicts_with = "query")]
    pub id: Option<String>,
    /// Read a saved snapshot offline instead of running fresh discovery.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
}
pub(crate) async fn input(
    cli: &Cli,
    file: Option<&Path>,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<(Outcome, Artifact), AppError> {
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    if let Some(file) = file {
        let file = file.to_owned();
        let artifact = pool.run(move || artifact::load(&file)).await??;
        let mut outcome = Outcome::new("resource", json!({}))?;
        outcome.report.complete = artifact.complete();
        outcome.code = if artifact.complete() {
            0
        } else if artifact
            .providers
            .iter()
            .all(|p| p.state == artifact::State::Failed)
        {
            3
        } else {
            4
        };
        outcome.report.providers = artifact
            .providers
            .iter()
            .map(|p| ProviderStatus {
                id: p.instance.clone(),
                kind: p.provider_type.clone(),
                state: match p.state {
                    artifact::State::Complete => "connected",
                    artifact::State::Partial => "partial",
                    artifact::State::Failed => "failed",
                }
                .into(),
                message: "Saved observation; no provider contacted".into(),
                limitations: p.limitations.clone(),
            })
            .collect();
        Ok((outcome, artifact))
    } else {
        crate::snapshot_command::capture(cli, pool, cancel).await
    }
}
pub(crate) fn domain(artifact: &Artifact) -> (Vec<core::Snapshot>, core::Aliases) {
    let snapshots = artifact
        .providers
        .iter()
        .filter_map(|p| p.data.as_ref().map(core::Snapshot::from))
        .collect();
    let mut aliases = core::Aliases::new();
    for b in &artifact.identity.bindings {
        aliases
            .entry(b.identity.clone())
            .or_default()
            .entry(b.instance.clone())
            .or_default()
            .push(b.account.clone());
    }
    (snapshots, aliases)
}
#[derive(Serialize)]
struct AccessPath {
    account: records::EntityKey,
    groups: Vec<records::Group>,
    memberships: Vec<records::Membership>,
    grant: records::Grant,
    certainty: records::Certainty,
}
impl From<&core::AccessPath> for AccessPath {
    fn from(p: &core::AccessPath) -> Self {
        Self {
            account: (&p.account).into(),
            groups: p.groups.iter().map(records::Group::from).collect(),
            memberships: p
                .memberships
                .iter()
                .map(records::Membership::from)
                .collect(),
            grant: (&p.grant).into(),
            certainty: p.certainty().into(),
        }
    }
}
pub async fn run(
    cli: &Cli,
    args: &ResourceArgs,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    let (outcome, artifact) = input(cli, args.snapshot.as_deref(), pool, cancel).await?;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    let args = args.clone();
    let outcome = pool.run(move || finish(&args, outcome, artifact)).await??;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    Ok(outcome)
}
fn finish(
    args: &ResourceArgs,
    mut outcome: Outcome,
    artifact: Artifact,
) -> Result<Outcome, AppError> {
    if args
        .instance
        .as_ref()
        .is_some_and(|id| !artifact.providers.iter().any(|p| p.instance == *id))
    {
        return Err(AppError::input(
            "Unknown resource provider instance in this review",
        ));
    }
    let (snapshots, aliases) = domain(&artifact);
    let mut resources: Vec<_> = snapshots
        .iter()
        .flat_map(|s| s.resources.iter())
        .filter(|r| args.instance.as_ref().is_none_or(|i| r.key.provider == *i))
        .collect();
    if resources.len() > 100_000 {
        return Err(AppError::input(
            "Resource review exceeds the 100000-resource limit; narrow provider scope",
        ));
    }
    resources.sort_by(|a, b| a.key.cmp(&b.key));
    let matches: Vec<_> = resources
        .into_iter()
        .filter(|r| {
            if let Some(id) = &args.id {
                r.key.id == *id
            } else {
                args.query.as_ref().is_none_or(|q| r.name == *q)
            }
        })
        .collect();
    outcome.report.command = "resource".into();
    let selected = args.id.is_some() || args.query.is_some();
    if !selected || matches.len() != 1 {
        if selected {
            outcome.code = if matches.len() > 1 {
                2
            } else if outcome.code == 0 {
                1
            } else {
                outcome.code
            };
        }
        outcome.report.result = json!({"resource_review_version":1,"selection_required":selected&&matches.len()>1,"resources":matches.iter().map(|r|records::Resource::from(*r)).collect::<Vec<_>>(),"observation_started_at":artifact.started_at,"observation_completed_at":artifact.completed_at,"message":"Select the exact provider instance and immutable resource ID with --instance and --id. Missing observations do not establish absence of access."});
        return Ok(outcome);
    }
    let result = core::query_resource(
        &snapshots,
        &aliases,
        &artifact.identity.authorities,
        &matches[0].key,
    )?;
    let accounts: Vec<_> = result
        .accounts
        .iter()
        .map(|a| {
            let identity = match &a.identity {
                core::IdentityResolution::Unmapped => json!({"state":"unmapped"}),
                core::IdentityResolution::Resolved { identity } => {
                    json!({"state":"resolved","identity":records::Identity::from(identity)})
                }
                core::IdentityResolution::Ambiguous { candidates } => {
                    json!({"state":"ambiguous","candidates":candidates})
                }
            };
            json!({"account":records::Account::from(&a.account),"identity":identity})
        })
        .collect();
    let unresolved:Vec<_>=result.unresolved_grants.iter().map(|g|json!({"grant":records::Grant::from(&g.grant),"group":g.group.as_ref().map(records::Group::from)})).collect();
    outcome.report.result = json!({"resource_review_version":1,"resource":records::Resource::from(&result.resource),"provider_complete":result.provider_complete,"accounts":accounts,"access":result.access.iter().map(AccessPath::from).collect::<Vec<_>>(),"source_grants":result.grants.iter().map(records::Grant::from).collect::<Vec<_>>(),"unresolved_group_grants":unresolved,"path_count":result.access.len(),"unique_source_grant_count":result.grants.len(),"unique_account_count":result.accounts.len(),"observation_started_at":artifact.started_at,"observation_completed_at":artifact.completed_at,"message":"Paths preserve original source grant evidence and membership derivation. Duplicate routes are not distinct permissions; source grant counts do not prove effective access. Unresolved groups and unmapped accounts remain visible."});
    Ok(outcome)
}
pub fn write(out: &mut impl Write, result: &serde_json::Value) -> io::Result<()> {
    use crate::output::safe;
    let field = |v: &serde_json::Value, key: &str| safe(v[key].as_str().unwrap_or_default());
    if let Some(resources) = result["resources"].as_array() {
        writeln!(out, "Resource selection")?;
        for resource in resources {
            writeln!(
                out,
                "  {}  [{} / {}]",
                field(resource, "name"),
                field(&resource["key"], "provider"),
                field(&resource["key"], "id")
            )?;
        }
        if resources.is_empty() {
            writeln!(out, "  No matching resources observed")?;
        }
        return Ok(());
    }
    let resource = &result["resource"];
    writeln!(
        out,
        "Resource\n  {}  [{} / {}]",
        field(resource, "name"),
        field(&resource["key"], "provider"),
        field(&resource["key"], "id")
    )?;
    writeln!(out, "\nObserved accounts")?;
    for row in result["accounts"].as_array().into_iter().flatten() {
        let account = &row["account"];
        writeln!(
            out,
            "  {} [{}] — {} identity",
            field(account, "login"),
            field(&account["key"], "id"),
            field(&row["identity"], "state")
        )?;
    }
    writeln!(out, "\nAccess evidence")?;
    for path in result["access"].as_array().into_iter().flatten() {
        let via: Vec<_> = path["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| field(g, "name"))
            .collect();
        writeln!(
            out,
            "  Account {}: {} [{}; {} evidence]",
            field(&path["account"], "id"),
            field(&path["grant"], "role"),
            field(&path["grant"], "privilege"),
            field(path, "certainty")
        )?;
        writeln!(
            out,
            "    Source grant {} ({})",
            field(&path["grant"], "id"),
            field(&path["grant"], "evidence_kind")
        )?;
        if !via.is_empty() {
            writeln!(out, "    Via {}", via.join(" → "))?;
        }
    }
    if let Some(groups) = result["unresolved_group_grants"]
        .as_array()
        .filter(|g| !g.is_empty())
    {
        writeln!(out, "\nGroups with unresolved membership")?;
        for row in groups {
            writeln!(
                out,
                "  {}: {} (grant {})",
                field(&row["group"], "name"),
                field(&row["grant"], "role"),
                field(&row["grant"], "id")
            )?;
        }
    }
    writeln!(
        out,
        "\nSummary\n  {} accounts; {} paths; {} unique source grants",
        result["unique_account_count"], result["path_count"], result["unique_source_grant_count"]
    )?;
    writeln!(
        out,
        "  Observed {} to {}",
        field(result, "observation_started_at"),
        field(result, "observation_completed_at")
    )?;
    if result["provider_complete"] == false {
        writeln!(out, "  Resource provider visibility is incomplete")?;
    }
    Ok(())
}
