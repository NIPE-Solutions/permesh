// SPDX-License-Identifier: MIT
use crate::{
    args::Cli,
    artifact::{self, Artifact, Binding, IdentityContext, ProviderCapture, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    collection,
    error::AppError,
    report::Outcome,
};
use clap::Subcommand;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Subcommand)]
pub enum SnapshotCommand {
    /// Explicitly save access observations to a private local file.
    Create {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Inspect a saved snapshot offline; no workspace or credentials required.
    Inspect { file: PathBuf },
}
fn serialized<T: serde::Serialize>(v: &T) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(v).map_err(|_| AppError::new(5, "Cannot format snapshot result"))
}
pub async fn capture(
    cli: &Cli,
    blocking: &BlockingPool,
    cancel: &Cancellation,
) -> Result<(Outcome, Artifact), AppError> {
    let path = crate::workspace::path(cli)?
        .canonicalize()
        .map_err(|_| AppError::input("Cannot locate workspace"))?;
    let original = crate::workspace::source(&path)?;
    let config = permesh_config::Config::from_bytes(&original)?;
    if config.providers.is_empty() {
        return Err(AppError::input(
            "No providers configured; initialize the demo or add a provider",
        ));
    }
    // Capture available metadata before discovery; failed trust remains a provider failure.
    let metadata: BTreeMap<_, _> = config
        .providers
        .iter()
        .map(|p| (p.id.clone(), collection::metadata(p).ok()))
        .collect();
    let (outcome, snapshots) = collection::collect(
        blocking,
        &config,
        &path,
        cancel,
        config.providers.clone(),
        true,
        "snapshot_create",
    )
    .await?;
    if crate::workspace::source(&path)? != original {
        return Err(AppError::input(
            "Workspace changed during capture; no snapshot was written",
        ));
    }
    let snapshots: BTreeMap<_, _> = snapshots
        .into_iter()
        .map(|s| (s.provider.clone(), s))
        .collect();
    let mut providers = Vec::new();
    for p in &config.providers {
        let status = outcome
            .report
            .providers
            .iter()
            .find(|s| s.id == p.id)
            .ok_or_else(|| AppError::new(5, "Missing provider collection outcome"))?;
        let metadata = metadata.get(&p.id).and_then(Option::as_ref);
        let capabilities = metadata
            .map(|m| {
                m.capabilities
                    .iter()
                    .map(|c| {
                        match c {
                            permesh_provider_sdk::Capability::Accounts => "accounts",
                            permesh_provider_sdk::Capability::Identities => "identities",
                            permesh_provider_sdk::Capability::Resources => "resources",
                            permesh_provider_sdk::Capability::Groups => "groups",
                            permesh_provider_sdk::Capability::Memberships => "memberships",
                            permesh_provider_sdk::Capability::Grants => "grants",
                        }
                        .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let executable_sha256 = p
            .external
            .as_ref()
            .and_then(|e| {
                e.digest_for_target(permesh_provider_sdk::target::native_target())
                    .ok()
            })
            .map(str::to_owned);
        let source_observation =
            if p.kind == permesh_config::ProviderKind::Inventory && snapshots.contains_key(&p.id) {
                let provider = p.clone();
                let workspace = path.clone();
                let observation = blocking
                    .run(move || crate::inventory::observe(&provider, &workspace))
                    .await??;
                Some(artifact::SourceObservation::FileInventory {
                    declared_scope: observation.scope,
                    exported_at: observation.exported_at,
                    content_sha256: observation.content_sha256,
                })
            } else {
                None
            };
        // The reviewed inventory digest pins data, not collection scope. Bind its
        // declared scope separately so a newer export can be compared honestly.
        let mut context = serde_json::to_value(p)
            .map_err(|_| AppError::new(5, "Cannot bind collection context"))?;
        if p.kind == permesh_config::ProviderKind::Inventory {
            if let Some(inventory) = context
                .get_mut("inventory")
                .and_then(serde_json::Value::as_object_mut)
            {
                inventory.remove("sha256");
            }
            if let Some(artifact::SourceObservation::FileInventory { declared_scope, .. }) =
                &source_observation
            {
                context["inventory_declared_scope"] = declared_scope.clone().into();
            }
        }
        let context = serde_json::to_vec(&context)
            .map_err(|_| AppError::new(5, "Cannot bind collection context"))?;
        providers.push(ProviderCapture {
            source_observation,
            instance: p.id.clone(),
            provider_type: p
                .external
                .as_ref()
                .map(|e| e.provider.clone())
                .unwrap_or_else(|| collection::kind(p).into()),
            provider_version: if matches!(
                p.kind,
                permesh_config::ProviderKind::Demo | permesh_config::ProviderKind::Inventory
            ) {
                Some(env!("CARGO_PKG_VERSION").into())
            } else {
                None
            },
            executable_sha256,
            capabilities,
            context_sha256: artifact::digest(&context),
            configured_scope: crate::review_scope::configured(p),
            started_at: outcome.report.started_at.clone(),
            completed_at: outcome.report.completed_at.clone(),
            failure_code: (status.state == "failed")
                .then_some(artifact::FailureCode::ProviderCollectionFailed),
            state: match status.state.as_str() {
                "connected" => State::Complete,
                "partial" => State::Partial,
                _ => State::Failed,
            },
            limitations: status.limitations.clone(),
            data: snapshots.get(&p.id).map(Into::into),
        });
    }
    let mut bindings = Vec::new();
    for (identity, instances) in &config.identity.aliases {
        for (instance, accounts) in instances {
            for account in accounts {
                bindings.push(Binding {
                    identity: identity.clone(),
                    instance: instance.clone(),
                    account: account.clone(),
                });
            }
        }
    }
    let mut artifact = Artifact {
        format: "permesh_snapshot".into(),
        format_version: 1,
        producer_version: env!("CARGO_PKG_VERSION").into(),
        started_at: outcome.report.started_at.clone(),
        completed_at: outcome.report.completed_at.clone(),
        identity: IdentityContext {
            authorities: config
                .identity
                .sources
                .iter()
                .filter(|s| s.authoritative)
                .map(|s| s.provider.clone())
                .collect(),
            bindings,
        },
        providers,
    };
    artifact.validate()?;
    artifact.normalize();
    Ok((outcome, artifact))
}
pub async fn run(
    cli: &Cli,
    command: &SnapshotCommand,
    blocking: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    match command {
        SnapshotCommand::Create { output, overwrite } => {
            artifact::check_destination(output, *overwrite)?;
            if let Ok(existing) = output.canonicalize()
                && existing
                    == crate::workspace::path(cli)?
                        .canonicalize()
                        .map_err(|_| AppError::input("Cannot locate workspace"))?
            {
                return Err(AppError::input(
                    "Snapshot output must not replace workspace configuration",
                ));
            }
            let (mut outcome, artifact) = capture(cli, blocking, cancel).await?;
            if cancel.is_cancelled() {
                return Err(AppError::new(130, "Cancelled"));
            }
            let destination = output.clone();
            let overwrite = *overwrite;
            let token = cancel.clone();
            let count = artifact.providers.len();
            blocking
                .run(move || artifact::write(&destination, &artifact, overwrite, &token))
                .await??;
            outcome.report.result = serde_json::json!({"format_version":1,"file":output,"providers":count,"message":"Saved sensitive access metadata locally. Collection limitations remain part of the snapshot."});
            Ok(outcome)
        }
        SnapshotCommand::Inspect { file } => {
            let file = file.clone();
            let artifact = blocking.run(move || artifact::load(&file)).await??;
            let mut outcome = Outcome::new("snapshot_inspect", serialized(&artifact)?)?;
            outcome.report.complete = artifact.complete();
            outcome.code = if artifact.complete() { 0 } else { 4 };
            Ok(outcome)
        }
    }
}
pub fn compare(before: &Path, after: &Path) -> Result<Outcome, AppError> {
    let a = artifact::load(before)?;
    let b = artifact::load(after)?;
    let comparison = artifact::diff::compare(&a, &b)?;
    let mut outcome = Outcome::new("diff", serialized(&comparison)?)?;
    outcome.report.complete = comparison.inconclusive.is_empty();
    outcome.code = if outcome.report.complete { 0 } else { 4 };
    Ok(outcome)
}
pub fn write(
    out: &mut dyn std::io::Write,
    command: &str,
    result: &serde_json::Value,
) -> std::io::Result<()> {
    use crate::output::{field, safe};
    if command == "snapshot_inspect" {
        writeln!(
            out,
            "Saved access observations\n  Captured: {} to {}",
            safe(field(result, "started_at")),
            safe(field(result, "completed_at"))
        )?;
        if let Some(providers) = result["providers"].as_array() {
            for p in providers {
                writeln!(
                    out,
                    "  {}: {}",
                    safe(field(p, "instance")),
                    safe(field(p, "state"))
                )?;
            }
        }
        writeln!(
            out,
            "\nUse --json to inspect complete records, scope and limitations."
        )?;
    } else if command == "diff" {
        writeln!(out, "Observed changes")?;
        if let Some(changes) = result["changes"].as_array() {
            for c in changes {
                writeln!(
                    out,
                    "  {} / {} / {}\n    {}",
                    safe(field(c, "instance")),
                    safe(field(c, "entity_kind")),
                    safe(field(c, "id")),
                    safe(field(c, "change"))
                )?;
            }
            if changes.is_empty() {
                writeln!(out, "  No record changes.")?;
            }
        }
        if result["identity_context_changed"] == true {
            writeln!(out, "\nIdentity mapping or authority context changed.")?;
        }
        if let Some(gaps) = result["inconclusive"].as_array() {
            for gap in gaps {
                writeln!(
                    out,
                    "\nInconclusive: {}\n  {}",
                    safe(field(gap, "instance")),
                    safe(field(gap, "reason"))
                )?;
            }
        }
        writeln!(
            out,
            "\nAbsence is not proof of revoked access. Use --json for before/after evidence."
        )?;
    }
    Ok(())
}
