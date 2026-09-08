// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_PATH_STEPS: usize = 100_000;
const MAX_GROUP_DEPTH: usize = 256;
pub(crate) fn paths<'a>(
    snapshots: &[Snapshot],
    accounts: impl IntoIterator<Item = &'a Account>,
    privileged_only: bool,
) -> Result<Vec<AccessPath>, DomainError> {
    let mut grants: BTreeMap<&Subject, Vec<&Grant>> = BTreeMap::new();
    let mut edges: BTreeMap<&Subject, Vec<&Membership>> = BTreeMap::new();
    let mut groups = BTreeMap::new();
    let mut resources = BTreeMap::new();
    for s in snapshots {
        for g in &s.grants {
            if !privileged_only || g.privilege != Privilege::Standard {
                grants.entry(&g.subject).or_default().push(g);
            }
        }
        for m in &s.memberships {
            edges.entry(&m.member).or_default().push(m);
        }
        for g in &s.groups {
            groups.insert(&g.key, g);
        }
        for r in &s.resources {
            resources.insert(&r.key, r);
        }
    }
    // Reverse reachability visits each edge once and avoids enumerating paths that
    // cannot lead to a selected grant. The user query retains its existing limits.
    let mut relevant = BTreeSet::new();
    if privileged_only {
        let mut children: BTreeMap<Subject, Vec<&Subject>> = BTreeMap::new();
        for s in snapshots {
            for m in &s.memberships {
                children
                    .entry(Subject::Group(m.group.clone()))
                    .or_default()
                    .push(&m.member);
            }
        }
        let mut pending: Vec<Subject> = grants.keys().map(|s| (*s).clone()).collect();
        while let Some(subject) = pending.pop() {
            if relevant.insert(subject.clone())
                && let Some(members) = children.get(&subject)
            {
                pending.extend(members.iter().map(|member| (*member).clone()));
            }
        }
        edges.retain(|subject, memberships| {
            if !relevant.contains(*subject) {
                return false;
            }
            memberships.retain(|m| relevant.contains(&Subject::Group(m.group.clone())));
            !memberships.is_empty()
        });
    }
    for e in edges.values_mut() {
        e.sort_by(|a, b| a.group.cmp(&b.group));
    }
    let mut result = Vec::new();
    let mut steps = 0usize;
    for account in accounts {
        if privileged_only && !relevant.contains(&Subject::Account(account.key.clone())) {
            continue;
        }
        let mut stack = vec![(
            Subject::Account(account.key.clone()),
            Vec::<Group>::new(),
            Vec::<Membership>::new(),
        )];
        while let Some((subject, via, memberships)) = stack.pop() {
            steps += 1;
            if steps > MAX_PATH_STEPS || via.len() > MAX_GROUP_DEPTH {
                return Err(DomainError::PathLimit);
            }
            if let Some(subject_grants) = grants.get(&subject) {
                for grant in subject_grants {
                    if result.len() >= MAX_PATH_STEPS {
                        return Err(DomainError::PathLimit);
                    }
                    let resource = resources
                        .get(&grant.resource)
                        .ok_or(DomainError::Reference)?;
                    result.push(AccessPath {
                        account: account.key.clone(),
                        groups: via.clone(),
                        memberships: memberships.clone(),
                        resource: (*resource).clone(),
                        grant: (*grant).clone(),
                    });
                }
            }
            if let Some(parents) = edges.get(&subject) {
                for membership in parents {
                    if via.iter().any(|g| g.key == membership.group) {
                        continue;
                    }
                    if stack.len() >= MAX_PATH_STEPS {
                        return Err(DomainError::PathLimit);
                    }
                    let group = groups
                        .get(&membership.group)
                        .ok_or(DomainError::Reference)?;
                    let mut next = via.clone();
                    next.push((*group).clone());
                    let mut next_memberships = memberships.clone();
                    next_memberships.push((*membership).clone());
                    stack.push((
                        Subject::Group(membership.group.clone()),
                        next,
                        next_memberships,
                    ));
                }
            }
        }
    }
    Ok(result)
}

pub(crate) fn sort_paths(access: &mut [AccessPath]) {
    access.sort_by(|a, b| {
        (
            &a.account,
            &a.resource.key,
            &a.grant.id,
            a.groups.iter().map(|g| &g.key).collect::<Vec<_>>(),
        )
            .cmp(&(
                &b.account,
                &b.resource.key,
                &b.grant.id,
                b.groups.iter().map(|g| &g.key).collect::<Vec<_>>(),
            ))
    });
}
