# Output schema 1 and exit codes

`--json` is global and can appear before or after a subcommand. Normal successful command output goes to stdout. Human errors go to stderr; JSON errors go to stdout as one object. JSON contains no ANSI, progress lines or logs. Help/version flags are Clap's text presentation; use `permesh version --json` for structured version output.

## Successful command envelope

```json
{
  "schema_version": 1,
  "command": "user",
  "complete": true,
  "started_at": "2026-09-08T09:00:00Z",
  "completed_at": "2026-09-08T09:00:01Z",
  "providers": [],
  "result": { "identity": null, "accounts": [], "access": [] }
}
```

`command` is one of `init`, `provider_add`, `provider_list`, `provider_capabilities`, `provider_status`, `auth_login`, `auth_logout`, `auth_status`, `doctor`, `user`, `admins`, `version`. Times use UTC RFC3339. They delimit the command collection window, not a cross-provider transaction.

`providers` is sorted by instance ID. Each entry has `id`, `kind`, `state` (`connected`, `partial`, `failed`), curated `message`, and `limitations` array. `complete` describes completion within supported adapter scope. Even true does not certify exhaustive effective authorization. Failures never appear as empty successful provider snapshots.

## User result

- `identity`: null for an uncorrelated account lookup, otherwise `{id, kind, status, verified_emails}`.
- `accounts`: sorted records `{key: {provider,id}, login, kind, verified_emails}`.
- `access`: sorted paths `{account, groups, memberships, resource, grant}`. `account` is a scoped key. A group/resource is `{key,name}`. Membership is `{member,group,provenance}`.
- A subject is `{kind: "account" | "group", key: {provider,id}}`.
- A grant is `{id,subject,resource,role,privilege,certainty,provenance}`. Native role text is preserved. Privilege is `standard`, `elevated`, `admin`, `owner`, or `unknown`. Certainty is `observed`, `inferred`, or `unknown`.
- Provenance is `{method,observed_at}` with UTC RFC3339 time. Membership provenance is retained at every hop.
- Identity kind is `human`, `external`, `service`, `bot`, `unknown`; status is `active`, `inactive`, `external`, `service`, `unknown`.
- Additional curated `message`/`warnings` can explain missing evidence. Consumers must tolerate additive fields.

With all providers failed, `result` is `{}` and provider failures explain the absence of a query result. With partial providers and no match, a user result contains empty arrays plus a message; this is not a definitive not-found result.

## Admins result

`admins` uses the same envelope with `command: "admins"` and these result arrays:

- `accounts`: `{account, identity}` records. `account` has the same shape as a user-result account. `identity` is a tagged resolution: `{"state":"resolved","identity":{...}}`, `{"state":"unmapped"}`, or `{"state":"ambiguous","candidates":["canonical-id",...]}`. A conflicting authority can produce ambiguity even with one candidate ID. Accounts occur once, sorted by scoped key, and include those having known or unknown privilege paths.
- `access`: existing AccessPath records whose grants have `elevated`, `admin`, or `owner` privilege.
- `unknown_access`: AccessPath records whose grants have `unknown` privilege. They are not counted as confirmed administrators.
- `unresolved_grants`: `{grant,resource,group}` records for nonstandard grants with no observed account path. The group is the subject group record, or null if unavailable. These records retain privilege, certainty, role, and provenance. They do not certify an empty group or unused grant.

Standard grants are excluded. Arrays are deterministic, scoped by provider and native IDs. Multiple paths to the same grant are retained. All-provider failure leaves `result: {}` with `complete: false`, as for user queries. There is no policy finding exit code: completed inspection returns 0 even if administrators are present. Source ambiguity is data here; it remains an error for a user lookup that cannot resolve uniquely. Provider failures still return 3/4.

## Other results

`provider_list`: `{providers: [{id,type}]}`. `provider_capabilities`: `{id, metadata: {kind,capabilities}}`; capability strings enumerate supported read operations. `doctor`/`provider_status`: `{workspace_schema, organization, identity_sources, message, external_providers, local_overrides}`. `auth_status`: `{message}` plus provider status entries; availability does not mean remote authentication succeeded. `init`: `{message,file,next}`; `provider_add`: `{message,next}`; login/logout: `{message}`. `version`: `{version,telemetry:false,backend:false}`.

## Error envelope

```json
{"schema_version":1,"error":{"code":2,"message":"Invalid command arguments. Run permesh --help or permesh <command> --help for usage."}}
```

Errors deliberately omit raw arguments, YAML snippets, tokens, provider response bodies and low-level credential-store errors. Configuration syntax errors expose numeric line/column when the parser supplies it. Normal human output escapes provider-controlled terminal controls and bidi overrides; JSON strings preserve data using JSON escaping.

| Exit | Meaning |
| --- | --- |
| 0 | Success within documented scope; a known identity may have zero observed grants |
| 1 | No matching identity/account in completed collection |
| 2 | Invalid arguments/configuration or ambiguous lookup |
| 3 | All requested providers failed, or credential operation failed |
| 4 | Some results available, but collection/credential checks incomplete |
| 5 | Internal or output error |
| 130 | Cancelled by Ctrl+C |

A broken stdout pipe exits cleanly with 0, following Unix pipeline conventions. A partial provider takes precedence over not-found, since missing data cannot establish absence. Schema changes are intentional and recorded in CHANGELOG; output schema, workspace schema and plugin protocol versions are independent.
