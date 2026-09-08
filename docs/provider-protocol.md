# External provider protocol — drafts 1 and 2

The pure `permesh-provider-protocol` crate validates discovery and health
responses. The separate [native host](external-providers.md) requires local binary
trust; workspace execution additionally requires a matching local approval.
These drafts do not promise a stable deployed plugin API.

Draft 1 supports standalone native discovery and the existing offline Python
reference. Draft 2 adds approved workspace discovery and health checks with named
configuration and credential delivery. The caller pins the version for the entire
exchange. A draft-2 invocation rejects a draft-1 response without sending the
operation or credentials; negotiation never downgrades. Workspace YAML cannot
select an executable path.

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

One process handles a handshake followed by one operation. Request IDs
are reserved method names: `handshake`, `check`, `discover`, `cancel`. They are
unique within that process. The synchronous example additionally permits one
check before discovery for manual experimentation; the discovery validator
accepts only the handshake/discovery response sequence.

```json
{"protocol":1,"id":"handshake","method":"handshake","instance":"example-main"}
{"protocol":1,"id":"discover","method":"discover"}
```

In draft 1, handshake has exactly those four fields. Other requests have exactly `protocol`,
`id` and `method`; the integer version is 1 and `id` equals `method`. Instance
and provider-type names contain 1–64 ASCII letters, digits, hyphens or underscores
and begin with a letter. Draft-1 requests never contain secrets or executable
paths. `handshake_request` encodes a draft-1 handshake for Rust consumers.

For draft 2, the handshake has the same four fields with `protocol: 2`. After
successful handshake validation, exactly one operation request adds
`configuration` and `credentials` objects:

```json
{"protocol":2,"id":"handshake","method":"handshake","instance":"example-main"}
{"protocol":2,"id":"check","method":"check","configuration":{"endpoint":"https://internal.example.com"},"credentials":{}}
```

Discovery uses `id: "discover"` and `method: "discover"` with the same two objects.
Configuration is JSON-compatible data. Credential keys are reviewed slot names;
values are resolved credential strings sent only on this private stdin pipe.
Both objects are present, even when empty. They never contain executable paths
or command arguments as host instructions. The host bounds configuration to
64 KiB and 16 levels; at most 16 credential slots are accepted, each value at most
16 KiB and all values together at most 64 KiB. The operation request must fit the
one-frame limit. Its serialized buffer zeroizes on drop.

`handshake_request_versioned(instance, version)` accepts only versions 1 and 2.
A cancellation request retains the selected version and the three fields
`protocol`, `id: "cancel"`, `method: "cancel"`.

## Handshake

```json
{"protocol":1,"id":"handshake","event":"handshake","provider":"synthetic-example","capabilities":["accounts","identities","resources","groups","memberships","grants"],"draft":true}
```

Draft 2 uses the same handshake shape with `protocol: 2`. The expected provider
type and instance come from the validator's caller. The response provider must
match exactly; the host also requires the exact registered capability set,
regardless of order, before sending an operation or any credentials. `draft` must
be true for both supported drafts.
Capabilities must be unique known values from the list above; an empty set is
valid for an empty discovery. No mutation capability is defined. A record of an
undeclared kind is rejected. Declared capabilities describe available record
classes, not proof of complete provider visibility or permission to execute code.

## Discovery records

Each record has the pinned `protocol` (1 or 2), `id: "discover"`, `event: "record"`, `kind` and
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
verified by this parser. Workspace source authority requires explicit approved
configuration and a registration with the `identities` capability; syntax
validation alone never establishes identity truth or trusted code.

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

Draft-2 health has exactly these fields and accepts only `status: "ok"`:

```json
{"protocol":2,"id":"check","event":"health","status":"ok","limitations":[]}
```

`limitations` is mandatory and uses the same fixed codes as discovery. Exactly
one health terminal must follow the handshake, then EOF and successful process
exit. Records or discovery-completion events are invalid in a health exchange.
Any error rejects the health result. The SDK health message is curated by Permesh;
the provider cannot supply arbitrary display text. A health check does not return
or infer an access graph.

For draft-1 manual Python peer checks, a successful health response is
`{"protocol":1,"id":"check","event":"health","status":"ok"}`.
Cancellation acknowledges `{"protocol":1,"id":"cancel","event":"cancelled"}`.
Neither is valid inside the discovery transcript validator.

## Decoder and host

`DiscoveryDecoder` accepts bounded frames incrementally and requires a complete,
valid exchange before returning a snapshot. With expected capabilities supplied,
it rejects a changed handshake capability set before discovery is requested.
A rejected frame permanently invalidates that decoder.
`DiscoveryDecoder::new` keeps draft 1; `new_versioned` pins version 1 or 2.
`HealthDecoder::new` always pins draft 2 and shares the strict framing, handshake,
capability and poisoning rules. Health progress is `Handshake` then `Complete`;
`finish` returns SDK health only after the caller has observed EOF. Discovery
progress additionally includes `Record`. Neither decoder exposes earlier results
after an error. The pure protocol crate never resolves or stores credentials.

The offline `validate_discovery` API accepts blocking `BufRead`; a producer that
stops writing can block it. Use finite transcript files. The separate native
host uses asynchronous pipes, fixed deadlines, concurrent bounded stderr draining,
and explicit process cleanup. See [host limits and trust](external-providers.md).
The host closes stdin after the terminal completion event and requires EOF plus
successful process exit. Cancellation is best effort, followed by termination;
a cancellation acknowledgment is not accepted as successful discovery.
