# ADR 0016: Own serialized contracts at their boundaries

Status: Accepted for protocol record extraction; further boundary work follows
the architecture hardening backlog.

## Context

The original provider protocol embedded `permesh_core` records directly in its
serde wire events. Domain field or enum changes could therefore alter a draft
protocol without any protocol change. Strict structural round-trip validation
makes accidental schema drift particularly disruptive: an added serialized
default can reject a previously valid transcript.

The native NDJSON host already provides bounded framing, strict handshakes,
operation-specific drafts, cancellation and snapshot validation. These mechanisms
do not require replacing the transport or widening accepted input.

## Decision

Own drafts 1 and 2 discovery records, nested values and core-related enums in
`permesh-provider-protocol::records`. Keep their historical field layout and
serde representation. Freeze the handshake's six record capabilities separately
from SDK metadata and map them explicitly for registration checks. Map each field and enum explicitly into the domain; do
not use domain serde or JSON conversion as the mapping mechanism. The operation
decoder validates the complete mapped snapshot before exposing it.
The record mappers are crate-private and nested mapping helpers are private;
public DTOs provide no unchecked `From`/`Into` path into domain records.

Preserve drafts 1–4, strict unknown-field rejection, exact record capabilities,
completion/EOF rules and partial-result semantics. Check in complete synthetic
transcripts from the pre-extraction baseline, and retain them unchanged when
adding future versions. Expose record DTOs for native provider authors without
claiming that individual DTO deserialization validates a complete exchange.

Setup and browser authentication specs remain SDK-owned versioned boundary
types. A negotiated protocol will be a separate change with explicit version,
operation and feature rules; no new negotiation is implemented here.

## Consequences

Internal model evolution now requires a deliberate mapping decision instead of
implicitly modifying the external record schema. Some fields and enums are
duplicated intentionally because the wire and domain have different lifetimes.
Mapping and frozen-transcript tests must be maintained together. Existing
providers require no migration for this extraction, and no stored credentials,
workspace approvals or executable trust records change.

The same boundary-ownership principle applies to CLI JSON, but its implementation
and historical output fixtures are a separate slice. This ADR does not declare
all external contracts stable or resolve the remaining hardening backlog.
