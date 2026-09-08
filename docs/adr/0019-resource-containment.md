# ADR 0019: Provider-owned resource kinds and containment

Status: Accepted for the domain model; new wire emission remains unnegotiated

## Decision

Add an optional validated provider-namespaced resource kind and optional parent
EntityKey. Legacy resources have unknown kind and no observed parent. Names
remain labels; keys remain stable provider IDs. Examples are `github.repository`,
`github.organization`, `aws.account`, `aws.iam.policy` and `cloudflare.zone`.

Validate bounded ASCII namespace segments, parent existence, same provider
instance, no self-parent and acyclic containment. Do not enumerate every provider's
resource kind in a universal Rust enum. A hierarchy is inventory evidence; it
never creates access grants or membership inheritance by itself.

## Consequences

Provider contracts must test missing parents, duplicate IDs, cycles and shuffled
input order. Parent links can arrive out of order during discovery, but a complete
snapshot is accepted only after graph validation. New fields require negotiated
wire support and an intentional output-schema migration; old records remain valid
through the compatibility mapper.

The implementation uses access JSON schema 2 and retains legacy wire DTOs with
conservative ingress mapping. See the [migration guide](../migrations/domain-schema-2.md).
