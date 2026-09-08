# Negotiated discovery protocol 1

The negotiated family starts at `protocol_version: 1`. The distinct field
separates it from experimental legacy `protocol: 1`–`protocol: 4` envelopes;
the same numeric value cannot silently select another family. Future compatible
operations and capabilities do not increment the version. Only incompatible
wire changes require a version change.

This prerelease contract supports configured native discovery and health. It is
independent of workspace schema 1 and CLI access-output schema 2. Drafts 1–4
retain their historical fields, enums and behavior. Negotiated protocol 1 is not yet stable.

## Select it explicitly

A trusted provider must implement this contract before an instance opts in:

```yaml
version: 1
organization:
  name: Example
providers:
  - id: internal-main
    type: external
    external:
      provider: internal
      sha256: REVIEWED_EXECUTABLE_SHA256
      discovery_protocol: negotiated_v1
      configuration: {}
      credentials:
        token: env://PERMESH_INTERNAL_TOKEN
```

Replace the digest placeholder with the reviewed executable's real digest and
use the existing explicit native registration/review/approval workflow. Changing
`discovery_protocol` invalidates this instance's workspace approval. It cannot
execute or resolve its credentials until approved. Omitted or explicit `legacy`
selects the existing draft-2 configured discovery and health behavior; the default
field is omitted when serialized, preserving old approval inputs.

`user`, `admins`, `orphaned`, `doctor` and `provider status` use the selected
workspace discovery protocol. Standalone `provider external discover` remains the
legacy unconfigured draft-1 operation. Setup and browser-authentication descriptions
remain drafts 3 and 4. No background detection or fallback runs an alternative
contract. Currently published official providers retain their existing defaults.

## Handshake and operation negotiation

Each process serves one operation. The host sends a single LF-terminated frame:

```json
{"protocol_version":1,"id":"handshake","method":"handshake","instance":"internal-main","operation":"discover"}
```

The provider replies:

```json
{"protocol_version":1,"id":"handshake","event":"handshake","provider":"internal","capabilities":["accounts","resources","grants"],"operations":["check","discover"],"draft":true}
```

The selected wire version, provider ID and record capabilities must match the
host's expectations. Record capabilities are the six known categories:
`identities`, `accounts`, `resources`, `groups`, `memberships`, `grants`.
They must be unique and match the trusted registration exactly, ignoring order.
The required operation must appear in `operations` before the host sends any
invocation data. Duplicate, malformed or excessive declarations fail closed.

At most 16 unique operation names are allowed. Each is 1–64 ASCII bytes, starts
with a letter, and contains only letters, digits, `_` and `-`; matching is
case-sensitive. Operations are identifiers, not terminal text. Unknown advertised
operations are optional information and do not permit invocation. The host only
selects `check` or `discover`. Adding a compatible operation to this declaration
does not increment the wire version. The host currently supports only version 1
in this family and never downgrades after failure.

After successful validation, the invocation retains the existing private shape:

```json
{"protocol_version":1,"id":"discover","method":"discover","configuration":{},"credentials":{"token":"synthetic-example-only"}}
```

Only this instance's configured credential slots are sent. Credentials never
appear in arguments or inherited environment. Invocation buffers are zeroized;
provider stderr is bounded and discarded. Recognized credential reflection in
responses rejects the exchange. Native code still has the privileges of the user
who runs it; process isolation is not a sandbox.

## Records and evidence

Record frames use `event: record`, a `kind`, and a `data` object, with
`protocol_version: 1` and `id: discover`. DTOs live under
`permesh_provider_protocol::negotiated::records`; they are independent of core
Rust types. Deserialization alone does not validate a graph.

| Record | Wire semantics |
| --- | --- |
| Identity | Required `id`, `kind`, `affiliation`, `status`, `verified_emails` |
| Account | Required scoped `key`, `login`, `kind`, `affiliation`, `status`, `verified_emails` |
| Resource | Scoped `key`, `name`, nullable `kind`, nullable `parent` |
| Group | Scoped `key`, `name` |
| Membership | Account/group `member`, destination `group`, provenance |
| Grant | `id`, `subject`, `resource`, native `role`, normalized `privilege`, `evidence_kind`, `certainty`, provenance |

Kinds are `human`, `service`, `bot`, `unknown`; affiliations are `internal`,
`external`, `unknown`; lifecycle statuses are `active`, `inactive`, `suspended`,
`unknown`. None implies another dimension. Evidence kinds are `permission`,
`assignment`, `policy_attachment`, `unknown`. Certainty is `observed`, `derived`,
`inferred`, `unknown`. Native role names and provenance remain intact.

Containment parents must exist in the same provider snapshot, without cycles.
Resource kinds use provider-owned names such as `internal.application`.
Containment does not imply access inheritance. Membership paths can derive access
from observed assignments without claiming proven effective authorization.

## Completion, health and errors

Discovery terminates with the exact record count, scope completeness and bounded
known limitation codes:

```json
{"protocol_version":1,"id":"discover","event":"complete","count":0,"complete":false,"limitations":["visibility_limited"]}
```

Partial completion requires a limitation. A health response uses `id: check`,
`event: health`, `status: ok`, and a limitations array; it cannot carry discovery
records. Completion is not enough: the host requires EOF, a successful process
exit, and whole-snapshot validation before exposing results. Malformed exchanges
never return their accumulated records as useful partial results.

Errors use fixed classifications. An optional `provider_code` may carry a bounded
identifier of 1–64 bytes containing only `[a-z0-9._-]` for contract diagnostics.
It must not contain secrets. Validation is
not proof that a string is non-secret, so the CLI does not display this value or
arbitrary provider messages. Provider errors invalidate the exchange.

The limits remain 1 MiB per LF-terminated frame, 64 MiB per transcript, and
100,000 records. The host retains its five-second handshake, 60-second operation,
one-second cooperative cancellation and five-second cleanup deadlines, with
64 KiB of discarded stderr. These do not reset when output arrives. `cancel` uses the selected wire version. Failure does not
trigger another process under a different version.

## Compatibility

- Internal domain changes do not change the wire DTOs.
- Unknown fields, enum variants, record capabilities and protocol versions reject.
- Unknown bounded advertised operations do not reject by themselves, but cannot
  satisfy a missing required operation or widen trusted record capabilities.
- Changed field meaning, type, requiredness or enum spelling requires a new
  incompatible wire contract. Do not edit historical fixtures to hide a change.
- Provider outputs are sorted only after validation; API ordering cannot bypass
  duplicate-record or reference checks.

A [Python example](../../examples/negotiated-provider/README.md) exercises this
contract independently of Rust. The `validate_negotiated` Cargo example checks
transcripts offline and reports counts only; it does not trust or launch code.

See [normalization](../provider-development/normalization.md),
[versioning](versioning.md), and [ADR 0023](../adr/0023-negotiated-discovery-contract.md).
