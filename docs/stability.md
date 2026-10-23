# Stability

Permesh is moving from alpha to beta. This page says which parts you can build on
now and which may still change. It applies to current source and to releases cut
from it.

## Stable

These change only with a version bump, a migration guide in
[`docs/migrations/`](migrations/) and at least one release that still reads the
previous version.

| Surface | Defined in | Pinned by |
| --- | --- | --- |
| Workspace configuration `permesh.yaml`, schema `version: 1` | [Configuration](CONFIGURATION.md) | `permesh-config` boundary and parsing tests |
| Access JSON, schema 2 (`user`, `admins`, `orphaned`, `resource`, snapshots) | [Output schema](output-schema.md) | Golden fixtures in `crates/permesh-cli/tests/fixtures/schema2/` for `user`, `admins`, `orphaned` and snapshots; `resource` by integration tests |
| Control JSON, schema 1 (`init`, `doctor`, `provider`, `auth`, …) | [Output schema](output-schema.md) | Golden fixtures in `crates/permesh-cli/tests/fixtures/control/` for `doctor`, `provider status`, `provider capabilities` and `provider external` commands; others by integration tests |
| Exit codes 0–6 and 130 | [Output schema](output-schema.md#error-envelope), [policies](resource-policies.md) | CLI integration tests |
| Snapshot file format v1 | [Snapshots](snapshots.md) | `crates/permesh-cli/tests/fixtures/snapshot-v1.json` |
| Policy file `version: 1` | [Policies](resource-policies.md) | `tests/resource_policy.rs` and policy unit tests |
| Offboarding plan v1 | [Offboarding](offboarding.md) | `tests/offboarding.rs` |
| Command names: `init`, `doctor`, `provider`, `auth`, `user`, `admins`, `orphaned`, `identity`, `resource`, `policy`, `snapshot`, `diff`, `offboard` | `permesh --help` | CLI integration tests |
| `permesh.yaml` `check` section (notifications) and `profiles` section | [Notifications](notifications.md), [access profiles](profiles.md) | `fixtures/stable/config-check-profiles.*` |
| `profiles/*.yaml` and `people/*.yaml` format 1, including `status: active \| left` and the optional `version: 1` | [Access profiles](profiles.md) | `permesh-config` profile tests |
| Policy file `version: 2` (`unexplained_access` exceptions) | [Policies](resource-policies.md) | `fixtures/stable/policy-v2.json` |
| `check` report version 1 (`check.json` and the `check` result) | [Scheduled checks](check.md) | `fixtures/stable/check-report*.json` |
| `profile_check` and `profile_suggest` results, version 1 | [Output schema](output-schema.md#other-results) | `fixtures/stable/profile-*.json` |
| Access plan file `permesh_access_plan` version 1, including execution plans | [Access plans](access-plans.md) | `fixtures/stable/access-plan-*.json`, `access-execution-plan.json` |
| `access_plan`, `access_verify` and `access_review` results, and `index.json` version 1 | [Output schema](output-schema.md#access-plans) | `fixtures/stable/access-*.json` |
| Command names `check`, `profile`, `access` and `init --starter` | `permesh --help` | CLI integration tests |

Fixtures under `crates/permesh-cli/tests/fixtures/stable/` are checked by
`tests/stable_formats.rs`; see the README there. The second declaration
([ADR 0036](adr/0036-second-stability-declaration.md)) added the rows from
`permesh.yaml` `check` onward.

Additive changes are allowed on stable surfaces: new optional configuration keys,
new JSON fields, new commands and flags. Scripts should ignore JSON fields they
don't recognize.

## Experimental

These may change between releases without a migration guide.

- Provider protocol v1 and the provider SDK (still draft; see
  [provider protocol](provider-protocol.md)).
- `provider dev` commands.
- [AI assistants](mcp.md): `permesh mcp`, its tool names, arguments and result
  shape, and the supported MCP protocol version.
- Starter workflow templates written by `init --starter` (`.github/workflows/`,
  `.gitlab-ci.yml`, `ci/`). They are copied once and then belong to you; new
  releases may write different templates into new workspaces.

## Human-readable output

Terminal text, `summary.md`, HTML views, notification message text and the
`profile suggest` draft comments are for people and may change in any release.
Two promises hold: summaries and notifications carry counts only unless you ask
for `--detail` or `detail: true`, and nothing is reported as clean or removed when
a provider could not be fully read. Automate against the JSON instead.

## Provider maturity

Stability of the CLI does not mean every provider is tested against real tenants.
`permesh provider list` and the [providers table](providers.md) label each provider;
evidence is recorded in [live tenant acceptance](live-acceptance.md).

## Known gaps

Some stable surfaces are covered by behavior tests rather than byte-exact golden
files. A change there is still a breaking change, but CI may not catch it
automatically:

- Policy file v1 and offboarding plan v1 have no golden fixture.
- People and profile files are pinned by parsing tests rather than a golden file.
- Access JSON for `resource`, and control JSON for `init`, `auth`, `provider list`,
  `provider add` and `version`, have no golden fixture.
- Command flags are not checked against a recorded `--help` listing.
- `permesh.yaml` has parsing tests but no golden example covering every key.
