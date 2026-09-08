// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{EntityKey, Snapshot, Subject};
use std::collections::BTreeSet;

pub const MAX_SNAPSHOT_RECORDS: usize = 1_000_000;
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("provider snapshot exceeds the record limit")]
    Limit,
    #[error(
        "provider snapshot contains invalid observation provenance; UTC RFC3339 time is required"
    )]
    Provenance,
    #[error("provider snapshot contains duplicate, empty or cross-provider identifiers")]
    Identifier,
    #[error("provider snapshot contains a dangling relationship")]
    Reference,
    #[error("access path traversal exceeds the safety limit")]
    PathLimit,
    #[error("identity lookup is ambiguous; configure an explicit provider account mapping")]
    Ambiguous,
    #[error("no matching identity or provider account; configure an explicit alias if needed")]
    NotFound,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self
            .accounts
            .len()
            .saturating_add(self.groups.len())
            .saturating_add(self.resources.len())
            .saturating_add(self.grants.len())
            .saturating_add(self.memberships.len())
            .saturating_add(self.identities.len())
            > MAX_SNAPSHOT_RECORDS
        {
            return Err(DomainError::Limit);
        }
        if self.provider.is_empty() {
            return Err(DomainError::Identifier);
        }
        let keys = |items: Vec<&EntityKey>| -> Result<BTreeSet<EntityKey>, DomainError> {
            let mut result = BTreeSet::new();
            for key in items {
                if key.provider != self.provider || key.id.is_empty() || !result.insert(key.clone())
                {
                    return Err(DomainError::Identifier);
                }
            }
            Ok(result)
        };
        let accounts = keys(self.accounts.iter().map(|v| &v.key).collect())?;
        let groups = keys(self.groups.iter().map(|v| &v.key).collect())?;
        let resources = keys(self.resources.iter().map(|v| &v.key).collect())?;
        let has_subject = |s: &Subject| match s {
            Subject::Account(k) => accounts.contains(k),
            Subject::Group(k) => groups.contains(k),
        };
        let mut grant_ids = BTreeSet::new();
        let mut identity_ids = BTreeSet::new();
        for identity in &self.identities {
            if identity.id.is_empty() || !identity_ids.insert(&identity.id) {
                return Err(DomainError::Identifier);
            }
        }
        for grant in &self.grants {
            validate_provenance(&grant.provenance)?;
            if grant.id.is_empty() || !grant_ids.insert(&grant.id) {
                return Err(DomainError::Identifier);
            }
            if !has_subject(&grant.subject) || !resources.contains(&grant.resource) {
                return Err(DomainError::Reference);
            }
        }
        let mut memberships = BTreeSet::new();
        for membership in &self.memberships {
            validate_provenance(&membership.provenance)?;
            if !has_subject(&membership.member) || !groups.contains(&membership.group) {
                return Err(DomainError::Reference);
            }
            if !memberships.insert((&membership.member, &membership.group)) {
                return Err(DomainError::Identifier);
            }
        }
        Ok(())
    }
}

fn validate_provenance(provenance: &crate::Provenance) -> Result<(), DomainError> {
    if provenance.method.trim().is_empty() {
        return Err(DomainError::Provenance);
    }
    let timestamp = time::OffsetDateTime::parse(
        &provenance.observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| DomainError::Provenance)?;
    if !timestamp.offset().is_utc() {
        return Err(DomainError::Provenance);
    }
    Ok(())
}
