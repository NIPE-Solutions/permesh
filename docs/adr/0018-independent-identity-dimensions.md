# ADR 0018: Independent identity dimensions

Status: Accepted for the domain model; new wire emission remains unnegotiated

## Decision

Represent principal kind (human, service, bot, unknown), affiliation (internal,
external, unknown) and lifecycle (active, inactive, suspended, unknown)
independently for canonical identities. Provider accounts retain their own
observed principal classification and affiliation. An account is not an employee.
Unknown remains a valid value in every dimension; providers must not fill gaps
with guesses. Google Directory suspension is distinct from archival/inactivity.

Keep stable native account keys and canonical authority IDs. Correlation remains
exact and ambiguity-preserving. Conflicting authoritative dimensions must remain
visible as conflicts rather than allowing one provider's order to win.

## Migration

The current `external` kind/status and `service` status are legacy wire concepts.
Dedicated old-wire mappers can recover affiliation or kind, but cannot invent
lifecycle. Conflicting legacy claims require conservative unknown/conflict
handling. Freeze old DTOs before removing those variants from core. New CLI
output must use an explicitly versioned schema rather than silently changing
the meanings of schema-1 fields. Old wire cannot carry every new combination;
providers need negotiated record support before emitting new dimensions.

Orphan review checks inactivity/suspension before intentional service/bot/external
classification. An inactive service identity with access still merits review;
an unmatched active service account is not automatically a former employee.

The implementation uses access JSON schema 2 and retains legacy wire DTOs with
conservative ingress mapping. See the [migration guide](../migrations/domain-schema-2.md).
