# Security policy

This repository is pre-release; no stable release currently receives security support. Do not assume the CLI establishes complete effective authorization or use its output as the sole basis for an access-control decision.

For a suspected vulnerability, use the repository host's private vulnerability reporting feature if enabled. If it is unavailable, contact a listed maintainer through an existing private contact method. A dedicated security address and response SLA have not been established. Do not put tokens, private organization data, or exploit details in public issues. A public issue may request a private reporting channel without disclosing the vulnerability.

Include the affected revision, operating system, minimal synthetic reproduction, expected security boundary, and observed behavior. Remove credentials and personal data. If a credential was exposed, revoke it with its issuer; deleting a log or issue does not revoke a token.

Maintainers should acknowledge privately, reproduce with synthetic fixtures, agree disclosure timing with the reporter, patch and add regression coverage, and publish an advisory when distribution warrants it. These are intended practices, not a guaranteed response time.

See the [security model](docs/SECURITY_MODEL.md), [threat model](docs/threat-model.md), and [privacy statement](docs/privacy.md). External native execution requires explicit local trust; workspace execution also
requires exact local approval. It is not sandboxed; see the
[external provider boundary](docs/external-providers.md).
