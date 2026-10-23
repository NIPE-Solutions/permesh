# ADR 0035: Read-only MCP server

Status: accepted for the unreleased source candidate (experimental)

## Decision

`permesh mcp` runs a local [Model Context Protocol](https://modelcontextprotocol.io)
server over stdio, so an AI assistant on the administrator's machine can ask the
same questions as the CLI. It never changes access, including after a later apply
stage ([ADR 0028](0028-separate-opt-in-remediation.md)): applying stays a deliberate
CLI action.

- **Same commands, same trust.** Each tool is one variant of a closed list and
  becomes an exact `permesh --json` argument vector, which the normal parser and
  command code run. The server therefore uses the same workspace, approvals and
  credentials as the CLI. There is no generic pass-through, so no tool can reach
  `check` (it sends notifications), `snapshot save`, `identity map`, `auth`,
  `provider`, `init`, `offboard` or `access review`.
- **Tools.** `doctor`, `user_access`, `resource_access`, `admins`, `orphaned`,
  `profile_check`, `plan_join`, `plan_move`, `plan_leave` and `verify_plan`.
- **One write.** The plan tools write a new `.json` plan file, which a person
  reviews and carries out. They never overwrite a file, and never write into
  `people/`, `profiles/` or hidden folders, where plan JSON would be read as
  workspace YAML. HTML views are not offered.
- **Paths stay inside the workspace.** File arguments are relative to the workspace
  folder. Absolute paths, `..` that leaves it and symlinks that point outside are
  input errors, checked before any command runs. Values are passed as `--name=value`
  and positionals follow `--`, so a value never becomes a flag.
- **Results carry completeness.** A result holds the exit code and the same report
  `--json` prints. When the capture is incomplete it adds a note that missing access
  is not proof of absence. Exit codes 4 and 6 are answers, not tool errors.
- **One call at a time.** Calls run in sequence. A cancelled call cancels the
  command and waits until provider processes are stopped, as Ctrl+C does. Ending the
  session (stdin closed, Ctrl+C or SIGTERM) does the same for a running call.
- **Protocol 2026-07-28 only.** The server uses the official Rust SDK (`rmcp`) and
  supports only the stateless MCP revision with `server/discover`. Clients that
  start with the older `initialize` handshake get an error naming that version.

## Why

A single administrator often has no second person to ask. An assistant that can
answer "who can still reach this repository, and through which group?" helps only
if it reports uncertainty honestly and cannot be talked into changing anything.
Running the existing commands, instead of a second implementation, keeps the
answers identical to the CLI's and inherits every safety rule they already follow.
Supporting one protocol revision keeps the surface small while there are no users
to stay compatible with.

## Consequences

The assistant sees the same access data the CLI shows, so it is only as private as
the assistant and its client. Older MCP clients cannot connect until they support
2026-07-28. Each call runs fresh discovery unless a snapshot is given, so answers
can be slow and use provider rate limits. Plan files need an existing folder in the
workspace.
