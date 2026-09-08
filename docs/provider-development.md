# Provider development

The current provider boundary is the Rust SDK and validated core snapshot. Read [PROVIDER_MODEL.md](PROVIDER_MODEL.md), [DOMAIN_MODEL.md](DOMAIN_MODEL.md), and the existing demo/GitHub adapters. A provider must preserve scoped native IDs, distinguish observation from effective authorization, report incomplete visibility, and return structured failures. Never turn permission denial into an apparently successful empty result. Use synthetic fixtures and local mock APIs for contract tests.

## External protocol validation

The pure Rust `permesh-provider-protocol` crate validates bounded draft-1
handshake/discovery transcripts into core snapshots. Read the
[wire specification](provider-protocol.md) and run the
[Python example and offline validator](../examples/external-provider/README.md).
All six normalized record kinds are exercised across the language boundary.

## Explicit native execution

The separate `permesh-provider-external` crate implements user-local trust
registration and supervised discovery. See [external providers](external-providers.md)
for commands and trust boundaries. Workspace YAML cannot select an executable;
ordinary access queries never launch registered programs.

Implement a self-contained native binary speaking draft 1. The registered ID
must equal the handshake provider type, and its declared capabilities must match
the reviewed registration exactly. Build for each target platform. The Python
example remains an offline interoperability reference: scripts and interpreter
launch commands are not accepted by the native host.

The host supplies no provider configuration or credentials. Credential transport,
interpreted-provider trust, workspace integration and authoritative identity-source
selection require separate designs. Parsed identity assertions are not automatically
promoted to authority or correlated with the ordinary access graph.

Use synthetic fixtures for hostile-process tests: malformed responses, floods,
hangs, cancellation, descendant cleanup, unexpected capabilities and executable
replacement. The protocol parser alone cannot satisfy process-security requirements.
