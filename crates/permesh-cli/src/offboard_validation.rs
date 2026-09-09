// SPDX-License-Identifier: MIT
//! Validate imported advisory documents and their target-only record graph.
use super::model::*;
use crate::{
    artifact::{self, records::*},
    error::AppError,
};
use std::collections::{BTreeMap, BTreeSet};
impl Assessment {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.target.id.is_empty()
            || self.sources.is_empty()
            || self.sources.len() > 256
            || self.paths.len() > 10000
            || self.accounts.len() > 10000
            || self.memberships.len() > 10000
            || self.ownership.len() > 10000
            || self.ownership_findings.len() > 10000
            || self
                .paths
                .iter()
                .map(|p| p.memberships.len().saturating_add(p.groups.len()))
                .sum::<usize>()
                > 100000
            || self.recommendations.len() > 40000
            || self.authoritative_sources.is_empty()
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        for s in &self.sources {
            if s.capture.data.is_some()
                || !ids.insert(&s.capture.instance)
                || !artifact::valid_digest(&s.observation_sha256)
            {
                return Err(invalid());
            }
        }
        if self.authoritative_sources.iter().any(|s| !ids.contains(s)) {
            return Err(invalid());
        }
        let mut accounts = BTreeSet::new();
        for a in &self.accounts {
            if !ids.contains(&a.key.provider) || !accounts.insert((&a.key.provider, &a.key.id)) {
                return Err(invalid());
            }
        }
        let mut paths = BTreeSet::new();
        for p in &self.paths {
            if !accounts.contains(&(&p.account.provider, &p.account.id))
                || p.id != path_id(&p.account, &p.grant, &p.memberships)?
                || !paths.insert(&p.id)
                || p.grant.resource != p.resource.key
                || p.groups.len() != p.memberships.len()
            {
                return Err(invalid());
            }
            let mut subject = Subject::Account(p.account.clone());
            for (g, m) in p.groups.iter().zip(&p.memberships) {
                if m.member != subject || m.group != g.key || g.key.provider != p.account.provider {
                    return Err(invalid());
                }
                subject = Subject::Group(g.key.clone());
            }
            if p.grant.subject != subject || p.resource.key.provider != p.account.provider {
                return Err(invalid());
            }
        }
        for o in &self.ownership {
            time(&o.account_observed_at)?;
            let mut snapshot = permesh_core::Snapshot::new(&o.account.key.provider);
            snapshot.accounts.push((&o.account).into());
            snapshot.validate().map_err(|_| invalid())?;
            if o.identity != self.target.id
                || !ids.contains(&o.account.key.provider)
                || !matches!(o.account.kind, IdentityKind::Service | IdentityKind::Bot)
            {
                return Err(invalid());
            }
        }
        for finding in &self.ownership_findings {
            if finding.observed_owner_accounts == 0
                || finding.sole_observed_owner != (finding.observed_owner_accounts == 1)
                || !self.paths.iter().any(|p| {
                    p.resource.key == finding.resource && p.grant.privilege == Privilege::Owner
                })
            {
                return Err(invalid());
            }
        }
        let steps: BTreeSet<_> = self.recommendations.iter().map(|r| &r.id).collect();
        if steps.len() != self.recommendations.len() {
            return Err(invalid());
        }
        for r in &self.recommendations {
            if !artifact::valid_digest(&r.id)
                || !ids.contains(&r.account.provider)
                || r.prerequisites.iter().any(|id| !steps.contains(id))
            {
                return Err(invalid());
            }
        }
        if hash(&self.recommendations)? != hash(&super::recommendations(self)?)? {
            return Err(invalid());
        }
        // Revalidate the target-only graph and source summaries using the artifact contract.
        // Resource ancestors outside the selected paths are not claimed by this document.
        for source in &self.sources {
            let p = &source.capture;
            let mut data = Dataset {
                provider: p.instance.clone(),
                identities: vec![],
                accounts: vec![],
                resources: vec![],
                groups: vec![],
                memberships: vec![],
                grants: vec![],
                limitations: p.limitations.clone(),
                complete: p.state == artifact::State::Complete,
            };
            let mut accounts_index = BTreeMap::new();
            let mut resources_index = BTreeMap::new();
            let mut groups_index = BTreeMap::new();
            let mut members_index = BTreeMap::new();
            let mut grants_index = BTreeMap::new();
            for account in self
                .accounts
                .iter()
                .filter(|a| a.key.provider == p.instance)
            {
                add(
                    &mut data.accounts,
                    &mut accounts_index,
                    account.clone(),
                    hash(&account.key)?,
                )?;
            }
            if self.authoritative_sources.contains(&p.instance) {
                data.identities.push(self.target.clone());
            }
            for path in self
                .paths
                .iter()
                .filter(|path| path.account.provider == p.instance)
            {
                let mut resource = path.resource.clone();
                resource.parent = None;
                let id = hash(&resource.key)?;
                add(&mut data.resources, &mut resources_index, resource, id)?;
                for group in &path.groups {
                    add(
                        &mut data.groups,
                        &mut groups_index,
                        group.clone(),
                        hash(&group.key)?,
                    )?;
                }
                for member in &path.memberships {
                    add(
                        &mut data.memberships,
                        &mut members_index,
                        member.clone(),
                        hash(&(&member.member, &member.group))?,
                    )?;
                }
                add(
                    &mut data.grants,
                    &mut grants_index,
                    path.grant.clone(),
                    path.grant.id.clone(),
                )?;
                let expected = if path.grant.certainty == Certainty::Observed
                    && !path.memberships.is_empty()
                {
                    Certainty::Derived
                } else {
                    path.grant.certainty
                };
                if path.certainty != expected {
                    return Err(invalid());
                }
            }
            for m in self
                .memberships
                .iter()
                .filter(|m| m.group.provider == p.instance)
            {
                add(
                    &mut data.memberships,
                    &mut members_index,
                    m.clone(),
                    hash(&(&m.member, &m.group))?,
                )?;
                let id = hash(&m.group)?;
                if !groups_index.contains_key(&id) {
                    add(
                        &mut data.groups,
                        &mut groups_index,
                        Group {
                            key: m.group.clone(),
                            name: m.group.id.clone(),
                        },
                        id,
                    )?;
                }
            }
            let mut capture = p.clone();
            capture.data = if p.state == artifact::State::Failed {
                if !data.accounts.is_empty()
                    || !data.identities.is_empty()
                    || !data.grants.is_empty()
                {
                    return Err(invalid());
                }
                None
            } else {
                Some(data)
            };
            let authorities = if self.authoritative_sources.contains(&p.instance) {
                vec![p.instance.clone()]
            } else {
                vec![]
            };
            artifact::Artifact {
                format: "permesh_snapshot".into(),
                format_version: 1,
                producer_version: "offboard-validation".into(),
                started_at: p.started_at.clone(),
                completed_at: p.completed_at.clone(),
                identity: artifact::IdentityContext {
                    authorities,
                    bindings: vec![],
                },
                providers: vec![capture],
            }
            .validate()
            .map_err(|_| invalid())?;
        }
        Ok(())
    }
}
impl Plan {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.format != "permesh_offboard_plan"
            || self.format_version != 1
            || !artifact::valid_digest(&self.source_snapshot_sha256)
            || !artifact::valid_digest(&self.identity_context_sha256)
        {
            return Err(invalid());
        }
        if self.max_age_hours == 0 || self.max_age_hours > 168 {
            return Err(invalid());
        }
        let basis = evidence_start(self.assessment.sources.iter().map(|s| &s.capture))?;
        if time(&self.expires_at)? != basis + time::Duration::hours(i64::from(self.max_age_hours)) {
            return Err(invalid());
        }
        let created = time(&self.created_at)?;
        if created > time::OffsetDateTime::now_utc() {
            return Err(invalid());
        }
        for source in &self.assessment.sources {
            if time(&source.capture.completed_at)? > created {
                return Err(invalid());
            }
        }
        let duration = time(&self.expires_at)? - created;
        if duration <= time::Duration::ZERO || duration > time::Duration::days(7) {
            return Err(invalid());
        }
        self.assessment.validate()
    }
}
impl Report {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.format != "permesh_offboard_report" || self.format_version != 1 {
            return Err(invalid());
        }
        time(&self.generated_at)?;
        self.assessment.validate()
    }
}

fn add<T: PartialEq>(
    items: &mut Vec<T>,
    index: &mut BTreeMap<String, usize>,
    value: T,
    key: String,
) -> Result<(), AppError> {
    if let Some(position) = index.get(&key) {
        if items[*position] != value {
            return Err(invalid());
        }
    } else {
        index.insert(key, items.len());
        items.push(value);
    }
    Ok(())
}
