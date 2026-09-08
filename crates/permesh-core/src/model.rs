// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityKey {
    pub provider: String,
    pub id: String,
}
impl EntityKey {
    pub fn new(provider: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            id: id.into(),
        }
    }
}
macro_rules! domain_enum {
    ($name:ident { $($variant:ident),+ }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}
domain_enum!(IdentityKind {
    Human,
    External,
    Service,
    Bot,
    Unknown
});
domain_enum!(IdentityStatus {
    Active,
    Inactive,
    External,
    Service,
    Unknown
});
domain_enum!(Privilege {
    Standard,
    Elevated,
    Admin,
    Owner,
    Unknown
});
domain_enum!(Certainty {
    Observed,
    Inferred,
    Unknown
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub kind: IdentityKind,
    pub status: IdentityStatus,
    pub verified_emails: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub key: EntityKey,
    pub login: String,
    pub kind: IdentityKind,
    pub verified_emails: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub key: EntityKey,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub key: EntityKey,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "key", rename_all = "snake_case")]
pub enum Subject {
    Account(EntityKey),
    Group(EntityKey),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub method: String,
    pub observed_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Membership {
    pub member: Subject,
    pub group: EntityKey,
    pub provenance: Provenance,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grant {
    pub id: String,
    pub subject: Subject,
    pub resource: EntityKey,
    pub role: String,
    pub privilege: Privilege,
    pub certainty: Certainty,
    pub provenance: Provenance,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub provider: String,
    pub identities: Vec<Identity>,
    pub accounts: Vec<Account>,
    pub resources: Vec<Resource>,
    pub groups: Vec<Group>,
    pub memberships: Vec<Membership>,
    pub grants: Vec<Grant>,
    pub limitations: Vec<String>,
    pub complete: bool,
}
impl Snapshot {
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            identities: vec![],
            accounts: vec![],
            resources: vec![],
            groups: vec![],
            memberships: vec![],
            grants: vec![],
            limitations: vec![],
            complete: true,
        }
    }
    pub fn sort(&mut self) {
        self.identities.sort_by(|a, b| a.id.cmp(&b.id));
        self.accounts.sort_by(|a, b| a.key.cmp(&b.key));
        self.resources.sort_by(|a, b| a.key.cmp(&b.key));
        self.groups.sort_by(|a, b| a.key.cmp(&b.key));
        self.memberships
            .sort_by(|a, b| (&a.member, &a.group).cmp(&(&b.member, &b.group)));
        self.grants.sort_by(|a, b| a.id.cmp(&b.id));
        self.limitations.sort();
        self.limitations.dedup();
        for account in &mut self.accounts {
            account.verified_emails.sort();
            account.verified_emails.dedup();
        }
        for identity in &mut self.identities {
            identity.verified_emails.sort();
            identity.verified_emails.dedup();
        }
    }
}
