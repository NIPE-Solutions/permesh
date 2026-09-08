# ADR 0023 — Negotiate operations within a pinned discovery wire contract

Status: Accepted for prerelease implementation; negotiated protocol 1 is not declared stable.

## Context

Legacy drafts 1–4 use operation-specific numbers. Their record types combine
principal kind, affiliation and lifecycle, and cannot represent the current domain
without dropping information. Reinterpreting those fields would break published evaluation
native providers and make historical evidence ambiguous.

## Decision

Keep every legacy draft unchanged. Add an explicit `negotiated_v1` discovery
selection in workspace configuration. Negotiated protocol 1 introduces independent record DTOs
and carries both health and discovery under the same wire version. The handshake
names the required operation; the provider advertises supported operations and
record capabilities. The host validates the required operation and the exact
trusted record-capability set before delivering configuration or credentials.

The negotiated family starts at **version 1**, before public adoption. Its envelope
uses `protocol_version: 1`; the experimental legacy envelopes use `protocol: 1`
through `protocol: 4`. Distinct required fields prevent numeric overlap from
confusing decoders or permitting fallback. This is a one-time naming reset before
stability, not permission to reset versions after adoption.

A future compatible operation does not need a new protocol version. Only
incompatible wire changes justify a new version. There is currently one supported
version in this negotiated family. The host pins it explicitly; no highest-version
search, automatic probing, retry against a legacy draft, or silent downgrade occurs.

Unknown advertised operation names are bounded optional information. They never
allow the host to execute an unknown operation. Unknown record capabilities remain
errors because those capabilities are part of the executable's explicit trust.
Unknown fields and enum variants remain errors. No arbitrary extension map is
introduced without a concrete compatibility need.

Selection is included in the provider-scoped workspace approval. Omitting the
selection retains exactly the old serialized provider configuration, so unrelated
upgrades do not revoke existing legacy approvals. Selecting a different contract
requires review and approval before secret resolution.

Setup and browser authentication retain their existing independently selected
drafts in this slice. Official artifacts and SDK pins do not change automatically.
A provider must implement negotiated protocol 1 before its workspace opts in.

## Alternatives

Extending draft 2 in place would silently change a public schema. Incrementing the
version separately for health and discovery would repeat the original coupling.
Automatic fallback would obscure which contract received credentials. A general
multi-version negotiation framework is unnecessary while only one new contract
exists; explicit pinning keeps compatibility and failure behavior reviewable.

## Consequences

Provider authors can represent external inactive humans, suspended services,
resource containment and policy-attachment evidence without semantic loss. Old
providers still work through conservative legacy mapping. Authors temporarily
support separate discovery and setup/auth contracts. A later SDK/provider rollout
must be qualified before changing official installation defaults.

See [negotiated protocol 1](../provider-protocol/negotiated-v1.md) and
[domain migration](../migrations/domain-schema-2.md).
