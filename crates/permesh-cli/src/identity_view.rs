// SPDX-License-Identifier: MIT
//! CLI-owned identity-review DTOs and safe human rendering.
use permesh_config::Config;
use permesh_core::{
    identity_review::{AccountEvidence, CanonicalIdentity},
    *,
};
use serde_json::{Value, json};
use std::io::{self, Write};
fn kind(value: IdentityKind) -> &'static str {
    match value {
        IdentityKind::Human => "human",
        IdentityKind::Service => "service",
        IdentityKind::Bot => "bot",
        IdentityKind::Unknown => "unknown",
    }
}
fn status(value: IdentityStatus) -> &'static str {
    match value {
        IdentityStatus::Active => "active",
        IdentityStatus::Inactive => "inactive",
        IdentityStatus::Suspended => "suspended",
        IdentityStatus::Unknown => "unknown",
    }
}
fn affiliation(value: Affiliation) -> &'static str {
    match value {
        Affiliation::Internal => "internal",
        Affiliation::External => "external",
        Affiliation::Unknown => "unknown",
    }
}
pub(super) fn identity(target: &CanonicalIdentity, config: &Config) -> Value {
    let authorities: Vec<_> = target.sources.iter().map(|instance| {
        let scope = config.providers.iter().find(|p| p.id == *instance).map(crate::review_scope::configured).unwrap_or_default();
        json!({"instance":instance,"configured_scope":scope,"scope_independently_verified":false})
    }).collect();
    json!({"id":target.identity.id,"kind":kind(target.identity.kind),"status":status(target.identity.status),"affiliation":affiliation(target.identity.affiliation),"verified_emails":target.identity.verified_emails,"authoritative_sources":target.sources,"authorities":authorities,"conflicting":target.conflicting})
}
pub(super) fn account(entry: &AccountEvidence, config: &Config) -> Value {
    let resolution = match &entry.resolution {
        IdentityResolution::Unmapped => json!({"state":"unmapped"}),
        IdentityResolution::Resolved { identity } => {
            json!({"state":"resolved","canonical_identity_id":identity.id})
        }
        IdentityResolution::Ambiguous { candidates } => {
            json!({"state":"ambiguous","canonical_candidates":candidates})
        }
    };
    let scope = config
        .providers
        .iter()
        .find(|p| p.id == entry.account.key.provider)
        .map(crate::review_scope::configured)
        .unwrap_or_default();
    json!({"instance":entry.account.key.provider,"account_id":entry.account.key.id,"login":entry.account.login,"kind":kind(entry.account.kind),"status":status(entry.account.status),"affiliation":affiliation(entry.account.affiliation),"verified_emails":entry.account.verified_emails,"explicit_mappings":entry.explicit_mappings,"verified_identity_matches":entry.verified_matches,"resolution":resolution,"configured_scope":scope,"scope_independently_verified":false})
}
pub(super) fn mappings(aliases: &Aliases, key: &EntityKey) -> Value {
    let identities: Vec<_> = aliases
        .iter()
        .filter(|(_, instances)| {
            instances
                .get(&key.provider)
                .is_some_and(|ids| ids.contains(&key.id))
        })
        .map(|(identity, _)| identity)
        .collect();
    json!({"instance":key.provider,"account_id":key.id,"canonical_identity_ids":identities})
}
fn block(out: &mut impl Write, value: &Value) -> io::Result<()> {
    for line in serde_json::to_string_pretty(value)
        .map_err(io::Error::other)?
        .lines()
    {
        writeln!(out, "{}", crate::output::safe(line))?;
    }
    Ok(())
}
pub(super) fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    use crate::output::safe;
    if let Some(change) = result.get("change") {
        writeln!(
            out,
            "Identity mapping {}",
            if result["applied"] == true {
                "saved"
            } else {
                "proposal"
            }
        )?;
        block(out, change)?;
        writeln!(out, "\nReviewed collection scope limitations")?;
        block(out, &result["collection_scope"])?;
        writeln!(
            out,
            "\nFingerprint: {}\nOriginal file SHA-256: {}\nProposed file SHA-256: {}",
            safe(result["fingerprint"].as_str().unwrap_or_default()),
            safe(result["original_sha256"].as_str().unwrap_or_default()),
            safe(result["proposed_sha256"].as_str().unwrap_or_default())
        )?;
    } else {
        writeln!(out, "Account identity evidence")?;
        for account in result["accounts"].as_array().into_iter().flatten() {
            block(out, account)?;
        }
        writeln!(out, "\nObserved authoritative canonical targets")?;
        for target in result["canonical_identities"]
            .as_array()
            .into_iter()
            .flatten()
        {
            block(out, target)?;
        }
        if let Some(mapping) = result
            .get("explicit_mappings")
            .filter(|value| !value.is_null())
        {
            writeln!(out, "\nExplicit mappings")?;
            block(out, mapping)?;
        }
    }
    Ok(())
}
