# Hardening backlog

Status as of 2026-09-09. The [audit](architecture-hardening.md) records the
baseline and breaking-change ledger. Each slice requires implementation,
negative tests, documentation and migration notes before it is complete.

| Slice | Priority / dependencies | Tasks | Acceptance criteria | Status |
| --- | --- | --- | --- | --- |
| 1. Baseline | P0 | Audit both repositories; identify compatibility surfaces and risks. | Evidence-backed audit and ordered breaking-change ledger exist before refactoring. | Complete |
| 2. Boundary freeze | P0 / 1 | Historical wire transcripts; wire-owned records/capabilities and private validated mapping; CLI-owned query/snapshot/control DTOs; official emitter projection; versioning ADR. | Old transcripts and full JSON fixtures survive extraction; malformed input still fails closed; no implicit schema change. | Complete; CLI PR 25 and provider PR 11 qualified |
| 3. Concrete defects | P1–P2 / 1 | Bounded revision-aware config writes; correct unsupported operation errors. | Regressions reproduce original failures and pass; rejected writes preserve original file; official responses decode correctly. | Complete; PRs 23 and provider 10 qualified |
| 4. Domain semantics | P0 / 2 | Independent kind/affiliation/lifecycle; resource type/parent; evidence kind/certainty; normalization guidance. | External inactive humans and inactive service principals are representable; parent cycles rejected; assignments never presented as proved effective access; stable IDs retained. | Implemented; prerelease contract |
| 5. Negotiated protocol | P0 / 2,4 | Explicit version/operation capability contract, diagnostics, independent DTO validation, conformance fixtures. | Old drafts remain accepted unchanged; unknown version/operation/capability behavior documented and tested; provider SDK does not require internal serialization knowledge. | Planned |
| 6. Scoped approval | P1 / 1 | Provider context fingerprint and migration; remaining substitution/pre-cancellation tests. | Unrelated provider edits preserve approval; config, credential, digest or registration changes revoke it; no credential delivery before approval. | Fingerprint implemented; integrity follow-up open |
| 7. Official onboarding | P1 / 6 | Add/install/verify/trust/setup/approve orchestration; concise consent; resumable prompts and CI flags. | Add/auth/query requires no manual hashes; rejection executes nothing; updates never silently adopt pins; expert workflow preserved. | Complete for GitHub; CLI PR 25 qualified |
| 8. Enterprise network | P1 / 5,6 | Explicit proxy/CA/endpoint context; validation and provider support negotiation. | Tests prove no ambient inheritance; changed network context invalidates approval; secrets never enter Git or diagnostics. | Planned |
| 9. Provider/query quality | P2 / 4,5 | Contract suite, GitHub duplicate conflicts, Google suspended evidence, AWS attachment cross-decode, traversal pruning; structured doctor/status. | Mock HTTP and native contracts pass; partial semantics remain honest; no-grant populations avoid irrelevant path limits. | Traversal and GitHub conflict handling qualified; remaining provider/diagnostic work tracked |
| 10. Stability review | P0 / 2–9 | Threat-model pass; schema/migration docs; team/contributor/README update; all-platform checks. | Format, clippy, tests, dependency checks and provider conformance pass; no stable claim while P0 items remain. | Planned |
| Future | P3 | Identity Center/Organizations and native SSO auth design; Entra; developer mode; lock-state evaluation; snapshots/diffs/policies. | Separate proposals with concrete use cases and explicit security boundaries; no automatic feature expansion during hardening. | Deferred |

Target-aware release pinning is P1 (H16): configuration currently pins one native
executable digest. A reviewed target-to-digest map is a concrete reason to evaluate
lock state; a single pin does not yet provide heterogeneous team reproducibility.
Keep that schema/trust migration separate from guided add using current pins.

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
wire remains unchanged, so native providers cannot yet emit every new dimension.
Negotiated operations/records and final contract stability review remain P0.
See [migration details](../migrations/domain-schema-2.md). This slice requires its
own complete verification before merge; none of these contracts is declared stable.
