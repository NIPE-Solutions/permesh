// SPDX-License-Identifier: MIT
//! Explicit identity review and revision-bound local alias changes.
use crate::{
    args::Cli, blocking::BlockingPool, cancellation::Cancellation, error::AppError, report::Outcome,
};
use clap::Subcommand;
use permesh_config::Config;
use permesh_core::{EntityKey, Snapshot, identity_review};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    path::PathBuf,
};
#[path = "identity_view.rs"]
mod view;

#[derive(Clone, Subcommand)]
pub enum IdentityCommand {
    /// Fresh discovery of unresolved accounts and authoritative canonical identities.
    Unresolved,
    /// Inspect exact stable account evidence and configured mappings.
    Inspect { instance: String, account: String },
    /// Propose an exact local alias; repeat with the reviewed fingerprint to save.
    Map {
        instance: String,
        account: String,
        #[arg(long)]
        identity: String,
        #[arg(long)]
        fingerprint: Option<String>,
    },
    /// Propose removal of an exact local alias, including a deleted account's stale alias.
    Unmap {
        instance: String,
        account: String,
        #[arg(long)]
        identity: String,
        #[arg(long)]
        fingerprint: Option<String>,
    },
}
impl IdentityCommand {
    fn selected(&self) -> Option<EntityKey> {
        match self {
            Self::Unresolved => None,
            Self::Inspect { instance, account }
            | Self::Map {
                instance, account, ..
            }
            | Self::Unmap {
                instance, account, ..
            } => Some(EntityKey::new(instance, account)),
        }
    }
    fn change(&self) -> Option<(&str, bool, Option<&str>)> {
        match self {
            Self::Map {
                identity,
                fingerprint,
                ..
            } => Some((identity, false, fingerprint.as_deref())),
            Self::Unmap {
                identity,
                fingerprint,
                ..
            } => Some((identity, true, fingerprint.as_deref())),
            _ => None,
        }
    }
}
fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
struct Captured {
    path: PathBuf,
    original: Vec<u8>,
    config: Config,
}
impl Captured {
    fn load(cli: &Cli) -> Result<Self, AppError> {
        let path = crate::workspace::path(cli)?;
        let original = crate::workspace::source(&path)?;
        let config = Config::from_bytes(&original)?;
        let path = path
            .canonicalize()
            .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
        Ok(Self {
            path,
            original,
            config,
        })
    }
}
pub async fn run(
    cli: &Cli,
    command: &IdentityCommand,
    blocking: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancellation)?;
    let captured = Captured::load(cli)?;
    if let Some(key) = command.selected() {
        if !captured
            .config
            .providers
            .iter()
            .any(|p| p.id == key.provider)
        {
            return Err(AppError::input(
                "Unknown provider instance; run permesh provider list",
            ));
        }
        if key.id.is_empty() || key.id.len() > 1024 || key.id.chars().any(char::is_control) {
            return Err(AppError::input(
                "Use an exact immutable native account ID from identity unresolved",
            ));
        }
    }
    if let Some((_, _, Some(fingerprint))) = command.change()
        && (fingerprint.len() != 64
            || !fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    {
        return Err(AppError::input(
            "Use the exact reviewed mapping fingerprint",
        ));
    }
    let (outcome, snapshots) = crate::collection::collect(
        blocking,
        &captured.config,
        &captured.path,
        cancellation,
        captured.config.providers.clone(),
        true,
        "identity_review",
    )
    .await?;
    finish(captured, command, outcome, &snapshots, cancellation)
}
fn finish(
    captured: Captured,
    command: &IdentityCommand,
    mut outcome: Outcome,
    snapshots: &[Snapshot],
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancellation)?;
    let authorities: Vec<_> = captured
        .config
        .identity
        .sources
        .iter()
        .filter(|s| s.authoritative)
        .map(|s| s.provider.clone())
        .collect();
    let review =
        identity_review::review(snapshots, &captured.config.identity.aliases, &authorities)?;
    let selected = command.selected();
    let rows: Vec<_> = review
        .accounts
        .iter()
        .filter(|entry| {
            selected.as_ref().map_or_else(
                || {
                    !matches!(
                        entry.resolution,
                        permesh_core::IdentityResolution::Resolved { .. }
                    )
                },
                |key| entry.account.key == *key,
            )
        })
        .map(|entry| view::account(entry, &captured.config))
        .collect();
    let targets: Vec<_> = review
        .identities
        .iter()
        .map(|target| view::identity(target, &captured.config))
        .collect();
    let selected_mappings = selected
        .as_ref()
        .map(|key| view::mappings(&captured.config.identity.aliases, key));
    outcome.report.command = "identity_review".into();
    outcome.report.result = json!({"identity_review_version":1,"accounts":rows,"canonical_identities":targets,"explicit_mappings":selected_mappings,"collection_complete":outcome.report.complete,"message":"Fresh read-only identity evidence. Labels are not mapping keys. Canonical targets come only from explicitly authoritative sources; unknown scope and conflicting evidence remain unresolved."});
    let Some((identity, remove, expected)) = command.change() else {
        return Ok(outcome);
    };
    if !outcome.report.complete
        || snapshots.len() != captured.config.providers.len()
        || snapshots.iter().any(|s| !s.complete)
    {
        return Err(AppError::new(
            3,
            "Mapping changes require complete fresh discovery from every configured provider; resolve provider failures and incomplete observations first",
        ));
    }
    let key = selected.ok_or_else(|| AppError::new(5, "Mapping account is missing"))?;
    let proposed_aliases = identity_review::change(
        snapshots,
        &captured.config.identity.aliases,
        &authorities,
        &key,
        identity,
        remove,
    )
    .map_err(|error| AppError::input(error.to_string()))?;
    let mut proposed = captured.config.clone();
    proposed.identity.aliases = proposed_aliases;
    let proposed_bytes = permesh_config::to_yaml(&proposed)?.into_bytes();
    Config::from_bytes(&proposed_bytes)?;
    let after = identity_review::review(snapshots, &proposed.identity.aliases, &authorities)?;
    let before_account = review
        .accounts
        .iter()
        .find(|a| a.account.key == key)
        .map(|entry| view::account(entry, &captured.config));
    let after_account = after
        .accounts
        .iter()
        .find(|a| a.account.key == key)
        .map(|entry| view::account(entry, &proposed));
    let target = review
        .identities
        .iter()
        .find(|i| i.identity.id == identity)
        .map(|target| view::identity(target, &captured.config));
    let change = json!({"operation":if remove {"remove"} else {"add"}, "instance":key.provider,"account_id":key.id,"canonical_identity_id":identity,"before":view::mappings(&captured.config.identity.aliases,&key),"after":view::mappings(&proposed.identity.aliases,&key),"account_before":before_account,"account_after":after_account,"canonical_target":target});
    let collection_scope: std::collections::BTreeMap<_, _> = snapshots
        .iter()
        .map(|snapshot| {
            let mut limitations = snapshot.limitations.clone();
            limitations.sort();
            limitations.dedup();
            (&snapshot.provider, limitations)
        })
        .collect();
    let original_sha256 = sha256(&captured.original);
    let proposed_sha256 = sha256(&proposed_bytes);
    // Bind semantic evidence, stable IDs, canonical workspace and exact bytes;
    // capture timestamps/provenance are intentionally excluded so fresh revalidation can match.
    let binding = json!({"mapping_proposal_version":1,"workspace":captured.path,"original_sha256":original_sha256,"proposed_sha256":proposed_sha256,"change":change,"collection_scope":collection_scope});
    let fingerprint = sha256(
        &serde_json::to_vec(&binding)
            .map_err(|_| AppError::new(5, "Cannot fingerprint mapping proposal"))?,
    );
    let applied = if let Some(expected) = expected {
        if expected != fingerprint {
            return Err(AppError::input(
                "Mapping proposal changed; inspect fresh evidence and approve the new fingerprint before saving",
            ));
        }
        cancelled(cancellation)?;
        crate::workspace::replace(&captured.path, &captured.original, &proposed_bytes)?;
        true
    } else {
        false
    };
    outcome.report.command = "identity_mapping".into();
    outcome.report.result = json!({"identity_review_version":1,"proposal_version":1,"workspace":captured.path,"applied":applied,"fingerprint":fingerprint,"original_sha256":original_sha256,"proposed_sha256":proposed_sha256,"change":change,"collection_scope":collection_scope,"formatting_will_be_normalized":true,"comments_will_be_removed":true,"message":if applied {"Exact reviewed mapping saved. Configuration formatting was normalized and comments removed; review the file diff. Relevant external provider execution approvals are now stale and require explicit review."} else {"No file changed. Review this exact alias change and file digests. Saving normalizes the complete configuration and removes comments while preserving unrelated values. Repeat the same command with --fingerprint FINGERPRINT to accept the change and formatting effects."},"next":if applied {format!("permesh provider external review {}",key.provider)}else{format!("Repeat this command with --fingerprint {fingerprint}")}});
    Ok(outcome)
}
pub fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    view::write(out, result)
}
#[cfg(test)]
#[path = "identity_command_tests.rs"]
mod tests;
