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

Common `command` values include `init`, `provider_add`, `provider_migrate`, `provider_setup`, `provider_setup_describe`, `provider_list`, `provider_capabilities`, `provider_status`, `auth_login`, `auth_logout`, `auth_status`, `doctor`, `user`, `admins`, `orphaned`, `version`. Times use UTC RFC3339. They delimit the command collection window, not a cross-provider transaction.

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

## Orphaned result

`orphaned` uses the same envelope. `result` contains:

- `authorities`: sorted, deduplicated configured authoritative provider IDs.
- `authority_complete`: true only when every named authority has a complete snapshot. This does not imply unrestricted directory visibility.
- `accounts`: sorted `{account, identity, reason}` records. Account and identity resolution use the existing user/admin shapes. `reason` is one of `inactive_identity`, `unknown_identity`, `unknown_status`, `ambiguous_identity`, `external_identity`, `service_account`, `bot`, or `unassessed`.
- `access`: existing AccessPath records for returned accounts, preserving membership and grant provenance. Accounts without observed paths remain present.

When any authority is absent or partial, every observed account has reason `unassessed`; identity evidence remains available but no orphan classification is made. With complete authorities, ordinary active identities are omitted and service, bot and external identities are listed separately. Ambiguity and resolved inactivity take precedence over those separate categories. All-provider failure leaves `result: {}` and `complete: false`. No authoritative source is a configuration error (exit 2). Completed inspection returns 0 regardless of findings; provider failure codes remain 3/4. See [classification details](orphaned.md).

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

`completion <shell>` emits shell source rather than an access report. It rejects
`--json` with an input error using the existing error envelope. See
[shell completion](completion.md).

## Explicit external commands

Standalone external commands (`inspect`, `trust`, `list`, `remove`, `discover`)
bypass workspace loading and retain the schema-1 report envelope. Workspace
`review` and `approve` load the selected configuration; `revoke` uses its canonical
path. These workspace commands manage local execution permission without discovery
or credential resolution.

- `external_inspect`: `{inspection: {sha256,size}, message}`; hashes without executing.
- `external_trust`: `{registration: {schema,id,sha256,capabilities}, storage, message}`.
- `external_list`: `{registrations: [...], storage}` sorted by registered ID.
- `external_remove`: `{id, storage, message}`.
- `external_discover`: `{snapshot, storage}` and one provider-status entry. Snapshot
  fields are `provider`, `identities`, `accounts`, `resources`, `groups`,
  `memberships`, `grants`, `complete` and `limitations`, using the domain schema.

The registration schema is independent of the output and wire versions. Digests
are lowercase SHA-256 hex; size is bytes. Capabilities describe normalized record
classes. Discovery exits 0 for a validated complete snapshot, 4 for a validated
partial snapshot, 3 for host/protocol/provider failure and 130 on cancellation.
Invalid input, missing registration or changed trust state exits 2. Raw child
stderr and operating-system diagnostics are not included in errors. The explicit
storage path is local configuration metadata; protect reports appropriately.

Workspace approval commands also use the schema-1 report envelope:

- `external_review`: `{workspace, instance, registration, fingerprint, approved,
  configuration, credential_references, identity, message}`. `workspace` is the
  canonical configuration-file path; `identity` contains configured sources and
  aliases. The fingerprint binds the full normalized configuration, including
  providers not separately displayed here. Settings and credential locators are
  visible; resolved credential values are absent.
- `external_approve`: `{approval: {schema, workspace, instance, fingerprint},
  message}` with approval schema 1.
- `external_revoke`: `{workspace, instance, message}`.

Approved workspace external providers feed the existing user/admins/orphaned
shapes. Health contributes curated status and limitation text to doctor/status;
it does not return a discovery snapshot. Missing/stale approval is a provider
failure during collection, preserving the existing all-failed/partial exit rules.

## Provider setup

`provider_setup_describe` returns `{provider, id, sha256, spec, message}` under
`result`. It executes a verified registered provider to obtain validated setup
schema 1, without loading or writing a workspace. `spec.schema_version` is
independent of report schema 1 and wire draft 3.

`provider_setup` returns `{provider, id, sha256, file, next, message}` after adding
an instance. `provider` is the registered ID, `id` is the workspace instance, and
`sha256` is the registered digest pinned in configuration. This result does not
indicate workspace approval or successful remote authentication. Noninteractive
and JSON mutation require `--answers FILE`; prompts never appear in JSON output.
See [setup behavior and answer format](provider-setup.md).

## Provider packages

`provider_install` and `provider_update` use the normal schema-1 envelope. Results
include `provider`, `target`, sorted `installed_versions` after the operation,
`available_version`, `selected` (the full release record), `update_available`,
`check_only`, `changed`, `package`, `trust_changed: false`,
`workspace_changed: false`, and a curated `message`. `package` is null for checks;
otherwise it contains `release` and the absolute local `executable` path.
`changed` describes installation, never workspace adoption. No compatible release
produces the ordinary versioned error envelope. Check-only update availability is
informational and does not return audit-findings exit 1.

## Provider migration

`provider_migrate` returns `{id, provider, sha256, file, changed, trust_changed,
approval_records_changed, execution_approvals_require_review, credentials_resolved,
message, next}`. `id` is the preserved workspace instance, `provider` is `github`,
`sha256` is the reviewed trusted digest, and `file` is the canonical workspace
path. Successful conversion sets `changed: true`, `trust_changed: false`,
`approval_records_changed: false`, `execution_approvals_require_review: true` and
`credentials_resolved: false`. No credential references or values are included.

This describes an explicit configuration conversion, not authentication or access
discovery. Approval records remain stored, but their old whole-workspace
fingerprints no longer approve the changed configuration. Repeated conversion of
an already external instance returns input error 2 without rewriting it. See
[migration behavior and exit codes](github-migration.md).
