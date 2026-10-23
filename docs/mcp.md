# AI assistants (MCP)

`permesh mcp` lets an AI assistant on your machine ask Permesh the questions you
would ask the CLI: who has access to a repository, why someone still has it, what a
move or departure should remove. The assistant gets the same answers as the CLI,
including whether they are complete. It cannot change access.

The server and its tools are experimental ([stability](stability.md)).

## Set it up

The assistant starts `permesh mcp` itself and talks to it over stdin and stdout.
Point it at your workspace file with an absolute path.

Claude Code:

```bash
claude mcp add permesh -- permesh mcp --config /path/to/company-access/permesh.yaml
```

Other clients take an entry like this in their MCP server configuration:

```json
{
  "mcpServers": {
    "permesh": {
      "command": "permesh",
      "args": ["mcp", "--config", "/path/to/company-access/permesh.yaml"]
    }
  }
}
```

Set up providers, credentials and external-provider approvals with the CLI first.
The server uses them as they are; it never approves, stores credentials or prompts.

**Client requirement.** The server speaks MCP protocol version 2026-07-28 only.
A client that starts with the older `initialize` handshake gets error `-32022`
listing `2026-07-28`, and `permesh mcp` exits with code 2. Update the client if
you see that error. If Permesh cannot write to the client at all, it exits with
code 5 instead.

## Tools

| Tool | Answers | Runs |
| --- | --- | --- |
| `doctor` | Is the configuration valid and can each provider be reached? | `permesh doctor` |
| `user_access` | What can this person reach, and through which groups and roles? | `permesh user` |
| `resource_access` | Who can reach this resource? Give `query`, or `instance` and `id`. | `permesh resource` |
| `admins` | Who has privileged access, and where is privilege uncertain? | `permesh admins` |
| `orphaned` | Which accounts don't belong to an active person? | `permesh orphaned` |
| `profile_check` | What access is missing, and what does no profile explain? | `permesh profile check` |
| `plan_join` | Write a plan for the access a new profile needs. | `permesh access join` |
| `plan_move` | Write a plan for a profile change. | `permesh access move` |
| `plan_leave` | Write a plan to remove a departing person's access. | `permesh access leave` |
| `verify_plan` | Which plan steps are done, not done, or can't be checked? | `permesh access verify` |

`resource_access`, `profile_check`, the plan tools and `verify_plan` accept
`snapshot` to read a saved snapshot instead of running fresh discovery. See
[access plans](access-plans.md) for what plans contain.

Every other command is left out on purpose; use the CLI for it:

- `init`, `provider`, `auth` and `identity`: they set up the workspace, trust,
  credentials and mappings, which stay your decisions.
- `check` (it sends notifications) and `access review` (the pull-request flow).
- `snapshot`, `diff`, `policy`, `offboard` and `profile suggest`.
- `completion`, `version` and `mcp` itself.

## Reading results

Each result's structured content is:

- `exit_code`: the CLI exit code ([exit codes](output-schema.md)). 4 (incomplete)
  and 6 (needs action) are answers, not errors.
- `report`: the same report `--json` prints, with `complete`, provider states and,
  on every access path, its certainty.
- `note`, when `report.complete` is false: "Some providers could not be fully
  read. Missing access here is not proof of absence."

A failure sets `isError` and gives `error.message` instead of `report`. The same
JSON is also in the text content.

Ending the session (closing stdin, Ctrl+C or SIGTERM) during a call stops that
call the same way: provider processes are stopped before `permesh mcp` exits.

When `complete` is false, a good assistant says it can't tell, rather than "no
access". The server's instructions tell it so.

## Files and paths

File arguments (`snapshot`, `policy`, `annotations`, `plan`, `output`) are paths
relative to the folder that holds `permesh.yaml`. Absolute paths and `..` that leaves
the folder are refused. A symlink that points outside it reads as "File not found in
the workspace", so it never tells whether its target exists.

The plan tools are the only tools that write. They create `output` as a new
`.json` file; its folder must already exist (for example `plans/`) and the file
must not. Plans can't be written into `people/`, `profiles/` or hidden folders,
where they would be read as workspace files. Permesh
never overwrites a plan and never carries one out. The checks run just before the
command; they don't protect against another program on your machine swapping in a
symlink in between, which could already write anywhere you can.

## Privacy

The assistant sees what the CLI shows: names, accounts, groups and resources. Use
an assistant and client you trust with that data. Each call may run fresh
discovery against your providers, which takes time and uses their rate limits;
give `snapshot` to work from a saved capture instead.
