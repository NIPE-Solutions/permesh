# Synthetic negotiated discovery provider (wire 5)

This standalone Python 3 example demonstrates the draft wire-5 contract independently
of the Rust SDK. It uses only the standard library, reads NDJSON on stdin and writes
NDJSON on stdout. It makes no network calls, reads no environment variables, and
returns fixed synthetic data. Input and output frames are limited to 1 MiB including
LF. Requests reject unknown or duplicate fields, invalid versions, wrong operation
ordering, oversized configuration/credentials and malformed JSON. Credentials are
validated and discarded without being logged or reflected.

Run from the repository root:

```sh
python3 -m unittest discover -s examples/negotiated-provider -v
```

A complete discovery exchange (shell syntax for macOS/Linux):

```sh
python3 examples/negotiated-provider/provider.py <<'EOF_REQUESTS'
{"protocol":5,"id":"handshake","method":"handshake","instance":"example","operation":"discover"}
{"protocol":5,"id":"discover","method":"discover","configuration":{},"credentials":{}}
EOF_REQUESTS
```

The response is a handshake, seven records and a completion record. The records
include an inactive external service identity/account, organization/repository
hierarchy, group membership and a derived policy-attachment grant. Every identity
and account explicitly supplies `kind`, `affiliation` and `status`; every resource
supplies nullable `kind` and `parent`; grants supply `evidence_kind`. These are
independent wire DTOs, not the legacy v1/v2 records. Schema-2 CLI query output
preserves the new dimensions; health/control output remains schema 1.

For health, change the handshake operation and the second request's `id` and `method`
to `check`. The result is a single `health` event with `status: "ok"`. The process
serves one selected operation and expects EOF afterward. A `cancel` request may
replace the operation request. Errors exit with code 2 and do not print input or
exception details.

The handshake advertises `operations: ["check", "discover"]` separately from its
record capabilities. A host must verify the selected operation and the exact
registered capability set before sending an invocation with credentials. Wire 5
is explicitly selected; unsupported or malformed negotiation must fail without
falling back to wire 1/2. Setup and browser-auth contracts are separate.

## Native CLI integration

Permesh's external-provider trust workflow accepts native executables, not Python
scripts or generic interpreter registrations. Run this example manually as above;
do not trust the Python interpreter as a shortcut. A deployable implementation must
package a provider-specific native executable and follow the ordinary inspect,
trust, workspace review and approval workflow.

For such a native implementation, the workspace selector is:

```yaml
external:
  provider: synthetic-example
  sha256: <sha256-of-provider-native-executable>
  discovery_protocol: negotiated_v5
  configuration: {}
  credentials: {}
```

This snippet belongs inside an external provider instance; it is not a complete
workspace. Changing `discovery_protocol` invalidates the instance approval. Review
the new fingerprint and approve it before querying. Omitting the selector retains
the legacy configured protocol. The example does not change the official SDK pins
or establish wire 5 as stable.
