// SPDX-License-Identifier: MIT
//! Policy document version 1. Annotations are local organizational assertions.
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Rule {
    InactiveAccess,
    ExternalPrivileged,
    MachineOwner,
    ExpiredException,
    RequiredCoverage,
}
impl Rule {
    pub(super) fn access(self) -> Option<permesh_core::AccessRule> {
        use permesh_core::AccessRule as R;
        match self {
            Self::InactiveAccess => Some(R::InactiveAccess),
            Self::ExternalPrivileged => Some(R::ExternalPrivileged),
            Self::MachineOwner => Some(R::MachineOwner),
            _ => None,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub version: u32,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub required_providers: Vec<RequiredProvider>,
    #[serde(default)]
    pub owners: Vec<Owner>,
    #[serde(default)]
    pub exceptions: Vec<Exception>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequiredProvider {
    pub instance: String,
    pub capabilities: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Owner {
    pub instance: String,
    pub account: String,
    pub owner: String,
    pub reason: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Exception {
    pub id: String,
    pub rule: Rule,
    pub instance: String,
    pub account: String,
    pub resource: String,
    pub grant_id: String,
    pub reason: String,
    pub owner: String,
    pub expires_at: String,
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max
        && !value.chars().any(char::is_control)
}
fn invalid() -> AppError {
    AppError::input(
        "Invalid policy version 1: check supported rules, exact unique scopes, bounded text, owners and UTC expiries",
    )
}
pub(super) fn expiry(value: &str) -> Result<OffsetDateTime, AppError> {
    let value = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| invalid())?;
    if !value.offset().is_utc() {
        return Err(invalid());
    }
    Ok(value)
}
impl Policy {
    pub(super) fn validate(&self) -> Result<(), AppError> {
        if self.version != 1
            || self.rules.is_empty()
            || self.rules.len() > 5
            || self.rules.iter().copied().collect::<BTreeSet<_>>().len() != self.rules.len()
            || self.owners.len() > 1000
            || self.exceptions.len() > 1000
            || self.required_providers.len() > 256
        {
            return Err(invalid());
        }
        let mut required = BTreeSet::new();
        for provider in &self.required_providers {
            if !text(&provider.instance, 256)
                || !required.insert(&provider.instance)
                || provider.capabilities.is_empty()
                || provider.capabilities.len() > 6
                || provider.capabilities.iter().collect::<BTreeSet<_>>().len()
                    != provider.capabilities.len()
                || provider.capabilities.iter().any(|c| {
                    !matches!(
                        c.as_str(),
                        "identities"
                            | "accounts"
                            | "resources"
                            | "groups"
                            | "memberships"
                            | "grants"
                    )
                })
            {
                return Err(invalid());
            }
        }
        if self.rules.contains(&Rule::RequiredCoverage) && self.required_providers.is_empty() {
            return Err(invalid());
        }
        let mut owners = BTreeSet::new();
        for owner in &self.owners {
            if [&owner.instance, &owner.account, &owner.owner]
                .iter()
                .any(|v| !text(v, 256))
                || !text(&owner.reason, 1024)
                || !owners.insert((&owner.instance, &owner.account))
            {
                return Err(invalid());
            }
        }
        let mut ids = BTreeSet::new();
        let mut scopes = BTreeSet::new();
        for exception in &self.exceptions {
            if exception.rule.access().is_none()
                || [
                    &exception.id,
                    &exception.instance,
                    &exception.account,
                    &exception.resource,
                    &exception.grant_id,
                    &exception.owner,
                ]
                .iter()
                .any(|v| !text(v, 256))
                || !text(&exception.reason, 1024)
                || !ids.insert(&exception.id)
                || !scopes.insert((
                    exception.rule,
                    &exception.instance,
                    &exception.account,
                    &exception.resource,
                    &exception.grant_id,
                ))
            {
                return Err(invalid());
            }
            expiry(&exception.expires_at)?;
        }
        Ok(())
    }
}
