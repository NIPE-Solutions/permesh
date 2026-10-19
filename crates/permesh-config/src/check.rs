// SPDX-License-Identifier: MIT
//! Optional `check` section: explicit notification destinations (ADR 0032).
//! Experimental format. Destinations are credential references, never inline values.
use crate::{Result, invalid, valid_id, valid_text};
use permesh_secrets::SecretRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_NOTIFICATIONS: usize = 16;
pub const MAX_RECIPIENTS: usize = 32;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    #[serde(default)]
    pub notifications: Vec<Notification>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    NewAdmin,
    RemovedAdmin,
    DepartedWithAccess,
    NoLongerObserved,
    PolicyFinding,
    /// Access no declared profile explains (`profiles` section, ADR 0033).
    UnexplainedAccess,
    /// Declared profile entries not observed for an assigned person.
    MissingAccess,
    Incomplete,
    ProviderFailed,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Channel {
    SlackWebhook {
        url: String,
    },
    TeamsWebhook {
        url: String,
    },
    Webhook {
        url: String,
    },
    Email {
        smtp_host: String,
        smtp_port: u16,
        username: String,
        password: String,
        from: String,
        to: Vec<String>,
    },
}
impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::SlackWebhook { .. } => "SlackWebhook([REDACTED])",
            Self::TeamsWebhook { .. } => "TeamsWebhook([REDACTED])",
            Self::Webhook { .. } => "Webhook([REDACTED])",
            Self::Email { .. } => "Email([REDACTED])",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawNotification", into = "RawNotification")]
pub struct Notification {
    pub id: String,
    pub channel: Channel,
    pub on: Vec<FindingKind>,
    pub detail: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ChannelType {
    SlackWebhook,
    TeamsWebhook,
    Webhook,
    Email,
}

/// Strict wire shape; `flatten` cannot be combined with `deny_unknown_fields`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNotification {
    id: String,
    #[serde(rename = "type")]
    kind: ChannelType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    smtp_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    smtp_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    on: Vec<FindingKind>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    detail: bool,
}

impl TryFrom<RawNotification> for Notification {
    type Error = &'static str;
    fn try_from(raw: RawNotification) -> std::result::Result<Self, Self::Error> {
        let email = raw.smtp_host.is_some()
            || raw.smtp_port.is_some()
            || raw.username.is_some()
            || raw.password.is_some()
            || raw.from.is_some()
            || raw.to.is_some();
        let channel = match raw.kind {
            ChannelType::Email => {
                if raw.url.is_some() {
                    return Err("email notifications do not take a url");
                }
                Channel::Email {
                    smtp_host: raw.smtp_host.ok_or("email requires smtp_host")?,
                    smtp_port: raw.smtp_port.ok_or("email requires smtp_port")?,
                    username: raw.username.ok_or("email requires username")?,
                    password: raw.password.ok_or("email requires password")?,
                    from: raw.from.ok_or("email requires from")?,
                    to: raw.to.ok_or("email requires to")?,
                }
            }
            kind => {
                if email {
                    return Err("webhook notifications take only a url");
                }
                let url = raw.url.ok_or("webhook notifications require a url")?;
                match kind {
                    ChannelType::SlackWebhook => Channel::SlackWebhook { url },
                    ChannelType::TeamsWebhook => Channel::TeamsWebhook { url },
                    _ => Channel::Webhook { url },
                }
            }
        };
        Ok(Self {
            id: raw.id,
            channel,
            on: raw.on,
            detail: raw.detail,
        })
    }
}

impl From<Notification> for RawNotification {
    fn from(n: Notification) -> Self {
        let mut raw = Self {
            id: n.id,
            kind: ChannelType::Webhook,
            url: None,
            smtp_host: None,
            smtp_port: None,
            username: None,
            password: None,
            from: None,
            to: None,
            on: n.on,
            detail: n.detail,
        };
        match n.channel {
            Channel::SlackWebhook { url } => {
                (raw.kind, raw.url) = (ChannelType::SlackWebhook, Some(url))
            }
            Channel::TeamsWebhook { url } => {
                (raw.kind, raw.url) = (ChannelType::TeamsWebhook, Some(url))
            }
            Channel::Webhook { url } => raw.url = Some(url),
            Channel::Email {
                smtp_host,
                smtp_port,
                username,
                password,
                from,
                to,
            } => {
                raw.kind = ChannelType::Email;
                raw.smtp_host = Some(smtp_host);
                raw.smtp_port = Some(smtp_port);
                raw.username = Some(username);
                raw.password = Some(password);
                raw.from = Some(from);
                raw.to = Some(to);
            }
        }
        raw
    }
}

/// Notification credentials resolve without a provider context, so only local
/// environment and keychain references are accepted.
fn local_reference(value: &str) -> bool {
    matches!(
        SecretRef::parse(value),
        Ok(SecretRef::Env(_) | SecretRef::Keychain { .. })
    )
}

pub(crate) fn validate(check: &CheckConfig) -> Result<()> {
    if check.notifications.len() > MAX_NOTIFICATIONS {
        return Err(invalid("check.notifications allows at most 16 entries"));
    }
    let mut ids = BTreeSet::new();
    for n in &check.notifications {
        if !valid_id(&n.id) {
            return Err(invalid(
                "notification id must contain only ASCII letters, digits, hyphen or underscore",
            ));
        }
        if !ids.insert(n.id.as_str()) {
            return Err(invalid("duplicate notification id"));
        }
        let mut kinds = BTreeSet::new();
        if !n.on.iter().all(|k| kinds.insert(*k)) {
            return Err(invalid("duplicate notification finding kind"));
        }
        match &n.channel {
            Channel::SlackWebhook { url }
            | Channel::TeamsWebhook { url }
            | Channel::Webhook { url } => {
                if !local_reference(url) {
                    return Err(invalid(
                        "notification url must be an env:// or keychain:// reference, not an inline value",
                    ));
                }
            }
            Channel::Email {
                smtp_host,
                smtp_port,
                username,
                password,
                from,
                to,
            } => {
                if !local_reference(password) {
                    return Err(invalid(
                        "email password must be an env:// or keychain:// reference, not an inline value",
                    ));
                }
                if *smtp_port == 0
                    || !valid_text(smtp_host)
                    || !valid_text(username)
                    || !valid_text(from)
                    || to.is_empty()
                    || to.len() > MAX_RECIPIENTS
                    || !to.iter().all(|t| valid_text(t))
                {
                    return Err(invalid(
                        "email requires smtp_host, nonzero smtp_port, username, from and 1-32 recipients",
                    ));
                }
            }
        }
    }
    Ok(())
}
