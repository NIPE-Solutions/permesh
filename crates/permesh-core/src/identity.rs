// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub type Aliases = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IdentityResolution {
    Resolved { identity: Identity },
    Unmapped,
    Ambiguous { candidates: Vec<String> },
}
/// Exact matching only. Public/unverified addresses are never inserted into this index.
pub(crate) struct IdentityIndex<'a> {
    pub(crate) identities: BTreeMap<String, Identity>,
    pub(crate) conflicting: BTreeSet<String>,
    pub(crate) email_index: BTreeMap<&'a str, BTreeSet<String>>,
    pub(crate) accounts: BTreeMap<EntityKey, &'a Account>,
    pub(crate) candidates: BTreeMap<EntityKey, BTreeSet<String>>,
}
impl<'a> IdentityIndex<'a> {
    pub(crate) fn build(snapshots: &'a [Snapshot], aliases: &Aliases) -> Result<Self, DomainError> {
        Self::build_filtered(snapshots, aliases, None)
    }
    pub(crate) fn build_with_authorities(
        snapshots: &'a [Snapshot],
        aliases: &Aliases,
        authorities: &BTreeSet<&str>,
    ) -> Result<Self, DomainError> {
        Self::build_filtered(snapshots, aliases, Some(authorities))
    }
    fn build_filtered(
        snapshots: &'a [Snapshot],
        aliases: &Aliases,
        authorities: Option<&BTreeSet<&str>>,
    ) -> Result<Self, DomainError> {
        let mut identities: BTreeMap<String, Identity> = BTreeMap::new();
        let mut conflicting = BTreeSet::new();
        let mut email_index: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut accounts = BTreeMap::new();
        let mut provider_ids = BTreeSet::new();
        for snapshot in snapshots {
            if !provider_ids.insert(&snapshot.provider) {
                return Err(DomainError::Identifier);
            }
            snapshot.validate()?;
            for identity in snapshot
                .identities
                .iter()
                .filter(|_| authorities.is_none_or(|ids| ids.contains(snapshot.provider.as_str())))
            {
                let merged = identities
                    .entry(identity.id.clone())
                    .or_insert_with(|| identity.clone());
                if merged.status != identity.status || merged.kind != identity.kind {
                    conflicting.insert(identity.id.clone());
                }
                merged
                    .verified_emails
                    .extend(identity.verified_emails.iter().cloned());
                merged.verified_emails.sort();
                merged.verified_emails.dedup();
                for email in &identity.verified_emails {
                    email_index
                        .entry(email)
                        .or_default()
                        .insert(identity.id.clone());
                }
            }
            for account in &snapshot.accounts {
                if accounts.insert(account.key.clone(), account).is_some() {
                    return Err(DomainError::Identifier);
                }
            }
        }
        let mut candidates: BTreeMap<EntityKey, BTreeSet<String>> = BTreeMap::new();
        for (identity, instances) in aliases {
            identities
                .entry(identity.clone())
                .or_insert_with(|| Identity {
                    id: identity.clone(),
                    kind: IdentityKind::Unknown,
                    status: IdentityStatus::Unknown,
                    verified_emails: vec![],
                });
            for (provider, ids) in instances {
                for id in ids {
                    candidates
                        .entry(EntityKey::new(provider, id))
                        .or_default()
                        .insert(identity.clone());
                }
            }
        }
        for (key, account) in &accounts {
            for email in &account.verified_emails {
                if let Some(matches) = email_index.get(email.as_str()) {
                    candidates
                        .entry(key.clone())
                        .or_default()
                        .extend(matches.iter().cloned());
                }
            }
        }
        Ok(Self {
            identities,
            conflicting,
            email_index,
            accounts,
            candidates,
        })
    }
    pub(crate) fn resolve(&self, key: &EntityKey) -> IdentityResolution {
        let Some(candidates) = self.candidates.get(key) else {
            return IdentityResolution::Unmapped;
        };
        if candidates.len() == 1
            && let Some(id) = candidates.first()
            && !self.conflicting.contains(id)
            && let Some(identity) = self.identities.get(id)
        {
            return IdentityResolution::Resolved {
                identity: identity.clone(),
            };
        }
        IdentityResolution::Ambiguous {
            candidates: candidates.iter().cloned().collect(),
        }
    }
}
