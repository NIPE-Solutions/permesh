# Changelog

User-visible changes are recorded here. No public CLI release has been qualified.

## Unreleased

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

Release remains gated on the checks in [releasing.md](docs/releasing.md). This entry is not a statement that all release gates have passed.

- Add explicit official provider install/update/check commands, bounded catalog and ZIP validation, retained package versions, and digest-pinned trust lookup. No automatic update checks or provider execution during installation.
