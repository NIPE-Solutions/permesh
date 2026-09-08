# Domain and access-output schema 2

This is an intentional prerelease Rust API and access JSON change. Workspace
schema 1, secret references, trust registration, approval inputs, exit codes and
legacy NDJSON drafts remain unchanged. Existing releases are not overwritten.

## CLI consumers

`user`, `admins`, `orphaned` and `provider external discover` now return access
reports with `schema_version: 2`. Control commands such as doctor, provider list,
setup, installation and authentication retain schema 1. Version selection follows
the parsed command, including its errors, cancellation and partial results.
Generic argument-parser errors remain schema 1 because a command may not exist.

Consumers must dispatch on both command and schema version. Schema 2 adds:

| Record | Change |
| --- | --- |
| Identity | `affiliation`; kind excludes external; status excludes external/service and includes suspended |
| Account | `affiliation` and its own lifecycle `status`; kind excludes external |
| Resource | nullable namespaced `kind` and nullable scoped `parent` |
| Grant | `evidence_kind`; certainty additionally permits derived |
| Access path | path `certainty`, separate from original grant certainty |
| Orphaned account | `suspended_identity` reason |

For example, an external service principal is represented independently:

```json
{"id":"directory:automation","kind":"service","affiliation":"external","status":"inactive","verified_emails":[]}
```

Automation should check lifecycle before intentionally separate principal
categories. A service account can still have an inactive or suspended authority.
Continue checking completeness and provider limitations before treating absence
as evidence. Existing role strings, scoped IDs, membership hops and timestamps
retain their meanings.

There is no lossy schema-1 export switch. Applications needing the earlier output
must keep using the previously released CLI until their consumer handles schema 2.
Historical schema-1 fixtures remain as a record of that contract; new schema-2
fixtures cover the migrated access reports. JSON is still emitted only to stdout,
with no prompts or decoration. No new export or snapshot command is introduced.

## Provider authors and Rust users

The core identity enums no longer mix affiliation and lifecycle. Construct
identities and accounts with explicit `kind`, `affiliation` and `status`; use
Unknown when a source provides no evidence. Add optional resource kind/parent and
explicit grant evidence kind. Use `AccessPath::certainty()` to obtain derived
path certainty without changing the stored source grant.

Existing official native provider packages retain their pinned SDK and frozen
wire DTOs. They continue to communicate through the legacy decoder. Do not update
a native provider's SDK and project unrepresentable new dimensions back into old
wire records. The opt-in [wire-5 contract](../provider-protocol/negotiated-v5.md) carries those
dimensions, but each native emitter must adopt it and pass qualification first.
The bundled adapters can supply the new internal model directly.

## Legacy ingress

Legacy record fields and enum serialization remain accepted unchanged. The host
maps what they actually express:

- Legacy external kind or status establishes external affiliation, not human kind.
- Legacy service status establishes service kind and unknown lifecycle when no
  contradictory known kind is present. A human/bot kind combined with service
  status is contradictory; principal kind remains unknown.
- Legacy active and inactive retain their lifecycle meanings. Legacy external,
  service and unknown statuses do not establish active lifecycle.
- Ordinary human/service/bot kinds retain their meaning. Other affiliation and
  legacy account lifecycle remain unknown.
- Old resources have no observed kind or parent. Old grants have unknown evidence
  kind. The host does not guess these fields from role names or provenance text.

A legacy provider that reports inactive for multiple provider-side states cannot
retroactively supply a suspended distinction. Only a source that explicitly
observes suspension can emit it in the new internal model. This limitation is
particularly relevant when comparing bundled and native Google adapter versions.
