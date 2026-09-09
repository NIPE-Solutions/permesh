# Provider development

Build a provider when your system has access metadata Permesh should understand.
The provider authenticates, fetches and normalizes observations; the CLI owns
identity correlation, queries and presentation. An internal provider can stay in
its own repository.

Start with the official repository’s
[build-and-test walkthrough](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/development.md).
This page maps the shared contracts a provider author needs.

## Learn the contracts

| Concern | Reference |
| --- | --- |
| Stable IDs, identity authority and access evidence | [Normalization](provider-development/normalization.md) |
| Domain concepts and provenance | [Domain model](DOMAIN_MODEL.md) |
| Provider responsibilities and capabilities | [Provider model](PROVIDER_MODEL.md) |
| Wire messages, framing and bounds | [Protocol specification](provider-protocol.md) |
| Negotiated v1 versus frozen legacy exchanges | [Versioning](provider-protocol/versioning.md) |
| Declarative questions and named credential slots | [Setup schema](provider-setup.md#provider-owned-schema-cli-owned-questions) |
| Execution, hashing and workspace approval | [External providers](external-providers.md) |
| Proxy and additional CA context | [Networking](networking.md) |

Current candidates use negotiated v1 for health and discovery. Legacy setup and
browser description exchanges remain separate. Read the
[candidate adoption guide](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/negotiated-v1.md)
and use a compatible host. Protocol v1 is not yet declared stable.

## Choose an implementation

Rust adapters can use the shared SDK and the official native runtime. Other
languages can implement the public NDJSON contract without depending on internal
Rust domain types. Use explicit wire DTOs and validation at the boundary.

The [synthetic Python example](../examples/external-provider/README.md) demonstrates
interoperability and offline validation. It is not a production installation path:
registration accepts self-contained native executables, not scripts, shell commands
or interpreter invocations. No repository plugin directory is executed automatically.

## Preserve what the API actually proves

Return immutable, provider-scoped IDs and native role names. Keep observed
assignments separate from claims about effective authorization. Preserve resource
parents, membership paths, provenance, known limitations and partial completeness.
Never turn an inaccessible endpoint into an apparently complete empty result.

Identity assertions become authoritative only when the user explicitly selects
that source. Never use public profile email as verified evidence or infer employment
from a provider account’s active status. Follow the normalization guide for
independent lifecycle, kind and affiliation fields.

## Keep execution and credentials explicit

The host validates the operation handshake before sending configured values and
only the named credentials required by that instance. Do not print, log, or echo
credential values in errors. Stdout is protocol-only; arbitrary provider diagnostics
must not become user-facing messages. The child receives a sanitized environment.

Setup descriptions declare fields and conditions; the CLI renders prompts and
validates answers. Description operations do not receive answers or credentials.
Native provider code runs with the user’s privileges: trust and protocol validation
are not an OS sandbox.

## Test the full boundary

Use synthetic fixtures and local mock APIs. Test normalized snapshots through the
shared SDK contract validator and the emitted transcript through the protocol
decoder. Cover duplicate IDs, invalid references, hierarchy, lifecycle, certainty,
limitations and incomplete responses.

Test API pagination, throttling, denied permissions, malformed responses and empty
results. Add credential-redaction and process-level tests for incorrect handshakes,
extra frames, floods, hangs, cancellation and crashes. The host’s tests cover
trust and cleanup; they do not replace adapter-specific normalization tests.

Run `cargo test --locked -p permesh-provider-protocol` in this repository for
protocol fixtures and `python3 scripts/check_protocol_example.py` for the explicit
Python interoperability check. The official provider repository has its own
[required checks](https://github.com/NIPE-Solutions/permesh-providers/blob/main/CONTRIBUTING.md).

## Ship an honest integration

Document permissions, authentication, observed data, unsupported access paths,
rate limits, enterprise endpoint support and security implications. Source
implementation, live validation and catalog publication are different milestones.
See [provider packages](provider-packages.md) and the official
[release qualification guide](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/releasing.md).
