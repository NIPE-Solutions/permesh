# Roadmap

Permesh's first stable release remains smaller than the long-term product vision.
The CLI is currently published as an unsigned alpha; GitHub is the only published
official provider package. Implemented source and qualified releases are tracked
separately.

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
  generates and verifies GitHub provenance attestations for reviewed candidates;
  only a successful run qualifies those exact artifacts.

## Before stable release and provider publication

1. Complete the broader credentialed GitHub fixture exercise, live Google
   directory/browser login, and live Cloudflare/AWS acceptance. Google and
   Cloudflare draft packages are not cataloged; AWS also needs native artifact
   qualification. Synthetic tests do not substitute for these checks.
2. Complete remaining Windows terminal and interactive desktop credential-store
   acceptance, then qualify the exact release revision on all supported targets.
3. Verify and promote the original qualified artifacts, update release evidence
   and documentation, and keep versioned assets immutable. See [release
   gates](releasing.md).
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
