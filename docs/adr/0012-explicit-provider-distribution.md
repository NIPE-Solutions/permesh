# ADR 0012: Explicit provider distribution and retained versions

Status: Accepted

## Decision

Host the official catalog and provider releases in `NIPE-Solutions/permesh-providers`.
Keep the CLI, SDK and protocol in the main repository. Implement explicit install,
update and metadata-only check commands. No ordinary query checks for updates.
This deliberately extends runtime networking to user-requested GitHub distribution
requests; it introduces no project-operated backend or access-data transmission.

Separate verified downloaded packages from trusted executable registrations and
workspace approval. Store package coordinates immutably and preserve previously
trusted digest versions. Workspace discovery resolves the configured digest,
not the most recently trusted registration. Package installation never executes,
trusts code, or changes workspace configuration.

Use one bounded ZIP layout for all five supported platforms, avoiding two archive
parsers. A strict allowlist extracts only the provider and license; reject paths,
links and additional entries. Disable default ZIP features except the selected
DEFLATE implementation. Reuse the existing HTTP client and SHA-256 dependency.
Use maintained semver for exact version comparison rather than hand-written
parsing or lexicographic version ordering.

## Consequences

Downloads are auditable and ordinary queries keep predictable network behavior.
Keeping old versions permits reviewed rollback and preserves workspace approvals.
Disk usage grows until users explicitly manage retention; automatic cleanup and
bulk updates are outside this slice. GitHub availability and repository control
are distribution trust boundaries. Checksums do not independently authenticate a
compromised catalog; attestation verification remains explicitly unsupported.
The first catalog stays empty until a real provider has qualified native releases.
