# Changelog

User-visible changes are recorded here. Alpha releases are for evaluation; stable qualification remains open.

## Unreleased

- Add a [team setup guide](docs/team-setup.md) for a small team on GitHub and
  Google Workspace.
- Fix: someone with `status: left` is expected to hold nothing in `profile check`,
  even when a directory group still assigns them profiles.
- `access review` refuses existing plan files before discovery, escapes Markdown
  in `--detail` summaries, and names the folder and the CI remedy when a reviewed
  `index.json` has an old format.
- The starter verify workflow accepts only a successful post-merge plan run from
  this repository's default branch, and exits like the CLI for the worst plan
  (errors, then 3, 4, 6).
- `permesh mcp`: a failure to reach the client exits 5 instead of blaming the
  protocol version; symlinks that leave the workspace read as "File not found", so
  they reveal nothing about their target; empty `profiles` or `remove` lists get a
  specific message.
- Declare the formats added since the first stability declaration stable: the
  `check` and `profiles` configuration, people and profile files, policy
  `version: 2`, the check report, profile results, access plan files and the
  `access` JSON results. Golden fixtures pin them. `permesh mcp`, starter templates
  and human-readable output stay experimental
  ([ADR 0036](docs/adr/0036-second-stability-declaration.md)).
- People and profile files accept an optional `version: 1`; any other value is
  an error that says what to do. `access review` writes `index.json` as
  `{"access_review_index_version": 1, "plans": [...]}`.
- Add `permesh check`: one unattended review run that writes a snapshot,
  a JSON report and a counts-only summary. It compares with a previous snapshot,
  including admin access gained through groups, and exits 6 when something needs
  review. See [scheduled checks](docs/check.md).
- Add optional check notifications to Slack, Microsoft Teams, a generic webhook or
  email, counts only by default ([ADR 0032](docs/adr/0032-explicit-check-notifications.md)).
- Add `permesh init --starter` with GitHub Actions and GitLab CI workflows for
  weekly checks ([starter](docs/starter.md)).
- Add access profiles: `permesh profile check` compares declared
  `profiles/` and `people/` files with observed access and reports missing and
  unexplained access, including the group route; `permesh profile suggest` drafts
  a profile from a group. `permesh check` reports profile counts, report-only unless
  `profiles.enforce: true`. Policy files gain `version: 2` with `unexplained_access`
  exceptions ([access profiles](docs/profiles.md),
  [ADR 0033](docs/adr/0033-access-profiles.md)).
- Add joiner, mover and leaver plans: `permesh access join|move|leave`
  write a `permesh_access_plan` v1 file with operation-shaped steps and stable step
  IDs; moves keep access a remaining profile justifies and plan derived access at
  its membership; `permesh access verify` checks the steps and flags extra access;
  `permesh access review` plans the people changes of a pull request and binds
  regenerated plans to the reviewed copy after merge. People files accept
  `status: left`, which `permesh check` counts as departed. The starter gains plan
  and verify workflows ([access plans](docs/access-plans.md),
  [ADR 0034](docs/adr/0034-access-plans.md)).
- Add experimental `permesh mcp`: a local, read-only MCP server (protocol
  2026-07-28) whose tools answer user, resource, admin, orphaned and profile
  questions, prepare join, move and leave plan files inside the workspace, and
  verify plans. Results carry the CLI's report, exit code and completeness. It never
  changes access ([AI assistants](docs/mcp.md),
  [ADR 0035](docs/adr/0035-read-only-mcp-server.md)).

- Guided `provider add` now supports GitLab, Entra ID and AWS Identity Center.
- `provider list` reports each external provider's ID and whether it has recorded
  live tenant acceptance (`provider`, `live_tested`); human output marks untested
  providers.
- Declare the [beta stability line](docs/stability.md): stable configuration,
  JSON schemas, exit codes and review file formats, with a migration policy.
- Add a Homebrew formula and Scoop manifest generator for release archives, and
  a live tenant acceptance runbook.
- Rewrite the README around access workflows; release and upgrade notes move to
  [release status](docs/status.md).

- Add explicitly selected temporary AWS shared-credentials profiles with
  account/role-bound provider settings; no automatic credential chain or helper execution.

- Add native provider scaffolding and offline discovery/health/setup transcript
  validation. Neither operation executes, installs or trusts generated code.
