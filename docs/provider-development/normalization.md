# Normalizing provider observations

Permesh answers who has access, through which path, and how much of that answer
the provider could observe. A snapshot is evidence gathered over an interval,
not a transaction across provider APIs or proof of every effective permission.

This guide describes the current draft record contract. The independent identity,
resource and evidence extensions in ADRs 0018–0020 remain proposals; do not emit
their fields into drafts 1 or 2.

## Identities and accounts

Use immutable provider IDs for account keys, scoped by instance. Display names,
login names and email addresses are labels, not replacement keys. A canonical
identity may represent a human, service, bot or unknown principal. Only emit
identities when the provider has evidence for them; account discovery does not
automatically make a provider an identity authority.

Good: Google customer ID plus immutable user ID identifies a directory principal;
its authoritative primary address is the asserted email. Bad: merge a GitHub
login with an employee whose name looks similar, or promote public profile email
to a verified organization identity. Host configuration selects authorities and
reviewed aliases; adapters do not own cross-provider correlation.

The legacy kind/status enums overlap. Do not invent employment or lifecycle to
fill those fields. Preserve unknowns. Future independent affiliation/lifecycle
fields will require explicit wire negotiation, not extra undeclared JSON fields.

## Resources, groups and relationships

Use stable IDs for resources and groups even when renamed. Membership records
must identify the actual account/group and parent group, retaining observed
source and UTC timestamp. Do not flatten nested teams into direct grants.
Duplicate identical observations may be deduplicated; conflicting observations
must reduce completeness or fail discovery rather than silently choosing one.

Current resources have key and name only. Clearly label evidence scopes in
provider documentation, especially AWS policies and Cloudflare wildcard scopes.
Future parent-resource containment must not imply permission inheritance.
Validate references after collection; do not emit dangling relations when a
restricted API hides a referenced account or group.

## Access and certainty

Keep the native role alongside normalized privilege. GitHub `maintain` may map to
`elevated`; the native role must remain `maintain`. AWS `AdministratorAccess`
attachment alone does not prove effective admin authorization and should retain
unknown privilege unless the adapter has stronger evidence.

`observed` means the source reported the record. It does not mean every request
will be authorized. `inferred` must be justified in provider documentation;
`unknown` must remain available. A path through observed memberships explains
the relationships, but does not account for hidden denies, branch protection,
session conditions or policies the API did not expose.

Good: report a policy attachment with its ARN, native name and documented limits.
Bad: transform that attachment into an assertion that all AWS actions are allowed.
Do not encode deny evidence as a positive grant.

## Provenance and completeness

Record a meaningful observation method and RFC 3339 UTC time for grants and
memberships. Preserve timestamps in fixtures. Do not fetch messages, repository
contents or documents merely to enrich labels.

Return `complete: false` when an intended collection is truncated, permission
denied, rate-limited beyond retry bounds or inconsistent. Attach supported
limitation categories; the host renders curated descriptions. A complete
collection can still have documented scope limits: neither complete nor observed
means universal effective authorization. A malformed, unterminated or crashed
exchange must not be disguised as an empty successful snapshot.

## Provider review checklist

- Stable keys survive renames and reordered API pages.
- No duplicates, cross-instance references or unexplained conflicting records.
- Pagination, rate limits, denial, malformed payloads and cancellation have mock tests.
- Native runtime output decodes through the host, including negative operations.
- Unknown privilege, partial completion and source limitations survive the query.
- Credentials never appear in output, diagnostics, fixtures or stored configuration.
- Required scopes, revocation, supported authentication and unobservable paths
  are documented. Synthetic qualification is distinguished from live tenant tests.

Use the [protocol fixtures](../../crates/permesh-provider-protocol/tests/fixtures)
and [versioning rules](../provider-protocol/versioning.md). The SDK's snapshot
validator is necessary but does not prove provider-specific semantic correctness.
