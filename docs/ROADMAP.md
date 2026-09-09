# Roadmap

Source readiness and published artifacts are separate. CLI 0.1.0-alpha.2 and
GitHub provider 0.1.0 remain the published releases. All four official providers
have unpublished 0.2.0 source candidates; offline qualification is recorded in
[provider PR 13](https://github.com/NIPE-Solutions/permesh-providers/pull/13).
Nothing here declares the SDK or protocol stable.

## Implemented foundation

- Local read-only `user`, `admins`, `orphaned`, `doctor`, versioned JSON and an
  offline demo. No backend, telemetry or persistent access database.
- Independent domain, wire and output models; principal kind/affiliation/lifecycle,
  resource containment, native roles, access evidence and conservative certainty.
- Native external execution with digest verification, instance-scoped credentials,
  workspace approval, bounded communication, cancellation and partial results.
- Explicit provider installation/updates and guided add/setup, declarative setup
  forms, keychain/environment references and provider-described browser login.
- Negotiated v1 discovery/health in the host and official emitters. Verified
  package compatibility propagates to guided setup; advanced setup/migration
  has an explicit selector. Existing legacy metadata remains unchanged.
- Official GitHub, Google, Cloudflare and AWS adapters in the separate provider
  repository. Only demo remains bundled. Legacy Google/GitHub configurations
  remain readable for explicit migration, with execution blocked before secrets.
- Cross-platform CI, dependency audits, native archives and notices, checksums,
  target-bound dependency inventories and CLI provenance qualification.

## Next hardening slices

1. **P1 — Enterprise networking:** explicit proxy/custom CA context, provider
   feature negotiation and approval binding are implemented in the host. Qualify
   official adapter support; custom endpoints, AWS transport and host browser
   networking remain separate follow-ups. Child environments stay sanitized.
2. **P1 — Mixed-platform teams:** explicit target-to-digest maps and guided
   `--portable` setup are implemented, preserving scalar pins. Each machine
   selects its own reviewed digest and establishes local trust and approval.
   Cross-platform qualification remains a merge gate.
3. **P2 — Diagnostics and integrity:** clearer doctor/status stages and stable
   provider diagnostic codes; further executable-substitution tests and reduced
   verification/execution gaps. Document OS-level residual risk.
4. **P0 — Final contract review:** review protocol/SDK, config, output schemas,
   setup/auth operation negotiation and provider conformance together. Complete
   the threat-model pass before declaring stability. The number of supported
   operations must not drive protocol version increments.

See the [architecture backlog](audits/architecture-hardening-backlog.md) for
acceptance criteria and historical implementation evidence.

## Release and adoption gates

- Qualify the exact candidate CLI with candidate provider installation, trust,
  setup, approval, credentials and queries. Publish compatible artifacts and
  catalog entries only after this gate; never replace existing release bytes.
- Complete broader live GitHub acceptance and live Google directory/browser,
  Cloudflare and AWS checks with least-privilege credentials. Mock tests prove
  behavior against fixtures, not real tenant permissions or API visibility.
- Complete remaining Windows terminal and interactive desktop credential-store
  acceptance. CI unit tests do not substitute for platform credential UX checks.
- Apply exact-revision native build, dependency and provenance verification to the
  new release. Native code signing/notarization remains dependent on available
  Apple and Windows signing identities; GitHub provenance does not replace it.
- Google source removal is a deliberate pre-adoption breaking change. Until its
  external package is published, Google users need a reviewed native build or
  must remain on the existing published CLI. The migration guide states this gap.

## Later, separate work

- AWS Identity Center/Organizations and native short-lived authentication,
  including reviewed profile/SSO behavior; current IAM attachments are evidence,
  not effective authorization.
- More identity authorities such as Entra, then additional providers driven by
  concrete use cases and qualification, not provider-count targets.
- CLI self-update and package managers; provider updates already exist.
- Developer provider mode, conformance tooling, deterministic config composition,
  snapshots/diffs and local policies. No mutation features in this milestone.
- A standards SBOM generator; the current dependency inventory is not an
  SPDX/CycloneDX SBOM.

No collaboration server, remote credential vault, analytics or remediation is
needed to finish the discovery foundation.
