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

## Hardening source status

- **Enterprise networking:** approved proxy/custom CA context and `network_v1`
  negotiation are implemented. GitHub, Google and Cloudflare adapters apply it
  to their HTTP clients, including Google refresh. AWS transport, host browser
  networking, custom endpoints and authenticated proxies remain unsupported.
- **Mixed-platform teams:** explicit target-to-digest maps and guided `--portable`
  exact-version setup are implemented. Each machine selects its reviewed native
  digest and establishes local trust and approval; scalar pins remain compatible.
- **Diagnostics and integrity:** optional `doctor --details` reports curated
  stages from the existing health pass. All CLI launch paths recheck the trusted
  executable pin before spawn and observe pre-cancellation. This narrows the
  verification gap; it does not eliminate hostile same-user path races.
- **Contract review:** independent domain/wire/output boundaries, evidence
  semantics, feature negotiation and trust behavior have been reviewed together.
  Protocol v1 remains draft. Setup/auth retain frozen legacy descriptions; future
  compatible operations must not increment the protocol version.

See the [contract review](audits/hardening-contract-review.md) and
[architecture backlog](audits/architecture-hardening-backlog.md) for scope,
limitations and validation requirements. Source completion is distinct from
artifact qualification, publication and live tenant acceptance.

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
