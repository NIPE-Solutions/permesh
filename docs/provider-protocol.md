# External provider protocol — draft 1

This specification describes the offline discovery validator in
`permesh-provider-protocol` and the synthetic Python peer. It does **not** enable
plugin execution, establish trust, or promise a stable deployed plugin API.
Workspace YAML still cannot select an executable. The earlier illustrative draft
has changed: request IDs are fixed per method, handshake supplies an instance,
capabilities describe records, and records now use the normalized domain schema.

## Transport and budgets

Transport is UTF-8 NDJSON on dedicated stdin/stdout pipes. Every frame ends in LF.
CRLF is accepted and both bytes count. Blank lines, multiple JSON values per
frame, malformed UTF-8/JSON, duplicate keys at any nesting depth (including
escaped spellings), unknown fields and excessive JSON nesting are rejected.
Object key order and whitespace are insignificant. The default serde_json
recursion limit remains enabled.

| Limit | Inclusive maximum |
| --- | --- |
| One frame, including LF | 1,048,576 bytes |
| Complete response transcript, including handshake and terminal frames | 67,108,864 bytes |
| Discovery record events | 100,000 |

The reader bounds consumption before allocating a complete line, probing at most
one byte beyond the remaining budget to detect overflow. Parsed records and
validation indexes consume additional memory; the byte ceiling is an input
budget, not an exact resident-memory guarantee. No raw frame is logged.

## Requests

One future process handles a handshake followed by one operation. Request IDs
are reserved method names: `handshake`, `check`, `discover`, `cancel`. They are
unique within that process. The synchronous example additionally permits one
check before discovery for manual experimentation; the discovery validator
accepts only the handshake/discovery response sequence.

```json
{"protocol":1,"id":"handshake","method":"handshake","instance":"example-main"}
{"protocol":1,"id":"discover","method":"discover"}
```

Handshake has exactly those four fields. Other requests have exactly `protocol`,
`id` and `method`; the integer version is 1 and `id` equals `method`. Instance
and provider-type names contain 1–64 ASCII letters, digits, hyphens or underscores
and begin with a letter. Requests never contain secrets or executable paths.
`handshake_request` encodes a validated handshake for Rust consumers.

## Handshake

```json
{"protocol":1,"id":"handshake","event":"handshake","provider":"synthetic-example","capabilities":["accounts","identities","resources","groups","memberships","grants"],"draft":true}
```

The expected provider type and instance come from the validator's caller.
The response provider must match exactly. `draft` must be true for this draft.
Capabilities must be unique known values from the list above; an empty set is
valid for an empty discovery. No mutation capability is defined. A record of an
undeclared kind is rejected. Declared capabilities describe available record
classes, not proof of complete provider visibility or permission to execute code.

## Discovery records

Each record has `protocol: 1`, `id: "discover"`, `event: "record"`, `kind` and
`data`. `data` is the corresponding normalized domain object, with every field
required and no extra fields accepted, including within nested keys/subjects.

| Kind | Capability | Required data fields |
| --- | --- | --- |
| `identity` | `identities` | `id`, `kind`, `status`, `verified_emails` |
| `account` | `accounts` | `key`, `login`, `kind`, `verified_emails` |
| `resource` | `resources` | `key`, `name` |
| `group` | `groups` | `key`, `name` |
| `membership` | `memberships` | `member`, `group`, `provenance` |
| `grant` | `grants` | `id`, `subject`, `resource`, `role`, `privilege`, `certainty`, `provenance` |

A key is `{"provider":"example-main","id":"native-id"}`. A subject is
`{"kind":"account","key":{...}}` or `{"kind":"group","key":{...}}`.
Provenance contains `method` and UTC RFC 3339 `observed_at`. Domain enums and
identity rules are defined in [DOMAIN_MODEL.md](DOMAIN_MODEL.md); the
[complete synthetic exchange](../examples/external-provider/discovery.ndjson)
shows all six record types.

All entity keys must use the caller's instance ID. Identity IDs remain canonical
IDs, as in the domain model. Immutable IDs must be stable; display names are not
substitutes. Verified emails are assertions by the provider, not independently
verified by this parser. A future host must enforce explicit source authority;
syntax validation alone never establishes identity truth or trusted code.

The strict wire shape is checked by a structural serialization round trip. Changes to domain serialization (especially defaults, omitted fields or enum representations) therefore require protocol compatibility review and fixture updates.

Records can arrive in any order, including forward references. After completion,
core validation rejects duplicate IDs/memberships, dangling relationships,
cross-instance keys, and invalid provenance. Valid snapshots are sorted by the
existing domain rules before being returned. Original roles, certainty and
membership paths are retained.

## Completion and failure

```json
{"protocol":1,"id":"discover","event":"complete","count":6,"complete":true,"limitations":[]}
```

Exactly one terminal event is required, followed by EOF. `count` is the number
of record events across all kinds. A mismatch, extra frame, wrong ID, unexpected
event, or EOF before completion fails the entire exchange. A valid
`complete: false` terminal can retain a partial snapshot only when it includes
at least one limitation code. Codes are `visibility_limited`, `permission_denied`,
`rate_limited`, `page_limit`, `unknown`; the consumer maps them to curated text.
Known visibility limits may also accompany `complete: true`, which means
collection completed under those limits, not that access is exhaustively known.

An error event has exactly `protocol`, `id`, `event: "error"` and `code`.
Known codes are `protocol_error`, `unsupported_method`, `handshake_required`,
`authentication`, `permission_denied`, `rate_limited`, `unavailable`, `internal`.
Any error event fails discovery without returning earlier records. Arbitrary
error messages/details are not accepted or echoed. Parser errors are fixed
categories and never contain raw input or operating-system diagnostics.

For manual peer checks, a successful health response is
`{"protocol":1,"id":"check","event":"health","status":"ok"}`.
Cancellation acknowledges `{"protocol":1,"id":"cancel","event":"cancelled"}`.
Neither is valid inside the discovery transcript validator.

## What remains before execution

The validator accepts a blocking `BufRead` and consumes it through EOF. It does
not spawn, authenticate, sandbox, time out, cancel or reap a process. A producer
that stops writing without closing its stream can therefore block this offline
tool. Use finite transcript files for validation; this API is not an approved
live-process host.

Execution remains gated on the [local trust and supervision design](provider-development.md#future-local-trust-registration): executable identity, replacement races,
minimal environment, native process-tree termination, fixed overall deadlines,
bounded concurrent stderr draining, and hostile-process tests on all platforms.
The proposed host deadlines remain 5 seconds for handshake, 60 seconds for an
operation and 1 second of cancellation grace. Passing transcript tests does not
satisfy those process security requirements.
