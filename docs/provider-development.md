# Provider development

Providers normalize scoped native IDs, distinguish observation from effective
authorization, preserve provenance and report incomplete visibility. Core owns
correlation. Read [PROVIDER_MODEL.md](PROVIDER_MODEL.md),
[DOMAIN_MODEL.md](DOMAIN_MODEL.md), the bundled demo, and the
[external GitHub adapter](https://github.com/NIPE-Solutions/permesh-providers/tree/main/providers/github). Never turn
permission denial into an apparently successful empty result. Use synthetic
fixtures and local mock APIs for contract tests.

## Protocol and native execution

The pure `permesh-provider-protocol` crate validates bounded draft-1 and draft-2
discovery, draft-2 health and draft-3 setup descriptions. Read the [wire specification](provider-protocol.md).
The [Python example and offline validator](../examples/external-provider/README.md)
remain draft-1 interoperability references exercising all six normalized record
kinds. Scripts and interpreter commands cannot be registered as native providers.

Build a self-contained native binary for each target platform. Its handshake
provider type must equal the registered ID; capabilities must match registration
exactly. Standalone `provider external discover` speaks draft 1 and supplies no
configuration or credentials. Workspace operations require draft 2 and a matching
local approval. See the [registration and approval workflow](external-providers.md).
There is no automatic download, PATH lookup or executable selection from YAML.

For draft 2, accept the handshake before reading one `discover` or `check` request.
Only after exact handshake validation does the host send configuration and named
credentials over stdin. Handle those values as secrets: do not print, log or echo
them. Health returns the strict `health/ok` terminal with known limitation codes;
it must not enumerate the graph. Discovery retains the normalized record and
completion shapes with `protocol: 2`. Terminate with EOF and a successful exit.
Never rely on a draft-1 fallback for a configured invocation.

Return identity assertions only within documented provider visibility. Authority
requires explicit approved source selection and registered `identities` capability;
a syntactically valid identity is not automatically authoritative.

Use native fixtures to test malformed/extra/partial frames, wrong versions and
capabilities, missing health limitations, floods, hangs, cancellation, descendants,
changed executable bytes, rejected approvals and escaped credential reflection.
The parser alone cannot satisfy execution or credential-delivery requirements.
External code runs as the user's account; neither the protocol nor registration
provides a sandbox.

For guided setup, implement draft-3 handshake/describe and return SDK setup schema
1. The CLI owns prompts and validates conditional answers locally; the provider
receives no answers or credentials in this exchange. Keep draft-2 query support.
Read the [setup contract](provider-setup.md#provider-owned-schema-cli-owned-questions)
and [synthetic examples](../examples/setup/README.md). Catalogs, dynamic
provider-driven steps and interpreter/dependency-bundle trust remain deferred.

See [normalization guidelines](provider-development/normalization.md) for stable
identifiers, evidence, authority and completeness rules, and [versioning](provider-protocol/versioning.md)
for the frozen draft contracts.
