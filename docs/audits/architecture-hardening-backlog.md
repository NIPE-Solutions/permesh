# Hardening backlog

Status as of 2026-09-09. The [audit](architecture-hardening.md) records the
baseline and breaking-change ledger. Each slice requires implementation,
negative tests, documentation and migration notes before it is complete.

| Slice | Priority / dependencies | Tasks | Acceptance criteria | Status |
| --- | --- | --- | --- | --- |
| 1. Baseline | P0 | Audit both repositories; identify compatibility surfaces and risks. | Evidence-backed audit and ordered breaking-change ledger exist before refactoring. | Complete |
| 2. Boundary freeze | P0 / 1 | Historical wire transcripts; wire-owned records/capabilities and private validated mapping; CLI-owned query/snapshot/control DTOs; official emitter projection; versioning ADR. | Old transcripts and full JSON fixtures survive extraction; malformed input still fails closed; no implicit schema change. | Complete; CLI PR 25 and provider PR 11 qualified |
| 3. Concrete defects | P1–P2 / 1 | Bounded revision-aware config writes; correct unsupported operation errors. | Regressions reproduce original failures and pass; rejected writes preserve original file; official responses decode correctly. | Complete; PRs 23 and provider 10 qualified |
| 4. Domain semantics | P0 / 2 | Independent kind/affiliation/lifecycle; resource type/parent; evidence kind/certainty; normalization guidance. | External inactive humans and inactive service principals are representable; parent cycles rejected; assignments never presented as proved effective access; stable IDs retained. | Complete; CLI PR 26 qualified; prerelease contract |
| 5. Negotiated protocol | P0 / 2,4 | Explicit version/operation capability contract, diagnostics, independent DTO validation, conformance fixtures. | Old drafts remain accepted unchanged; unknown version/operation/capability behavior documented and tested; provider SDK does not require internal serialization knowledge. | Negotiated protocol v1 host and official emitters qualified (CLI PR 27, provider PR 13); remaining operation/SDK conformance review open |
| 6. Scoped approval | P1 / 1 | Provider context fingerprint and migration; remaining substitution/pre-cancellation tests. | Unrelated provider edits preserve approval; config, credential, digest or registration changes revoke it; no credential delivery before approval. | Fingerprint and final pinned launch checks implemented; same-user OS path race remains documented |
| 7. Official onboarding | P1 / 6 | Add/install/verify/trust/setup/approve orchestration; concise consent; resumable prompts and CI flags. | Add/auth/query requires no manual hashes; rejection executes nothing; updates never silently adopt pins; expert workflow preserved. | Guided GitHub baseline qualified in CLI PR 25; package-contract propagation and all-official routing implemented in current source |
| 8. Enterprise network | P1 / 5,6 | Explicit proxy/CA/endpoint context; validation and provider support negotiation. | Tests prove no ambient inheritance; changed network context invalidates approval; secrets never enter Git or diagnostics. | Proxy/CA context and negotiation implemented in the host and six candidate HTTP transports, including Identity Center. GitLab supports an approved HTTPS origin. AWS IAM and host browser networking remain unsupported. |
| 9. Provider/query quality | P2 / 4,5 | Contract suite, GitHub duplicate conflicts, Google suspended evidence, AWS attachment cross-decode, traversal pruning; structured doctor/status. | Mock HTTP and native contracts pass; partial semantics remain honest; no-grant populations avoid irrelevant path limits. | Traversal and GitHub conflict handling qualified; Google lifecycle and AWS runtime cross-decode qualified in provider PR 13; optional typed doctor diagnostics implemented |
| 10. Stability review | P0 / 2–9 | Threat-model pass; schema/migration docs; team/contributor/README update; all-platform checks. | Format, clippy, tests, dependency checks and provider conformance pass; no stable claim while P0 items remain. | Source contract/threat-model review complete; exact-revision CI and release/adoption gates remain mandatory; protocol remains draft |
| Follow-up program | P1–P3 | Identity Center, Entra, contributor tooling and snapshots/diffs/policies. | Bounded source implementation and separate qualification gates. | Implemented in the access-review program below; interactive SSO and remote mutation remain outside scope |

Target-aware release pinning is P1 (H16): current source supports an explicit
`sha256_by_target` alternative and guided `--portable` exact-version catalog
selection. Scalar serialization remains unchanged. No lockfile is added;
workspace maps bind executable identities, while trust and approval stay local.
See ADR 0026 and team workflows for migration and cross-platform limitations.

## Validation and release gates

