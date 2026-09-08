# Roadmap

Permesh's first stable release remains smaller than the long-term product vision.
CLI [0.1.0-alpha.2](releases/0.1.0-alpha.2.md) is published with verified GitHub
provenance for its five native target artifact sets. Platform code signatures and
notarization remain absent. GitHub 0.1.0 is the only published official provider
package; Google 0.1.1, Cloudflare 0.1.0 and AWS 0.1.0 are unpublished drafts.
Implemented source and qualified releases are tracked separately.

## Implemented source

- Local-first read-only queries: offline demo, `doctor`, `user`, `admins`,
  `orphaned`, versioned JSON, shell completions and explicit identity authority.
- Native external providers: digest-bound trust, separate exact workspace
  approval, named env/keychain credentials, bounded execution and cancellation,
  declarative setup, explicit package install/update and legacy migration.
- GitHub observed-access discovery, Google directory identities, Google
  refresh-token authentication and provider-declared browser login. Google
  migration preserves explicit authority; the bundled adapter remains available.
- In the official provider repository, Cloudflare account-access observations and
  an AWS IAM policy-attachment inventory with named credentials. Neither claims
  complete effective authorization. See [provider scope and status](providers.md).
- Native candidate packaging, dependency notices, checksums, target-bound
  dependency inventories and Unix terminal checks. A separate manual workflow
  generates and verifies GitHub provenance attestations for reviewed candidates.
  Alpha.2 promotes the exact subjects from [run 34273757910](https://github.com/NIPE-Solutions/permesh/actions/runs/34273757910),
  with its verified bundle; later revisions need their own qualification.

## Contract hardening

The [architecture audit and backlog](audits/architecture-hardening-backlog.md)
track the pre-stability work. Frozen legacy wire DTOs, independent output DTOs,
scoped approvals and guided GitHub setup are implemented. Core now separates
principal dimensions, validates resource containment and distinguishes access
evidence; access reports use JSON schema 2.

Next are negotiated operations/records that carry these dimensions to native
providers, explicit enterprise networking and target-aware provider pins. A
whole-contract review remains required before SDK or protocol stability. The
source changes do not qualify or replace published artifacts.

## Before stable release and provider publication

1. Complete the broader credentialed GitHub fixture exercise, live Google
   directory/browser login, and live Cloudflare/AWS acceptance. Google and
   Cloudflare draft packages and AWS 0.1.0 are not cataloged. Qualify their exact
   candidate artifacts separately before publication. Synthetic tests do not
   substitute for these checks.
2. Complete remaining Windows terminal and interactive desktop credential-store
   acceptance, then qualify the exact release revision on all supported targets.
3. Apply the same exact-revision qualification and public-verification gates to
   subsequent CLI and provider releases; keep the published alpha.1 and alpha.2
   assets immutable. See [release gates](releasing.md).
4. Complete native signing/notarization once Apple and Windows signing identities
   are available. GitHub provenance does not replace either platform's signing
   requirements. Package-manager manifests must use the final distributed hashes.
5. Publish qualified external Google packages before removing the bundled adapter;
   keep the offline demo available without downloads.

## Later options

- Package-manager distribution and CLI self-update. Provider package updates
  already exist; they do not update the CLI.
- Native AWS profiles, SSO and credential refresh. Current AWS credentials are
  explicit references; temporary sessions are refreshed outside Permesh.
- Dynamic API-driven onboarding and broader provider coverage.
- A standards SBOM generator with a clean reviewed dependency graph. The existing
  inventory is not a CycloneDX/SPDX SBOM; see [tool evaluation](releasing.md#standards-generator-evaluation-2026-09-08).
- Deterministic config composition/local overrides, schema migrations,
  policies/audit and snapshot diff. JSON output already supports report export;
  additional export formats are separate future work.

No collaboration backend, telemetry, remote configuration or mutation APIs are
planned for this milestone.
