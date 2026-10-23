// SPDX-License-Identifier: MIT
//! The closed set of MCP tools. Each maps to exactly one existing command.
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

pub(crate) fn definitions() -> Vec<Definition> {
    vec![query(
        "doctor",
        "Check the workspace configuration, credentials and provider connectivity.",
        json!({"type": "object", "properties": {"details": {"type": "boolean", "description": "Include curated stage codes and remediation."}}, "additionalProperties": false}),
    )]
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
                matches!(cli.command, Command::Doctor { .. } | Command::User { .. }),
                "{tool:?}"
            );
        }
    }

    #[test]
    fn hidden_variants_are_not_callable() {
        let mut arguments = Map::new();
        arguments.insert("identity".into(), json!("a"));
        assert!(parse("user_access", arguments).is_err());
    }
}