Use synthetic fixtures and local mock HTTP by default. Run workspace format,
clippy with warnings denied, full tests and dependency checks. Providers require
their declared Rust toolchain. CI must cover Linux, macOS and Windows; local
macOS tests alone are not cross-platform qualification. Keep live tenant and
OS signing/notarization gates distinct from synthetic compatibility testing.

No release is part of a refactor by implication. Review user-facing schema and
SDK migration notes before a new prerelease; never overwrite existing assets.

## First implementation batch

Host wire records/capabilities and CLI query/snapshot DTOs now own their legacy
formats. Historical fixtures cover drafts 1–4 and complete query output. Public
unchecked wire-to-domain conversions are unavailable; only a completed validated
decoder exposes a snapshot. Follow-up implementation also separates control-command
JSON and official runtime emission before domain migration.

Provider add/setup now share bounded captured-revision writes. Oversized serialized
configuration and detected concurrent edits leave the original file intact; the
final read/rename gap is not an atomic filesystem compare-and-swap. The shared
native runtime's unsupported-operation error now cross-decodes correctly.

Provider-scoped approvals include a migration test that stores a genuine legacy
approval and verifies rejection before unavailable credentials are resolved.
Selected alias changes invalidate approval; changes to another binding in the
same alias row do not. See ADR 0017 for the complete fingerprint contract.

Local macOS validation: 288 CLI-workspace tests passed, one native keychain test
remains opt-in; 107 provider-workspace tests passed. Formatting, Clippy with
warnings denied, dependency audit and Python/native query interoperability passed.
PR 23 and provider PR 10 subsequently passed Linux/macOS/Windows CI, dependency
checks and all five native candidate targets before merge. No live tenant
qualification or new release is implied.

## Follow-up implementation batch

Guided GitHub add composes current public-catalog installation, explicit native
trust, declarative setup and instance approval. Both consents remain explicit;
authentication is separate. Strict automation uses references in an answer file.
Captured revisions, exact registration pins and cancellation protect publication.
Control-command fixtures preserve existing metadata, registration, approval and
package JSON. The full local CLI workspace passed 309 tests (one native keychain
test remains opt-in). CLI PR 25 and provider PR 11 passed three-platform CI,
dependency checks and all five native candidate targets before merge. The provider
workspace passed 112 Rust tests plus 14 packaging tests; no release was published.

All queries now prune grant-irrelevant paths without dropping selected accounts.
Relevant path limits remain, plus a 64 MiB cumulative path-copy estimate prevents
large labels multiplying across paths. This is not a process RSS cap: snapshots,
indexes, relevance lookups, allocator overhead and output conversion are outside
that estimate. Oversized expansion fails explicitly rather than truncating.

The domain migration now implements independent principal dimensions, resource
containment and access-evidence semantics with access JSON schema 2. Legacy
wire remains unchanged. PR 26 passed cross-platform CI, dependency checks and five
native candidate targets before merge. Opt-in negotiated protocol 1 now carries the new dimensions
for configured discovery and health; official emitters qualified in provider PR 13; final contract stability review remains P0.
See [migration details](../migrations/domain-schema-2.md). Each new slice requires complete verification before merge; none of these
contracts is declared stable.

## Provider adoption and duplicate removal

Current source carries explicit negotiated discovery metadata into guided setup
and keeps advanced setup/migration selection explicit. Bundled Google has been
removed after offline external qualification; legacy configs fail before secrets
and retain a migration path. Google publication/live qualification remains open,
so source builds require a reviewed native executable until catalog publication.
See ADR 0024 and the [prioritized roadmap](../ROADMAP.md).

## Launch and diagnostic review

All CLI launch paths bind the verified registration digest to a final protected
path/native-byte check immediately before spawn. Already-observed cancellation
prevents startup. Deterministic substitution, truncation, symlink and permission
changes are covered; no atomic cross-platform execution-handle guarantee is made.
Detailed doctor output carries finite local stage codes without changing default
control output or probing credentials before approval. See ADR 0027 and the
[contract review](hardening-contract-review.md).


## Access-review implementation program

Baseline: CLI `c7784a4`, providers `5353ce8` (2026-09-09). The existing
read-only query engine, independent contracts, native trust, scoped approval,
network context and portable pins remain the foundation.

