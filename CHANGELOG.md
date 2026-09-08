# Changelog

User-visible changes are recorded here. Alpha releases are for evaluation; stable qualification remains open.

## Unreleased

- Separate legacy provider discovery records and CLI schema-1 query, snapshot and control-command output from internal domain serialization. Frozen protocol and JSON fixtures preserve existing field and enum contracts.
- Scope workspace approval to the selected provider, credential references, relevant identity aliases/authority and registration. **Existing full-workspace approvals require one fresh review and approval.** Unrelated provider and organization changes then preserve approval; changed security inputs still fail closed.
- Reject provider-add YAML updates that exceed the workspace load limit. Provider add/setup share captured-revision checks so detected concurrent edits leave the current configuration intact.
- Add an implementation-backed architecture audit, prioritized hardening backlog and explicit migration decisions. Domain, negotiated protocol and onboarding proposals remain unstabilized.

## 0.1.0-alpha.2

Evaluation prerelease built from [`aad5839`](https://github.com/NIPE-Solutions/permesh/commit/aad58397e139ed755fcf983da8e9f045e7e5a556) in [attested run 34273757910](https://github.com/NIPE-Solutions/permesh/actions/runs/34273757910); see [release notes](docs/releases/0.1.0-alpha.2.md). Published alpha.1 assets remain unchanged.

- Add explicit `auth login INSTANCE --browser [--no-open]` for approved external providers declaring OAuth browser login. The host owns PKCE, state, a bounded loopback callback and token exchange, then stores only the configured same-instance refresh credential in the native keychain. Ordinary queries never open a browser or persist tokens.
- Add optional protocol draft 4 browser-authentication descriptions while preserving draft-2 discovery, draft-3 setup and existing provider catalog compatibility. OAuth client details remain user-supplied; provider binaries and complete workspace configuration require approval before execution or credential access.
- Extend explicit provider migration to Google directory instances, preserving customer IDs, canonical identity mappings, authority and token references without execution or credential access.
- Add synthetic controlling-PTY acceptance checks on native macOS/Linux candidate jobs: prompts, hidden input, EOF, echo restoration and output redaction. Windows terminal and live browser/tenant qualification remain open.
- Include a target-bound dependency inventory and checksum beside each candidate archive, binding the executable and Cargo.lock digests to the target and version. This is Permesh inventory schema 1, not a standards SBOM.
- Add a manual same-run GitHub provenance attestation workflow and independent subject verification for candidate archives and inventories. Alpha.2 publishes the 20 verified same-run subjects and attestation bundle; Apple notarization and Windows Authenticode are not included.
- Clarify the separation between provider source readiness and independently published provider releases. CLI upgrades never install, update or trust provider packages automatically.

## 0.1.0-alpha.1 — 2026-09-08

- Bound external-provider supervisor stack usage so setup remains reliable on small native main-thread stacks, including Windows.

- Remove the bundled GitHub HTTP adapter. Legacy GitHub configurations remain readable for explicit migration; queries, health and credential commands fail with migration guidance before credential access. New GitHub instances use the separate install/trust/setup/approval flow; Google and demo remain bundled.

- Add explicit `provider migrate INSTANCE --sha256 DIGEST` for legacy GitHub configurations, preserving IDs, aliases and token references without provider execution or credential access. Existing external workspace approvals require renewed review after the configuration changes; bundled execution was retained in that migration step and is now removed.

- Standardize the main repository on MIT with one license file and consistent package metadata and SPDX identifiers. Earlier revisions and third-party dependencies retain their existing terms.

- Add declarative external-provider setup: validated SDK schema 1, draft-3 description exchanges, CLI-owned conditional prompts and strict YAML answers. Setup requires an already trusted binary, writes settings and credential references only, and leaves workspace approval explicit.

- Add approved native external providers to workspace user/admins/orphaned queries and doctor/status health checks. Local approval binds the canonical configuration path, full normalized configuration and registered binary metadata; clones, edits and revocation fail closed before external credential resolution or launch.
- Add draft-2 configuration and named env/keychain credential delivery over stdin after exact handshake matching, with bounded zeroizing requests and decoded response-reflection checks. Add external review/approve/revoke and named auth slots; native code remains unsandboxed.
- Add explicit native external-provider inspection, local digest-bound trust registration and draft-1 standalone discovery with bounded streams and cancellation cleanup.

- Add bounded offline external-provider protocol validation, normalized Python interoperability fixtures and a developer transcript validator; workspace execution requires separate local approval.

- Add static shell completion for Bash, Zsh, Fish, PowerShell and Elvish without workspace or provider access.

- Add `permesh orphaned` with explicit authoritative sources, conservative inactive/unmatched/ambiguous classifications, separate non-human/external accounts, preserved paths and unassessed results on authority failure.

- Add a read-only Google Workspace directory source with explicit customer/authority selection, immutable identity IDs, conservative account status and supplied OAuth access tokens. Live tenant qualification remains pending.

- Add `permesh admins` with JSON and human output, retaining elevated/admin/owner roles, unknown privilege, ambiguous identities and unresolved group grants.
- Share indexed identity resolution and bounded graph traversal; prune irrelevant standard-access branches before administrator inspection.
- Add five-target native candidate CI with allowlisted archives, checksums and synthetic smoke checks; artifacts are unsigned and do not constitute a release.

- Add the first executable demo and GitHub observed-access flow: init, doctor, provider metadata/status, auth, user queries and schema 1 JSON.
- Preserve immutable account IDs, verified/explicit identity evidence, inheritance paths, provenance, privilege semantics and partial failures.
- Bound provider requests, retries, response sizes and native I/O; make Ctrl+C cancel waits without blocking runtime shutdown.
- Define strict configuration, redacted secret references, provider observations, and versioned output boundaries.
- Add contributor and security documentation, project licensing, platform CI, and dependency policy.
- Document a draft external-provider protocol and a synthetic Python example; the CLI does not execute it.

Stable release remains gated on the checks in [releasing.md](docs/releasing.md). See the [alpha release notes](docs/releases/0.1.0-alpha.1.md) for scope and limitations.

- Add explicit official provider install/update/check commands, bounded catalog and ZIP validation, retained package versions, and digest-pinned trust lookup. No automatic update checks or provider execution during installation.

- Distribute unsigned alpha CLI archives for five native targets with checksums, MIT license, dependency notices, and installation instructions. Enable private vulnerability reporting.
