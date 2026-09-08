# Hardening backlog

Status as of 2026-09-08. The [audit](architecture-hardening.md) records the
baseline and breaking-change ledger. Each slice requires implementation,
negative tests, documentation and migration notes before it is complete.

| Slice | Priority / dependencies | Tasks | Acceptance criteria | Status |
| --- | --- | --- | --- | --- |
| 1. Baseline | P0 | Audit both repositories; identify compatibility surfaces and risks. | Evidence-backed audit and ordered breaking-change ledger exist before refactoring. | Complete |
| 2. Boundary freeze | P0 / 1 | Historical wire transcripts; wire-owned records/capabilities and private validated mapping; CLI-owned query/snapshot DTOs; versioning ADR. Official emitter and control-command DTO migration remain. | Old transcripts and full JSON fixtures survive extraction; malformed input still fails closed; no implicit schema change. | In progress |
| 3. Concrete defects | P1–P2 / 1 | Bounded revision-aware config writes; correct unsupported operation errors. | Regressions reproduce original failures and pass; rejected writes preserve original file; official responses decode correctly. | Implemented; cross-platform CI pending |
| 4. Domain semantics | P0 / 2 | Independent kind/affiliation/lifecycle; resource type/parent; evidence kind/certainty; normalization guidance. | External inactive humans and inactive service principals are representable; parent cycles rejected; assignments never presented as proved effective access; stable IDs retained. | Planned |
| 5. Negotiated protocol | P0 / 2,4 | Explicit version/operation capability contract, diagnostics, independent DTO validation, conformance fixtures. | Old drafts remain accepted unchanged; unknown version/operation/capability behavior documented and tested; provider SDK does not require internal serialization knowledge. | Planned |
| 6. Scoped approval | P1 / 1 | Provider context fingerprint and migration; remaining substitution/pre-cancellation tests. | Unrelated provider edits preserve approval; config, credential, digest or registration changes revoke it; no credential delivery before approval. | Fingerprint implemented; integrity follow-up open |
| 7. Official onboarding | P1 / 6 | Add/install/verify/trust/setup/approve orchestration; concise consent; resumable prompts and CI flags. | Add/auth/query requires no manual hashes; rejection executes nothing; updates never silently adopt pins; expert workflow preserved. | Planned |
| 8. Enterprise network | P1 / 5,6 | Explicit proxy/CA/endpoint context; validation and provider support negotiation. | Tests prove no ambient inheritance; changed network context invalidates approval; secrets never enter Git or diagnostics. | Planned |
| 9. Provider/query quality | P2 / 4,5 | Contract suite, GitHub duplicate conflicts, Google suspended evidence, AWS attachment cross-decode, traversal pruning; structured doctor/status. | Mock HTTP and native contracts pass; partial semantics remain honest; no-grant populations avoid irrelevant path limits. | Planned |
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
decoder exposes a snapshot. Control-command JSON DTOs and official runtime emission
remain separate work before domain migration.

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
Cross-platform and supply-chain CI are required before merging. No live tenant
qualification or new release is implied.
