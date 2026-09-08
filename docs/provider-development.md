# Provider development

The current provider boundary is the Rust SDK and validated core snapshot. Read [PROVIDER_MODEL.md](PROVIDER_MODEL.md), [DOMAIN_MODEL.md](DOMAIN_MODEL.md), and the existing demo/GitHub adapters. A provider must preserve scoped native IDs, distinguish observation from effective authorization, report incomplete visibility, and return structured failures. Never turn permission denial into an apparently successful empty result. Use synthetic fixtures and local mock APIs for contract tests.

## External protocol validation

The pure Rust `permesh-provider-protocol` crate validates bounded draft-1
handshake/discovery transcripts into core snapshots. Read the
[wire specification](provider-protocol.md) and run the
[Python example and offline validator](../examples/external-provider/README.md).
All six normalized record kinds are exercised across the language boundary.

No external runtime is enabled. The validator does not launch, supervise or
trust a program; merely placing one in a repository cannot trigger execution.
The draft has no stable deployed compatibility promise. Record capabilities,
strict framing and domain validation are implemented; credential exchange,
trusted registration and process supervision remain separate prerequisites.

## Future local trust registration

Execution requires an explicit user-level registration outside workspace YAML. Record an absolute executable path, a cryptographic digest and provider identity; show the path, digest, requested capabilities and same-user execution risk for acknowledgment. Bind trust to the actual executable and revalidate at launch; a changed digest requires renewed acknowledgment. For interpreted providers, trust must cover both interpreter and script plus meaningful dependency changes, not merely the Python executable. Reject repository-selected executable paths, PATH lookup, shell command strings and symlink substitutions. Avoid a check-then-execute race or document the platform-specific residual risk before enabling the feature.

Trust is permission to run reviewed local code, not a sandbox. A child can access files and the network with the user's authority unless separately restricted. Minimize inherited environment variables, handles and working-directory exposure. Never auto-install, auto-update or automatically run discovered provider programs. Workspace configuration can refer to a registered identity only after this trust mechanism exists.

Host implementation acceptance requires adversarial process tests for floods, hangs, forked children, malformed or oversized frames, credential-bearing stderr, unexpected capabilities, identity mismatch, partial discovery, cancellation and executable replacement. The current example and Rust validator cover framing, normalized records and exchange validation only; it must not be cited as passing those host requirements.
