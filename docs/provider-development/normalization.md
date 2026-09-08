# Normalizing provider observations

Permesh answers who has access, through which path, and how much of that answer
the provider could observe. A snapshot is evidence gathered over an interval,
not a transaction across provider APIs or proof of every effective permission.

This guide distinguishes the new internal model from the legacy wire contract.
ADRs 0018–0020 are implemented in core, but their new fields cannot be emitted
in drafts 1 or 2. Opt-in [negotiated protocol 1](../provider-protocol/negotiated-v1.md) carries the new dimensions.
Official native packages retain their compatible pins until their protocol-1 emitters
and provider semantics have been qualified; changing host support does not upgrade
a provider automatically. See the
[migration guide](../migrations/domain-schema-2.md).

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

Internally, kind (human/service/bot/unknown), affiliation (internal/external/unknown)
and lifecycle (active/inactive/suspended/unknown) are independent for identities
and accounts. A service principal may be inactive; an external human may be active.
Account lifecycle does not overwrite the canonical authority.

The old wire enums overlap. Their explicit ingress mapper recovers only evidence
they can express: external does not mean human, and service does not mean active.
Do not append new fields to a strict legacy envelope. Do not squeeze suspended
into inactive when authoring a new contract, or infer internal affiliation merely
from directory membership.

## Resources, groups and relationships

Use stable IDs for resources and groups even when renamed. Membership records
must identify the actual account/group and parent group, retaining observed
source and UTC timestamp. Do not flatten nested teams into direct grants.
Duplicate identical observations may be deduplicated; conflicting observations
must reduce completeness or fail discovery rather than silently choosing one.

Internal resources include optional provider-owned kinds such as `aws.account`
and a parent key. Legacy resources map to unknown kind and no observed parent.
Clearly label evidence scopes, especially AWS policies and Cloudflare wildcard
scopes. Containment never implies permission inheritance. Parent links must
resolve in the same snapshot and form an acyclic graph; discovery order is irrelevant.
Validate references after collection; do not emit dangling relations when a
restricted API hides a referenced account or group.

## Access and certainty

Keep the native role alongside normalized privilege. GitHub `maintain` may map to
`elevated`; the native role must remain `maintain`. AWS `AdministratorAccess`
attachment alone does not prove effective admin authorization and should retain
unknown privilege unless the adapter has stronger evidence.

Classify internal evidence as `permission`, `assignment`, `policy_attachment` or
`unknown`. The native role remains separate from this category and normalized
privilege. Legacy wire records default to unknown evidence kind: method strings
and policy names are not a substitute for an explicit contract.

`observed` means the source reported the record. It does not mean every request
will be authorized. A path joining observed membership with an observed grant
is `derived`, while the original grant remains observed. `inferred` must be
justified in provider documentation; `unknown` remains available. Neither
derivation nor resource containment accounts for hidden denies, branch protection,
session conditions or unobserved policies. New certainty values also require wire
negotiation before native providers can emit them.

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
- Independent lifecycle and affiliation evidence; inactive service principals remain visible.
- Valid resource kinds, parent references and acyclic containment; no invented access edges.
- Pagination, rate limits, denial, malformed payloads and cancellation have mock tests.
- Native runtime output decodes through the host, including negative operations.
- Unknown privilege, partial completion and source limitations survive the query.
- Credentials never appear in output, diagnostics, fixtures or stored configuration.
- Required scopes, revocation, supported authentication and unobservable paths
  are documented. Synthetic qualification is distinguished from live tenant tests.

Use the [protocol fixtures](../../crates/permesh-provider-protocol/tests/fixtures)
and [versioning rules](../provider-protocol/versioning.md). The SDK's snapshot
validator is necessary but does not prove provider-specific semantic correctness.
