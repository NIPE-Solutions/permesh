// SPDX-License-Identifier: MIT
//! The closed set of MCP tools. Each maps to exactly one existing command.
use super::paths;
use crate::{error::AppError, report::Outcome};
use rmcp::model::{CallToolResult, ContentBlock, Tool as Definition, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    sync::Arc,
};

pub(crate) const NOTE: &str =
    "Some providers could not be fully read. Missing access here is not proof of absence.";

#[derive(Debug, Deserialize)]
#[serde(
    tag = "name",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Tool {
    Doctor(DoctorArgs),
    #[serde(rename = "user_access")]
    User(UserArgs),
    #[serde(rename = "resource_access")]
    Resource(ResourceArgs),
    Admins(NoArgs),
    Orphaned(NoArgs),
    ProfileCheck(ProfileCheckArgs),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DoctorArgs {
    #[serde(default)]
    details: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserArgs {
    identity: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceArgs {
    query: Option<String>,
    instance: Option<String>,
    id: Option<String>,
    snapshot: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoArgs {}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfileCheckArgs {
    snapshot: Option<String>,
    policy: Option<String>,
}

fn schema(value: Value) -> Arc<Map<String, Value>> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(Map::new()),
    }
}
fn query(name: &'static str, description: &'static str, input: Value) -> Definition {
    Definition::new(name, description, schema(input))
        .annotate(ToolAnnotations::new().read_only(true).open_world(true))
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}
const SNAPSHOT: &str =
    "Saved snapshot to read offline instead of fresh discovery; a path relative to the workspace.";

pub(crate) fn definitions() -> Vec<Definition> {
    vec![
        query(
            "doctor",
            "Check the workspace configuration, credentials and provider connectivity.",
            object(
                json!({"details": {"type": "boolean", "description": "Include curated stage codes and remediation."}}),
                &[],
            ),
        ),
        query(
            "user_access",
            "Show a person's observed access across providers and the paths (groups, teams, roles) that explain it, each with its certainty.",
            object(
                json!({"identity": {"type": "string", "description": "Identity such as an email address, or a provider account."}}),
                &["identity"],
            ),
        ),
        query(
            "resource_access",
            "Show who has observed access to one resource and through which paths. Give query (the exact display label), or instance and id.",
            object(
                json!({
                    "query": {"type": "string", "description": "Exact resource display label; ambiguity returns candidates."},
                    "instance": {"type": "string", "description": "Provider instance ID, with id."},
                    "id": {"type": "string", "description": "Stable resource ID in that instance."},
                    "snapshot": {"type": "string", "description": SNAPSHOT}
                }),
                &[],
            ),
        ),
        query(
            "admins",
            "Show privileged access, uncertain roles and unresolved group grants.",
            object(json!({}), &[]),
        ),
        query(
            "orphaned",
            "Review accounts not confidently tied to an active identity in the authoritative directory, including inactive and departed people.",
            object(json!({}), &[]),
        ),
        query(
            "profile_check",
            "Compare people's declared access profiles with observed and derived access: missing access, and unexplained access nobody's profile justifies.",
            object(
                json!({
                    "snapshot": {"type": "string", "description": SNAPSHOT},
                    "policy": {"type": "string", "description": "Policy file whose unexplained_access exceptions apply; a path relative to the workspace."}
                }),
                &[],
            ),
        ),
    ]
}

/// Only listed tools are callable, even when a variant exists for a later tool.
pub(crate) fn parse(name: &str, arguments: Map<String, Value>) -> Result<Tool, String> {
    if !definitions().iter().any(|tool| tool.name == name) {
        return Err("Unknown tool; call tools/list for the available tools".into());
    }
    serde_json::from_value(json!({"name": name, "arguments": arguments}))
        .map_err(|_| "Invalid tool arguments; check the tool's input schema".into())
}

fn option(name: &str, value: impl AsRef<OsStr>) -> OsString {
    let mut flag = OsString::from(format!("--{name}="));
    flag.push(value);
    flag
}

impl Tool {
    /// Exact argv: values are `--name=value` and positionals follow `--`, so data never becomes a flag.
    pub(crate) fn argv(&self, workspace: &Path) -> Result<Vec<OsString>, AppError> {
        let root = workspace.parent().unwrap_or(Path::new("."));
        let file = |name: &str, value: &Option<String>| -> Result<Option<OsString>, AppError> {
            value
                .as_deref()
                .map(|value| Ok(option(name, paths::existing(root, value)?)))
                .transpose()
        };
        let mut argv: Vec<OsString> = vec!["permesh".into(), "--json".into()];
        argv.push(option("config", workspace));
        match self {
            Tool::Doctor(args) => {
                argv.push("doctor".into());
                if args.details {
                    argv.push("--details".into());
                }
            }
            Tool::User(args) => {
                argv.extend(["user".into(), "--".into(), (&args.identity).into()]);
            }
            Tool::Resource(args) => {
                argv.push("resource".into());
                argv.extend(file("snapshot", &args.snapshot)?);
                match (&args.query, &args.instance, &args.id) {
                    (Some(query), None, None) => argv.extend(["--".into(), query.into()]),
                    (None, Some(instance), Some(id)) => {
                        argv.extend([option("instance", instance), option("id", id)]);
                    }
                    _ => return Err(AppError::input("Give query, or instance and id")),
                }
            }
            Tool::Admins(_) => argv.push("admins".into()),
            Tool::Orphaned(_) => argv.push("orphaned".into()),
            Tool::ProfileCheck(args) => {
                argv.extend(["profile".into(), "check".into()]);
                argv.extend(file("snapshot", &args.snapshot)?);
                argv.extend(file("policy", &args.policy)?);
            }
        }
        Ok(argv)
    }
}

pub(crate) fn result(outcome: Result<Outcome, AppError>) -> CallToolResult {
    let (value, failed) = match outcome {
        Ok(outcome) => {
            let complete = outcome.report.complete;
            let mut value = json!({"exit_code": outcome.code, "report": outcome.report});
            if !complete {
                value["note"] = json!(NOTE);
            }
            (value, false)
        }
        Err(error) => (
            json!({"exit_code": error.code, "error": {"message": error.message}}),
            true,
        ),
    };
    let content = vec![ContentBlock::text(value.to_string())];
    let mut result = if failed {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    };
    result.structured_content = Some(value);
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::args::{Cli, Command};
    use clap::Parser;

    fn parsed(tool: &Tool) -> Cli {
        Cli::try_parse_from(tool.argv(Path::new("/w/permesh.yaml")).unwrap()).unwrap()
    }
    fn samples() -> Vec<Tool> {
        vec![
            Tool::Doctor(DoctorArgs { details: true }),
            Tool::User(UserArgs {
                identity: "a@example.com".into(),
            }),
            Tool::Resource(ResourceArgs {
                query: Some("-x".into()),
                instance: None,
                id: None,
                snapshot: None,
            }),
            Tool::Admins(NoArgs {}),
            Tool::Orphaned(NoArgs {}),
            Tool::ProfileCheck(ProfileCheckArgs {
                snapshot: None,
                policy: None,
            }),
        ]
    }

    #[test]
    fn flag_like_values_stay_positional() {
        let tool = Tool::User(UserArgs {
            identity: "--config=/x".into(),
        });
        let argv = tool.argv(Path::new("/w/permesh.yaml")).unwrap();
        assert_eq!(argv[argv.len() - 3..], ["user", "--", "--config=/x"]);
        let cli = parsed(&tool);
        assert!(matches!(&cli.command, Command::User { identity } if identity == "--config=/x"));
        assert_eq!(cli.config.as_deref(), Some(Path::new("/w/permesh.yaml")));
        assert!(cli.json);
    }

    #[test]
    fn argv_only_reaches_allowed_commands() {
        for tool in samples() {
            let cli = parsed(&tool);
            assert!(
                matches!(
                    cli.command,
                    Command::Doctor { .. }
                        | Command::User { .. }
                        | Command::Resource(_)
                        | Command::Admins
                        | Command::Orphaned
                        | Command::Profile {
                            command: crate::profile_command::ProfileCommand::Check { .. }
                        }
                ),
                "{tool:?}"
            );
        }
    }
}
