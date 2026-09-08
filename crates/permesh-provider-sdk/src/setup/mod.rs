// SPDX-License-Identifier: MIT
//! Bounded declarative provider setup. Evaluation performs no I/O or secret resolution.
mod evaluate;
mod validate;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MAX_SETUP_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupSpec {
    pub schema_version: u32,
    pub title: String,
    pub description: String,
    pub steps: Vec<SetupStep>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupStep {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub fields: Vec<SetupField>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupField {
    pub key: String,
    pub label: String,
    pub help: String,
    pub required: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_default"
    )]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub input: Input,
}
// Preserve an explicitly supplied null so validation can reject it as a default.
fn present_default<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub field: String,
    pub equals: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    Text {
        min_length: usize,
        max_length: usize,
    },
    Integer {
        minimum: i64,
        maximum: i64,
    },
    Boolean,
    Choice {
        options: Vec<Choice>,
    },
    StringList {
        min_items: usize,
        max_items: usize,
    },
    Json,
    Credential,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Copy, Debug)]
pub struct Question<'a> {
    pub step: &'a SetupStep,
    pub field: &'a SetupField,
}
#[derive(Default)]
pub struct ResolvedSetup {
    pub configuration: BTreeMap<String, Value>,
    pub credentials: BTreeMap<String, String>,
}
impl std::fmt::Debug for ResolvedSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResolvedSetup([REDACTED])")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SetupError {
    #[error("provider setup schema version is unsupported")]
    Version,
    #[error("provider setup specification is invalid")]
    Schema,
    #[error("provider setup exceeds its size or complexity limit")]
    Limit,
    #[error("provider setup condition must reference a valid earlier scalar field")]
    Condition,
    #[error("provider setup answer is invalid")]
    Answer,
    #[error("provider setup answers contain an unknown field")]
    UnknownAnswer,
    #[error("provider setup answers contain an inactive field")]
    InactiveAnswer,
    #[error("provider setup requires an answer to an active field")]
    Required,
}
