# ADR 0015: GitHub executes only as an approved external provider

Status: Accepted

## Decision

Remove the GitHub HTTP adapter crate and its workspace/CLI dependencies. Keep the
legacy `ProviderKind::Github` shape and config validation for explicit migration.
Reject legacy health, queries, metadata and credential commands with actionable
migration instructions before parsing or resolving credentials. Mixed-provider
queries retain available observations and report the legacy provider failure.

`provider add github` does not create a legacy configuration. Its initial
install/trust/setup guidance is superseded by the guided orchestration in
[ADR 0022](0022-guided-official-provider-trust.md). New GitHub instances use independently
packaged native code, explicit binary trust, declarative setup and workspace
approval. Google and the offline demo remain bundled.

## Consequences

No GitHub access-discovery HTTP adapter remains linked into the CLI. The separate provider repository
owns its adapter semantics, HTTP tests and documentation. Existing IDs, aliases and
token references migrate through the already implemented explicit command.
Historical bundled live checks do not establish external release qualification.
Removal is gated on the separately qualified release and catalog, including all
five native packages and a working install, trust and migration route.
