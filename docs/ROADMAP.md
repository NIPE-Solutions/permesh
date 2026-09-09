# Roadmap

Source readiness and published artifacts are separate. CLI 0.1.0-alpha.2 and
GitHub provider 0.1.0 remain the published releases. The provider repository now
contains seven unpublished 0.2.0 candidate binaries: GitHub, Google Workspace,
Cloudflare, AWS IAM, GitLab, Entra and AWS Identity Center. The original four
completed earlier offline qualification in [provider PR 13](https://github.com/NIPE-Solutions/permesh-providers/pull/13);
new candidates require their own exact-revision artifact gates. This document
includes the access-review source/PR program, not only merged or released code.
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
- Official provider sources in the separate provider repository, including the
  three additional candidates listed above. Demo and pinned identity inventories
  are local built-ins. Legacy Google/GitHub configurations
  remain readable for explicit migration, with execution blocked before secrets.
- Cross-platform CI, dependency audits, native archives and notices, checksums,
  target-bound dependency inventories and CLI provenance qualification.

## Hardening source status

- **Enterprise networking:** approved proxy/custom CA context and `network_v1`
  negotiation are implemented. GitHub, Google, Cloudflare, GitLab, Entra and Identity Center candidates apply
  it to their HTTP transport, including Google refresh. GitLab supports an
  explicitly approved HTTPS origin. AWS IAM, host browser networking, arbitrary
  endpoint overrides and authenticated proxies remain unsupported.
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

## Access-review candidate status

The unpublished source program implements the following bounded workflows. Its
PRs are integrated and checked separately; publication is a later decision.

- `identity` lists evidence and proposes stable-account mappings. Exact proposal
  approval binds the alias delta, source context and captured file revision.
  Pinned JSON inventories are explicit authority inputs, not live directory APIs.
- `snapshot create`, `snapshot inspect` and `diff` provide private,
  versioned local artifacts and offline comparisons. Changed scope or failed
  collection never proves access removal.
- `offboard assess`, `offboard plan` and `offboard verify` produce read-only
  JSON/HTML evidence and advisory work. No provider mutation occurs; stale,
  unmapped or incomplete evidence remains unresolved.
- `resource` exposes source grants and paths; `policy check` evaluates bounded
  local rules, scoped exceptions and declared ownership assertions. Unknown
  evidence is not clean, and findings use exit 6.
- `provider dev scaffold` generates a native workspace starter; `provider dev validate`
  checks offline transcripts. Neither executes or qualifies arbitrary binaries.
- Approved host-owned exact reads support 1Password Connect and separately
  dispatched Vault/OpenBao KV v2. Explicit temporary AWS profiles read one
  selected shared-credentials file/profile and bind account, region and caller
  role; there is no ambient chain, interactive SSO login or automatic refresh.

See the [implementation and qualification gap map](audits/architecture-hardening-backlog.md#access-review-implementation-program)
for validation evidence and remaining prerequisites.

## Release and adoption gates

- Qualify the exact candidate CLI with candidate provider installation, trust,
  setup, approval, credentials and queries. Publish compatible artifacts and
  catalog entries only after this gate; never replace existing release bytes.
- A local macOS ARM64 GitHub candidate passed authorized read-only health, status
  and stable-account JSON queries with explicit trust/approval. Visibility limits
  remained explicit. Exact tested hashes are retained; shared local build output
  does not prove reproducible full-source provenance or release qualification.
  Complete broader GitHub acceptance and live Google directory/browser,
  Cloudflare, AWS IAM, GitLab, Entra and Identity Center checks with explicitly
  authorized least-privilege credentials. Mock tests prove
  behavior against fixtures, not real tenant permissions or API visibility.
- Complete remaining Windows terminal and interactive desktop credential-store
  acceptance. CI unit tests do not substitute for platform credential UX checks.
- Apply exact-revision native build, dependency and provenance verification to the
  new release. Native code signing/notarization remains dependent on available
  Apple and Windows signing identities; GitHub provenance does not replace it.
- Google source removal is a deliberate pre-adoption breaking change. Until its
  external package is published, Google users need a reviewed native build or
  must remain on the existing published CLI. The migration guide states this gap.

## Future integrations: separate from the active program

The following Phase 6 priorities are provisional. Reorder them using consenting
pilot requests, available qualification tenants and maintenance capacity. Each
first slice needs explicit scope, permissions, stable IDs, pagination/failure
fixtures and live-acceptance criteria before being advertised as supported.
Entra, GitLab, AWS authentication/Identity Center, 1Password and OpenBao/Vault KV
belong to the active program, not this future list.

| Priority | Integration and first slice | Value and boundary |
| --- | --- | --- |
| 1 | Authentik / Keycloak: separate identity, group and role adapters | Self-hosted identity sources; IdP state is not complete downstream access. |
| 1 | LDAP / Active Directory: explicit directory/schema adapter | Existing directory inventory; LDAP transport does not define lifecycle semantics. |
| 1 | Okta: identity and application assignments | Another authority; assignment does not establish every downstream permission. |
| 1 | HR rosters: one explicitly scoped employment source | Lifecycle evidence; keep employment, account state and service identities separate. |
| 2 | Slack: membership, roles and supported invitations | Collaboration review; qualify each API/plan and make no universal SCIM promise. |
| 2 | Atlassian: one organization/product/project scope | Distinguish membership, product access, project permissions and ownership. |
| 2 | Azure RBAC: scoped resource role assignments | Resource authorization evidence, separate from Entra directory roles. |
| 2 | GCP IAM: scoped resource policies and bindings | Preserve hierarchy, conditions, denies and visibility limits. |
| 2 | Kubernetes: namespace/cluster roles and bindings | RBAC inventory does not cover all authentication, admission or external authorization. |
| 3 | Tailscale: supported user/device and access metadata | Network review; listings alone do not prove effective connectivity. |
| 3 | Proxmox: users, groups, roles and ACLs | Preserve native realms and inheritance rather than flattening permissions. |
| 3 | PostgreSQL: roles, membership, privileges and ownership | Separate membership from effective privileges, RLS and session behavior. |
| 3 | Generic SCIM: a bounded user/group adapter | Interoperability; SCIM is not universal permission discovery. |
| 3 | Baton import: investigate one versioned output fixture | Reuse only after mapping and testing provenance, scope and semantics; compatibility is unproven. |

Future secret resolution has its own ordered backlog:

1. SOPS with age or an explicitly selected KMS for encrypted Git-managed inputs.
2. AWS Secrets Manager, Azure Key Vault and Google Secret Manager, prioritized by
   actual pilot environments and retrieving only the configured field.
3. Bitwarden Secrets Manager, Infisical and Doppler according to demand. Bitwarden
   Password Manager and Vaultwarden are distinct, separately qualified targets.
4. An explicitly trusted external credential helper only when a concrete need
   cannot use a safer native resolver contract; no arbitrary shell execution.

A secret-store access-audit provider is separate from credential resolution. It
should collect permission metadata without reading secret values where possible;
potential access, logged access and proof of disclosure are different claims.

## Future remediation: design only

[ADR 0028](adr/0028-separate-opt-in-remediation.md) defines separate opt-in write
capabilities, exact reviewed plans, protected targets, fresh preflight and
independent verification. No remote mutation implementation or hidden write mode
in inspection commands is part of this program.

## Other deferred delivery work

CLI self-update/package managers and a standards SBOM generator remain separate
qualification projects. The current dependency inventory is not an
SPDX/CycloneDX SBOM. A collaboration server, remote credential vault or analytics
backend is not required for local read-only review.
