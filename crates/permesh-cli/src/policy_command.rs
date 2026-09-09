// SPDX-License-Identifier: MIT
//! Focused policy review against explicit saved evidence or fresh read-only discovery.
use crate::{
    args::Cli,
    artifact::{self, Artifact, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    report::Outcome,
};
use permesh_core::{self as core, PolicyState};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::PathBuf,
};
use time::OffsetDateTime;
#[path = "policy_config.rs"]
mod config;
use config::{Policy, Rule};
#[derive(Clone, clap::Subcommand)]
pub enum PolicyCommand {
    Check {
        #[arg(long)]
        rules: PathBuf,
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
}
fn state(value: PolicyState) -> &'static str {
    match value {
        PolicyState::Pass => "pass",
        PolicyState::Finding => "finding",
        PolicyState::NotEvaluable => "not_evaluable",
    }
}
fn from_access(rule: core::AccessRule) -> Rule {
    match rule {
        core::AccessRule::InactiveAccess => Rule::InactiveAccess,
        core::AccessRule::ExternalPrivileged => Rule::ExternalPrivileged,
        core::AccessRule::MachineOwner => Rule::MachineOwner,
    }
}
/// Collection and uncertainty take precedence over findings, which remain in the report.
fn exit_code(all_failed: bool, incomplete: bool, findings: usize) -> u8 {
    if all_failed {
        3
    } else if incomplete {
        4
    } else if findings > 0 {
        6
    } else {
        0
    }
}
fn push_check(rows: &mut Vec<Value>, budget: &mut usize, value: Value) -> Result<(), AppError> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|_| AppError::new(5, "Cannot encode policy check"))?
        .len();
    *budget = budget.checked_sub(bytes).ok_or_else(|| {
        AppError::input("Policy report exceeds its 64 MiB check budget; narrow the captured scope")
    })?;
    if rows.len() >= 100_000 {
        return Err(AppError::input(
            "Policy report exceeds 100000 checks; narrow the captured scope",
        ));
    }
    rows.push(value);
    Ok(())
}
fn evaluate(
    policy: &Policy,
    artifact: &Artifact,
    now: OffsetDateTime,
) -> Result<(Value, u8, bool), AppError> {
    policy.validate()?;
    artifact.validate()?;
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let owners: BTreeSet<_> = policy
        .owners
        .iter()
        .map(|o| core::EntityKey::new(&o.instance, &o.account))
        .collect();
    let mut rows = Vec::new();
    let mut row_budget = 64 * 1024 * 1024;
    let checks = if policy.rules.iter().any(|r| r.access().is_some()) {
        core::review_policy_access(
            &snapshots,
            &aliases,
            &artifact.identity.authorities,
            &owners,
        )?
    } else {
        vec![]
    };
    let annotations: BTreeMap<_, _> = policy
        .owners
        .iter()
        .map(|o| ((&o.instance, &o.account), o))
        .collect();
    let exceptions: BTreeMap<_, _> = policy
        .exceptions
        .iter()
        .map(|e| {
            (
                (
                    e.rule,
                    e.instance.as_str(),
                    e.account.as_str(),
                    e.resource.as_str(),
                    e.grant_id.as_str(),
                ),
                e,
            )
        })
        .collect();
    for check in checks {
        let rule = from_access(check.rule);
        if !policy.rules.contains(&rule) {
            continue;
        }
        let exception = check.account.as_ref().and_then(|a| {
            exceptions.get(&(
                rule,
                check.instance.as_str(),
                a.id.as_str(),
                check.resource.id.as_str(),
                check.grant_id.as_str(),
            ))
        });
        let excepted = check.state == PolicyState::Finding
            && exception
                .is_some_and(|e| config::expiry(&e.expires_at).is_ok_and(|expiry| now < expiry));
        let owner = check
            .account
            .as_ref()
            .and_then(|a| annotations.get(&(&a.provider, &a.id)));
        push_check(
            &mut rows,
            &mut row_budget,
            json!({"rule":rule,"state":if excepted{"excepted"}else{state(check.state)},"evidence_state":state(check.state),"instance":check.instance,"account":check.account.as_ref().map(|a|a.id.as_str()),"resource":check.resource.id,"grant_id":check.grant_id,"reason":check.reason,"owner_assertion":owner,"exception":if excepted{exception.copied()}else{None}}),
        )?;
    }
    let access_sources: Vec<_> = artifact
        .providers
        .iter()
        .filter(|p| {
            p.capabilities
                .iter()
                .any(|c| matches!(c.as_str(), "accounts" | "grants" | "resources"))
                || p.state == State::Failed
        })
        .collect();
    for rule in policy
        .rules
        .iter()
        .copied()
        .filter(|r| r.access().is_some())
    {
        let unavailable = access_sources.is_empty()
            || access_sources.iter().any(|p| {
                p.state != State::Complete
                    || !p.capabilities.iter().any(|c| c == "accounts")
                    || !p.capabilities.iter().any(|c| c == "grants")
            })
            || (rule == Rule::InactiveAccess
                && (artifact.identity.authorities.is_empty()
                    || artifact.identity.authorities.iter().any(|id| {
                        !artifact.providers.iter().any(|p| {
                            p.instance == *id
                                && p.state == State::Complete
                                && p.capabilities.iter().any(|c| c == "identities")
                        })
                    })));
        if unavailable {
            push_check(
                &mut rows,
                &mut row_budget,
                json!({"rule":rule,"state":"not_evaluable","reason":"required_observation_or_authority_coverage_unavailable"}),
            )?;
        } else if !rows.iter().any(|r| r["rule"] == json!(rule)) {
            push_check(
                &mut rows,
                &mut row_budget,
                json!({"rule":rule,"state":"pass","reason":"no_matching_access_in_complete_declared_scope"}),
            )?;
        }
    }
    if policy.rules.contains(&Rule::ExpiredException) {
        for exception in &policy.exceptions {
            push_check(
                &mut rows,
                &mut row_budget,
                json!({"rule":Rule::ExpiredException,"state":if now>=config::expiry(&exception.expires_at)?{"finding"}else{"pass"},"reason":"documented_exception_expiry","exception":exception}),
            )?;
        }
        if policy.exceptions.is_empty() {
            push_check(
                &mut rows,
                &mut row_budget,
                json!({"rule":Rule::ExpiredException,"state":"pass","reason":"no_documented_exceptions"}),
            )?;
        }
    }
    let mut coverage_gap = false;
    if policy.rules.contains(&Rule::RequiredCoverage) {
        for required in &policy.required_providers {
            let available = artifact.providers.iter().any(|p| {
                p.instance == required.instance
                    && p.state == State::Complete
                    && required
                        .capabilities
                        .iter()
                        .all(|c| p.capabilities.contains(c))
            });
            coverage_gap |= !available;
            push_check(
                &mut rows,
                &mut row_budget,
                json!({"rule":Rule::RequiredCoverage,"state":if available{"pass"}else{"finding"},"instance":required.instance,"required_capabilities":required.capabilities,"reason":"required_provider_coverage"}),
            )?;
        }
    }
    rows.sort_by_cached_key(Value::to_string);
    let findings = rows.iter().filter(|r| r["state"] == "finding").count();
    let not_evaluable = rows
        .iter()
        .filter(|r| r["state"] == "not_evaluable")
        .count();
    let excepted = rows.iter().filter(|r| r["state"] == "excepted").count();
    let incomplete = !artifact.complete() || coverage_gap || not_evaluable > 0;
    let all_failed = artifact.providers.iter().all(|p| p.state == State::Failed);
    let code = exit_code(all_failed, incomplete, findings);
    let result = json!({"policy_review_version":1,"rules":policy.rules,"checks":rows,"findings":findings,"not_evaluable":not_evaluable,"excepted":excepted,"collection_complete":artifact.complete(),"evaluation_complete":!incomplete,"clean":!incomplete&&findings==0&&excepted==0,"reviewed_at":now.format(&time::format_description::well_known::Rfc3339).map_err(|_|AppError::new(5,"Cannot format review time"))?,"observation_started_at":artifact.started_at,"observation_completed_at":artifact.completed_at,"message":"Read-only checks of the captured declared scope, not proof of effective authorization. Ownership and exception records are local organizational assertions. Exceptions never turn unknown evidence into a pass. Findings remain visible when collection or evaluation is incomplete."});
    Ok((result, code, !incomplete))
}
pub async fn run(
    cli: &Cli,
    command: &PolicyCommand,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    let PolicyCommand::Check { rules, snapshot } = command;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    let path = rules.clone();
    let policy: Policy = pool.run(move || artifact::load_document(&path)).await??;
    policy.validate()?;
    let (mut outcome, artifact) =
        crate::resource_command::input(cli, snapshot.as_deref(), pool, cancel).await?;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    let (result, code, complete) = pool
        .run(move || evaluate(&policy, &artifact, OffsetDateTime::now_utc()))
        .await??;
    if cancel.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    outcome.report.command = "policy_check".into();
    outcome.report.result = result;
    outcome.report.complete = complete;
    outcome.code = code;
    Ok(outcome)
}
pub fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    use crate::output::safe;
    let field = |v: &Value, key: &str| safe(v[key].as_str().unwrap_or_default());
    writeln!(
        out,
        "Policy review\n  {} findings; {} not evaluable; {} accepted exceptions",
        result["findings"], result["not_evaluable"], result["excepted"]
    )?;
    writeln!(
        out,
        "  Observed {} to {}",
        field(result, "observation_started_at"),
        field(result, "observation_completed_at")
    )?;
    for row in result["checks"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "\n{}: {}",
            field(row, "rule").replace('_', " "),
            field(row, "state").replace('_', " ")
        )?;
        if row["instance"].is_string() {
            writeln!(out, "  Instance {}", field(row, "instance"))?;
        }
        if row["account"].is_string() {
            writeln!(
                out,
                "  Account {} / resource {} / grant {}",
                field(row, "account"),
                field(row, "resource"),
                field(row, "grant_id")
            )?;
        }
        writeln!(out, "  {}", field(row, "reason").replace('_', " "))?;
        if let Some(owner) = row.get("owner_assertion").filter(|o| o.is_object()) {
            writeln!(
                out,
                "  Local owner assertion: {} — {}",
                field(owner, "owner"),
                field(owner, "reason")
            )?;
        }
        if let Some(exception) = row.get("exception").filter(|o| o.is_object()) {
            writeln!(
                out,
                "  Exception {} / owner {} / expires {}\n  {}",
                field(exception, "id"),
                field(exception, "owner"),
                field(exception, "expires_at"),
                field(exception, "reason")
            )?;
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