- Add resource-centric inspection and focused local review policies. Policy findings
  use exit 6; incomplete and failed collections retain precedence without hiding findings.

- Add explicit private snapshots, offline inspection and source-scoped diffs with
  independent format version 1. Changed visibility, failed collection and older
  exports never imply revoked access. Existing provider wire schemas are unchanged.
- Add read-only departure assessment, target-scoped advisory plans and repeatable
  verification with JSON/HTML reports. Evidence freshness, ownership dependencies
  and unsupported checks remain explicit; no remote mutation commands are added.
- Add host-owned, explicitly approved exact credential reads from 1Password
  Connect and separately dispatched Vault/OpenBao KV v2 stores. Configuration
  remains offline; remote auth status is configured/unverified. Whole item/object
  fetch, bounded HTTPS reads and live qualification limits are documented.

- Add reviewed stable-account identity mapping commands and an explicitly pinned,
  bounded JSON identity inventory. Contradictory evidence stays ambiguous; inventory
  lifecycle does not imply employment or verified email ownership.

- Recheck trusted executable pins immediately before launch and stop already
  cancelled operations before spawning. Path-based OS race limitations remain.
- Add `doctor --details` with curated diagnostic stages and remediation while
  preserving default output. Native cleanup failures consistently return exit 5.


- Add optional target-specific external executable pins and guided `--portable`
  setup for shared workspaces. Existing scalar pins remain supported; each
  machine still requires local trust and approval.


- Add explicit per-instance proxy/bypass and digest-pinned CA configuration for
  negotiated discovery/health. The host requires provider acknowledgement before
  credential delivery, preserves sanitized environments, and rejects unsupported
  legacy/browser flows. Provider rollout is qualified separately.


- Carry negotiated-v1 package compatibility into guided official provider setup;
  expose explicit discovery selection for advanced setup and migration. Legacy
  metadata remains unchanged; conflicting protocol declarations fail closed.
- Remove the duplicated bundled Google adapter. Legacy Google configuration
  remains parseable for explicit migration, but execution/authentication stop
  before reading credentials. Google external packages are not yet published;
  current source requires a reviewed native candidate. Only demo remains bundled.


- Add explicit `external.discovery_protocol: negotiated_v1` for native workspace discovery and health. Required-operation negotiation precedes credential delivery; independent wire DTOs preserve principal dimensions, resource containment and access evidence. Legacy drafts/defaults remain unchanged. Switching contracts requires fresh workspace approval; see [negotiated protocol 1](docs/provider-protocol/negotiated-v1.md).

- **Access JSON schema 2:** user, admins, orphaned and standalone external discovery now expose independent principal kind, affiliation and lifecycle, resource kind/parent, access evidence kind and path certainty. Control JSON and legacy provider wire formats stay unchanged; see [migration guidance](docs/migrations/domain-schema-2.md).
- Model suspension separately from inactivity, including in the bundled Google adapter. Orphan review prioritizes inactive/suspended identities even for service, bot and external principals. Resource containment is validated and never invents access inheritance.

- Prune membership branches that cannot reach observed grants in user and orphaned queries, keeping accounts without grants reviewable without consuming access-path limits. Preserve observed paths, provenance and ordering; compare path sort keys without allocating temporary vectors. Bound cumulative path-copy work to prevent large labels multiplying into unbounded materialized results; exhaustion fails explicitly.
- Separate legacy provider discovery records and CLI schema-1 query, snapshot and control-command output from internal domain serialization. Frozen protocol and JSON fixtures preserve existing field and enum contracts.
- Guide official GitHub add through verified download, explicit native-code trust, declarative setup and separate workspace approval. Automation requires an answer file and explicit risk acceptance; authentication stays separate and current-host pins remain unchanged.

- Scope workspace approval to the selected provider, credential references, relevant identity aliases/authority and registration. **Existing full-workspace approvals require one fresh review and approval.** Unrelated provider and organization changes then preserve approval; changed security inputs still fail closed.
- Reject provider-add YAML updates that exceed the workspace load limit. Provider add/setup share captured-revision checks so detected concurrent edits leave the current configuration intact.
- Add an implementation-backed architecture audit, prioritized hardening backlog and explicit migration decisions. Domain semantics and negotiated protocol work remain unstabilized.

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