| Phase | Gap and deliverable | Dependencies | Acceptance / status |
| --- | --- | --- | --- |
| 0 | Case-sensitive local documentation checks and accurate release/source guidance | Existing docs and CI | Implemented; offline checker and regression suite, PR 33/provider PR 16 merged after three-platform tests, dependency checks and five native targets |
| 1 | Cross-provider reference workflow, qualification matrix and compatible candidates | Existing host/adapters | Implemented reference fixture and qualification matrix; current candidate artifacts remain unpublished; directory live checks require a tenant |
| 2A | Explicit identity mapping tools and versioned file-backed identity source | Identity index, strict config, captured writes | Implemented; identity/inventory PR merged after passing cross-platform CI |
| 2B | Explicit private snapshots, offline inspection and scope-aware diffs | Validated graph, independent artifact DTOs | Implemented; 15 comparison/privacy/adversarial CLI tests and broader local suite passed |
| 3 | Read-only departure assessment, advisory plan, verification and HTML/JSON reports | Identity tools and snapshots | Implemented; 10 adversarial/fresh/replay CLI tests, including lost-correlation verification; source expiry, ownership and partial checks explicit |
| 4A | Resource queries and focused local review rules | Existing graph and review artifacts | Implemented; core and CLI tests passed, scoped exceptions/owners, findings exit 6 |
| 4B | Minimal native scaffold and conformance workflow | SDK/runtime and existing fixtures | Implemented; offline validator tests and generated native scaffold tests passed, no automatic execution |
| 5A | Entra, GitLab and explicit short-lived AWS/Identity Center integrations | Qualified existing-provider workflow | Implemented candidate sources and local HTTP/native tests; explicit temporary-profile integration implemented; candidate artifact gates tracked separately, live prerequisites recorded separately |
| 5B | Host-owned 1Password and OpenBao/Vault KV resolvers | Secret references, approved network/auth context | Implemented and locally tested exact-field retrieval, bounded cancellation, no recursive fallback or bootstrap reflection; remote-source PR merged after passing cross-platform CI |
| Future | Pilot-driven integrations and separate remediation design | Evidence from adoption | Backlog and ADR only; no mutation implementation |

The active program is not complete until phases 0–5 are implemented or their
remaining checks are explicitly recorded with actual prerequisites. Signing
identities and live tenants cannot be replaced by fixture-based claims. Product
contracts and release notes will be updated with each implemented slice.

## Access-review delivery evidence and remaining gates

The combined local source program at `37ddc5c` passed 503 tests, strict Clippy,
formatting, dependency checks and 101 tracked-document checks on macOS. Later
focused temporary-profile checks passed at `1a4b3e4`. A 14-command synthetic demo
at that later source revision
exercised review queries, snapshots/diffs, advisory plans and verification,
HTML output, private artifact modes and policy findings exit 6. These are local
source checks, not claims about every PR head, platform or published artifact.
The split PRs retain their own CI gates. Identity tools and remote credential
sources have reached main; remaining snapshot/offboard, policy, tooling and AWS
profile slices must retain combined regression coverage as they are integrated.

| Remaining gate | Concrete prerequisite and acceptance evidence |
| --- | --- |
| Candidate packages | Build and verify exact CLI/provider revisions for every supported target, dependency inventories, notices, checksums and provenance. The seven source binaries do not imply seven published packages or catalog entries. |
| GitHub breadth | An authorized tenant and least-privilege credential for API-path/visibility comparisons beyond the limited local health/status/account acceptance. No other provider inherits this result. |
| Google / Cloudflare / AWS IAM | Authorized tenants/accounts, explicit scopes and credentials; compare visible source records and document omitted permissions. Google browser flow needs desktop acceptance too. |
| GitLab / Entra | Authorized scoped GitLab groups/projects and read_api PAT; an Entra tenant and permitted Graph application credential. Verify pagination, tenant binding, membership visibility and unsupported-role boundaries against real APIs. |
| Identity Center / temporary AWS profile | An authorized STS account/role, Identity Center instance/store pair, account allowlist and temporary session. Verify provisioned permission-set assignments, explicit Organizations scope if enabled, and session expiry/role mismatch. No interactive SSO flow is implemented. |
| Remote credential stores | Explicitly authorized Connect and separately qualified Vault/OpenBao KV v2 services, HTTPS trust, bootstrap credentials and exact item/path/field permissions. Fixture qualification does not establish product/version compatibility or operational availability. |
| Desktop and Windows | Native interactive keychain/terminal tests on supported operating systems, plus five-target candidate CI. Local macOS tests do not substitute for these checks. |
| Signing and release | Available Apple/Windows signing identities where required; final reviewed release notes, compatible catalogs and immutable publication. No signing, publication or adoption is implied by source completion. |
| Voluntary pilots | Opt-in operator feedback with sanitized examples; no automatic telemetry, enrollment, outreach or fabricated tenant evidence. |

Future integrations remain prioritized only in the [roadmap](../ROADMAP.md).
Remote remediation remains the separate design-only ADR 0028. Core and official
connectors remain MIT with no paid feature gates.
