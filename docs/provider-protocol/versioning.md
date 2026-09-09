# Provider protocol versioning

The existing native NDJSON protocol uses four **operation-specific drafts**.
The number does not mean that a provider accepting draft4 also accepts draft1–3.
The host pins the complete exchange to the selected operation's draft and never
falls back after a version or handshake failure.

| Draft | Host operation | Invocation data |
| --- | --- | --- |
| 1 | Unconfigured discovery | Instance in handshake; no credentials |
| 2 | Configured discovery or health check | Configuration and named credentials after validated handshake |
| 3 | Setup description | No configuration answers or credentials |
| 4 | Browser authentication description | No configuration, credentials, authorization codes or browser state |

The handshake's `capabilities` are record categories, not a declaration that all
operations or protocol drafts are supported. Registered capabilities must match
exactly, apart from ordering. Unsupported drafts, unknown record capabilities,
unknown schema fields, repeated events and malformed frames fail closed. A
discovery result requires explicit completion, EOF, a successful process exit
at the host, and domain validation. Explicit partial completion remains partial.

## Frozen record contract

`permesh_provider_protocol::records` owns the discovery record DTOs for drafts 1
and 2: records, entity keys, subjects, provenance and their enums. These are
independent Rust types, not aliases of `permesh_core` records. Field spelling,
required fields, enum strings and serialization order preserve the historical
contract. The six wire capability values are also independent of SDK metadata;
adding an SDK capability cannot widen a historical draft's handshake. SDK-owned
setup and browser-authentication descriptions are already boundary-owned and retain their
existing versioned schemas.

The host converts fields and enum variants explicitly into domain records and
validates the assembled snapshot before returning it. Mapping is crate-private; there is no public unchecked `From`/`Into` conversion.
Converting an individual DTO internally is not validation: dangling references, duplicates and
cross-instance keys can only be checked with the complete snapshot. Use the
operation decoder for incoming frames; standalone DTO deserialization does not
replace framing, envelope validation or session ordering.

Historical fixtures live in
`crates/permesh-provider-protocol/tests/fixtures`. They are literal baseline
transcripts, and must continue to decode when internal models change. A future
domain field needs an explicit mapping decision; it must not appear in an old
wire draft merely because a domain struct gains a serde field. Existing
providers do not need to change their bytes or configuration for this extraction.

## Negotiated protocol 1

Protocol versions describe wire compatibility. Capabilities describe supported
records and operations. Negotiated protocol 1 now carries
health and discovery with separate operation negotiation. Adding a compatible
optional operation does not by itself increment that version. It is opt-in and
not declared stable.

| Change | Existing drafts 1–4 | Negotiated protocol 1 rule |
| --- | --- | --- |
| Internal domain field or enum | No wire change; explicit mapper decision | No wire change |
| Unknown envelope/record field | Reject, including additive fields | Reject unless a named bounded extension field explicitly permits it |
| Unknown enum value | Reject | Reject unless that field defines an explicit unknown-value representation |
| Unknown capability | Reject; exact registered set required | Reject unknown record capabilities; ignore bounded unknown optional operation names without granting execution permission |
| New operation | Current operation-specific drafts remain pinned | Required operation must appear in the handshake before invocation data is delivered |
| Renamed/removed field, changed type, enum spelling or requiredness | Incompatible; do not modify a frozen draft | Requires a new incompatible wire version and migration |
| Documentation, tighter provider implementation, internal optimization | Compatible if previously valid contract data remains valid | Same |

A newer host accepting frozen old transcripts is backward compatibility. An old
host accepting newly emitted data is forward compatibility. The strict existing
drafts guarantee neither acceptance of new fields nor unknown capabilities;
senders must emit exactly the selected draft. Security validation fixes may
intentionally reject previously accepted invalid input and must be documented.

Negotiated protocol 1 is explicitly pinned and negotiates operation support separately from
record capabilities. Only this version is currently supported in the new family;
there is no automatic downgrade or probe. Selecting the new contract in provider
configuration requires fresh approval. Unknown operation declarations are bounded
optional information, not executable requests. See the [negotiated protocol 1 specification](negotiated-v1.md)
and [ADR 0023](../adr/0023-negotiated-discovery-contract.md).

See [ADR 0016](../adr/0016-wire-contracts.md) and the
[current protocol](../provider-protocol.md).

## Domain schema migration

The host now maps legacy records into independent principal dimensions, optional
resource containment and explicit evidence categories. This does not add fields
or enum values to any legacy wire draft. Unexpressed values remain unknown.
Access CLI JSON uses its own schema 2; this is not a protocol version increment.
See [migration mappings](../migrations/domain-schema-2.md). Native provider SDK
updates must explicitly adopt and qualify the negotiated records rather than
silently dropping new semantics into a legacy projection. Official provider pins
and published artifacts are not changed by host support.

## Optional network feature

Negotiated v1 requests may include `features: ["network_v1"]` when an approved
network context is required. The provider acknowledges exactly that list in its
handshake. Missing, null, duplicate, unknown or unsolicited feature lists fail
closed. Existing no-feature request/response fixtures stay byte-compatible.
Only after agreement may an invocation include a validated `network` context.
Operation names and discovery record capabilities do not imply network support.
See [network context](../networking.md). Old providers reject the optional request;
the host does not downgrade or send credentials after rejection.
