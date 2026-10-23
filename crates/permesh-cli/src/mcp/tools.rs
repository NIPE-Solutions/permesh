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
    PlanJoin(JoinArgs),
    PlanMove(MoveArgs),
    PlanLeave(LeaveArgs),
    VerifyPlan(VerifyArgs),
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JoinArgs {
    person: String,
    profiles: Vec<String>,
    output: String,
    snapshot: Option<String>,
    expires_in_hours: Option<u32>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoveArgs {
    person: String,
    remove: Vec<String>,
    #[serde(default)]
    add: Vec<String>,
    output: String,
    snapshot: Option<String>,
    expires_in_hours: Option<u32>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LeaveArgs {
    person: String,
    annotations: Option<String>,
    output: String,
    snapshot: Option<String>,
    expires_in_hours: Option<u32>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifyArgs {
    plan: String,
    snapshot: Option<String>,
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
/// Writes one new local file and never overwrites; no provider is changed.
fn plan(name: &'static str, description: &'static str, input: Value) -> Definition {
    Definition::new(name, description, schema(input)).annotate(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(false)
            .open_world(true),
    )
}
fn plan_properties(mut properties: Value) -> Value {
    properties["person"] =
        json!({"type": "string", "description": "The person's identity, as in their people file."});
    properties["output"] = json!({"type": "string", "description": "New plan file to write, relative to the workspace; its folder must exist and the file must not."});
    properties["snapshot"] = json!({"type": "string", "description": SNAPSHOT});
    properties["expires_in_hours"] = json!({"type": "integer", "minimum": 1, "maximum": 168, "default": 24, "description": "Hours the plan stays fresh, from the evidence start."});
    properties
}
fn names(description: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": description})
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
        plan(
            "plan_join",
            "Prepare a reviewable plan file of the access a person needs for added profiles, including accounts still to create. Permesh never applies it.",
            object(
                plan_properties(json!({"profiles": names("Profiles to add.")})),
                &["person", "profiles", "output"],
            ),
        ),
        plan(
            "plan_move",
            "Prepare a reviewable plan file for a profile change: removals only for access no remaining profile justifies, and shared access listed as kept. Permesh never applies it.",
            object(
                plan_properties(
                    json!({"remove": names("Profiles to take away."), "add": names("Profiles to add.")}),
                ),
                &["person", "remove", "output"],
            ),
        ),
        plan(
            "plan_leave",
            "Prepare a reviewable plan file removing all of a departing person's access, separating expected from unexplained access, with account, ownership and machine-account review steps. Permesh never applies it.",
            object(
                plan_properties(
                    json!({"annotations": {"type": "string", "description": "Owner annotations for machine accounts, as in offboard; a path relative to the workspace."}}),
                ),
                &["person", "output"],
            ),
        ),
        query(
            "verify_plan",
            "Check each step of a plan file against fresh discovery: done, not done or cannot verify, plus any extra access.",
            object(
                json!({
                    "plan": {"type": "string", "description": "Plan file, relative to the workspace."},
                    "snapshot": {"type": "string", "description": SNAPSHOT}
                }),
                &["plan"],
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
        // Flags every plan tool shares; `output` is a new file inside the workspace.
        let common = |output: &str,
                      snapshot: &Option<String>,
                      hours: Option<u32>|
         -> Result<Vec<OsString>, AppError> {
            let mut flags = vec![option("output", paths::new_file(root, output)?)];
            flags.extend(file("snapshot", snapshot)?);
            flags.extend(hours.map(|hours| option("expires-in-hours", hours.to_string())));
            Ok(flags)
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
            Tool::PlanJoin(args) => {
                argv.extend(["access".into(), "join".into()]);
                argv.extend(args.profiles.iter().map(|p| option("profile", p)));
                argv.extend(common(&args.output, &args.snapshot, args.expires_in_hours)?);
                argv.extend(["--".into(), (&args.person).into()]);
            }
            Tool::PlanMove(args) => {
                argv.extend(["access".into(), "move".into()]);
                argv.extend(args.remove.iter().map(|p| option("remove", p)));
                argv.extend(args.add.iter().map(|p| option("add", p)));
                argv.extend(common(&args.output, &args.snapshot, args.expires_in_hours)?);
                argv.extend(["--".into(), (&args.person).into()]);
            }
            Tool::PlanLeave(args) => {
                argv.extend(["access".into(), "leave".into()]);
                argv.extend(file("annotations", &args.annotations)?);
                argv.extend(common(&args.output, &args.snapshot, args.expires_in_hours)?);
                argv.extend(["--".into(), (&args.person).into()]);
            }
            Tool::VerifyPlan(args) => {
                argv.extend(["access".into(), "verify".into()]);
                argv.extend(file("snapshot", &args.snapshot)?);
                let plan = paths::existing(root, &args.plan)?;
                argv.extend(["--".into(), plan.into()]);
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
    use crate::{
        access::AccessCommand,
        args::{Cli, Command},
    };
    use clap::Parser;

    fn parsed(root: &Path, tool: &Tool) -> Cli {
        Cli::try_parse_from(tool.argv(&root.join("permesh.yaml")).unwrap()).unwrap()
    }
    /// One sample per variant; file arguments exist inside `root`.
    fn samples(root: &Path) -> Vec<Tool> {
        std::fs::write(root.join("p.json"), "{}").unwrap();
        let snapshot = || Some("p.json".to_string());
        vec![
            Tool::PlanJoin(JoinArgs {
                person: "-p".into(),
                profiles: vec!["--x".into()],
                output: "new.json".into(),
                snapshot: snapshot(),
                expires_in_hours: Some(48),
            }),
            Tool::PlanMove(MoveArgs {
                person: "-p".into(),
                remove: vec!["a".into()],
                add: vec!["b".into()],
                output: "new.json".into(),
                snapshot: None,
                expires_in_hours: None,
            }),
            Tool::PlanLeave(LeaveArgs {
                person: "-p".into(),
                annotations: snapshot(),
                output: "new.json".into(),
                snapshot: None,
                expires_in_hours: None,
            }),
            Tool::VerifyPlan(VerifyArgs {
                plan: "p.json".into(),
                snapshot: snapshot(),
            }),
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
        let dir = tempfile::tempdir().unwrap();
        let cli = parsed(dir.path(), &tool);
        assert!(matches!(&cli.command, Command::User { identity } if identity == "--config=/x"));
        assert_eq!(cli.config, Some(dir.path().join("permesh.yaml")));
        assert!(cli.json);
    }

    #[test]
    fn argv_only_reaches_allowed_commands() {
        let dir = tempfile::tempdir().unwrap();
        for tool in samples(dir.path()) {
            let cli = parsed(dir.path(), &tool);
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
                        | Command::Access {
                            command: AccessCommand::Join { .. }
                                | AccessCommand::Move { .. }
                                | AccessCommand::Leave { .. }
                                | AccessCommand::Verify { .. }
                        }
                ),
                "{tool:?}"
            );
        }
    }
}
