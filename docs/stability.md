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

Additive changes are allowed on stable surfaces: new optional configuration keys,
new JSON fields, new commands and flags. Scripts should ignore JSON fields they
don't recognize.

## Experimental

These may change between releases without a migration guide.

- Provider protocol v1 and the provider SDK (still draft; see
  [provider protocol](provider-protocol.md)).
- `provider dev` commands.
- `permesh check`, its `check.json` report and `summary.md`, the `check` section of
  `permesh.yaml` (notifications) and `init --starter`.
- Everything still to come for access profiles, people files and joiner/mover/leaver
  plans, until a second stability declaration after that work lands.

## Provider maturity

Stability of the CLI does not mean every provider is tested against real tenants.
`permesh provider list` and the [providers table](providers.md) label each provider;
evidence is recorded in [live tenant acceptance](live-acceptance.md).

## Known gaps

Some stable surfaces are covered by behavior tests rather than byte-exact golden
files. A change there is still a breaking change, but CI may not catch it
automatically:

- Policy file v1 and offboarding plan v1 have no golden fixture.
- Access JSON for `resource`, and control JSON for `init`, `auth`, `provider list`,
  `provider add` and `version`, have no golden fixture.
- Command flags are not checked against a recorded `--help` listing.
- `permesh.yaml` has parsing tests but no golden example covering every key.
