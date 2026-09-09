# Release qualification

The project distributes an explicitly marked alpha for evaluation. Stable v0.1 qualification remains incomplete. A separate manual [attested candidate workflow](artifact-attestations.md) can build and verify signed provenance after review. It does not perform platform code signing. No automatic tag publication is enabled; reviewed native artifacts are promoted manually without rebuilding. A successful local build is not a qualified release.

## Alpha publication policy

For each alpha, require locked fmt/Clippy/tests, minimum Rust, fresh dependency checks, all five native build/test/smoke jobs, independent archive/checksum/notice inspection, exact binary version checks, and a public-catalog install/update smoke test. Private vulnerability reporting must be enabled. Promote only the exact reviewed source tree and original qualified artifact bytes to a draft GitHub prerelease; verify uploaded names, sizes and SHA-256 digests before publication. Tag the merged revision, confirm it has the qualified tree, and record run URLs and hashes in the release notes. Do not mark this release as latest stable.

Interactive Windows/Linux credential-store and terminal acceptance, broader live GitHub fixtures and Google tenant/browser acceptance remain **stable-release gates**. The historical [alpha.1 notes](releases/0.1.0-alpha.1.md) describe its immutable published assets. For the [alpha.2 release](releases/0.1.0-alpha.2.md), require fresh five-target evidence, inventory/checksum inspection, and verification of provenance for the exact merged release revision before publication. A standards SBOM, platform code signing and notarization remain absent; synthetic tests do not establish live-provider or desktop acceptance.

Packaging includes the exact project MIT `LICENSE` and complete source-supplied dependency license/notice texts in `THIRD-PARTY-NOTICES.txt`, collected from the locked target dependency graph. Collection fails on missing or unsafe source notices. Re-check the contents whenever dependencies change. Build jobs have read-only permissions and no release credentials; a maintainer publishes with the [GitHub CLI](https://cli.github.com/manual/gh_release_create) after all gates above pass.

Private vulnerability reporting was enabled and verified via the repository API on 2026-09-08. The reporting route is linked in [SECURITY.md](../SECURITY.md).

Alpha.2's [release evidence](releases/0.1.0-alpha.2.md#release-evidence) identifies
source `aad58397e139ed755fcf983da8e9f045e7e5a556` and [attested run 34273757910](https://github.com/NIPE-Solutions/permesh/actions/runs/34273757910).
Later releases must repeat qualification for their own exact merged revision.

## Additional alpha.2 publication gates

Alpha.2 must satisfy every alpha gate above **and** all of the following. These
are requirements, not claims that a candidate or release has already passed.

1. Merge the reviewed release source into `main`, record its full commit SHA,
   and dispatch the manual attested-candidate workflow with that exact SHA. A
   successful PR candidate run, a different merge revision, or a later rebuild
   cannot substitute for this run.
2. Require all five native jobs and the downstream attestation verification job
   from that same run to succeed. Independently verify the complete 20-file set:
   five archives, five dependency inventories, and their ten checksum files.
   Check archive allowlists, binary/version/target bindings and reviewed lockfile
   digests, then verify each signed subject against the repository, exact signing
   workflow identity, source and signer SHA, `refs/heads/main`, SLSA v1 predicate
   and GitHub-hosted-runner restriction in the [verification procedure](artifact-attestations.md).
3. Download the successfully verified same-run `attestation-bundle.json`. Publish
   that exact bundle alongside all 20 original candidate files in the draft
   alpha.2 prerelease. Record the full source SHA, workflow run URL, artifact
   names, sizes and SHA-256 hashes. Do not rebuild, recompress or alter a subject
   after verification; keep the immutable alpha.1 assets unchanged.
4. Download the staged release assets independently, compare all 20 subjects and
   the bundle with the qualified originals, and rerun verification. Publish only
   after those checks pass. After publication, verify all 20 files using the
   public repository attestation lookup as well as the distributed bundle; record
   both results and their commands on the release page. A lookup, subject or
   bundle failure blocks a claim that the public release is fully verified.

The 20 subjects receive signed build provenance. Their executables still have no
Apple Developer ID/notarization or Windows Authenticode signatures. Provenance
is therefore mandatory for alpha.2 publication; platform signing, standards SBOM
adoption and the live/desktop acceptance gates remain separate unfinished work.

## Stable release gates

1. Run locked build, fmt, clippy, unit/integration/doc tests, and the Python example tests on the reviewed revision. Require Ubuntu, macOS, and Windows CI, plus the declared Rust minimum check.
2. Run cargo-deny 0.20.2 and cargo-audit 0.22.2 with fresh advisory data. Review duplicates, licenses, dependency changes, and every exception. Do not suppress unexplained advisories to make a release green.
3. Complete a credentialed GitHub fixture-organization exercise: member/team/direct observations, private visibility, denied permissions, token revocation, pagination, and explicit completeness limitations. Keep tokens and private output out of CI artifacts.
4. Verify native credential stores and terminal behavior on the supported platforms. Headless Linux keychain failure must be actionable and redacted.
5. Review security-sensitive paths and confirm the advertised CLI/JSON matches actual behavior. Record known limitations, supported targets, version, and release notes.
6. Establish a private vulnerability-reporting route and confirm maintainers can receive reports. Verify the MIT license text ships with binary archives.
7. Before publication, prepare reproducible target builds where possible, archive contents, checksums, provenance/attestation policy, installation/uninstallation instructions, and an independently reviewed release workflow with minimal scoped write permissions. Signing/notarization and Windows reputation remain explicit distribution decisions.

No release gate is marked complete by this document. Record actual revision, commands, date, platforms, and acceptance results when qualifying a release.

## Qualification evidence (2026-09-08)

Revision `9ad15045394e69f20d2541949730b806608641e8` passed the following checks:

- [CI](https://github.com/NIPE-Solutions/permesh/actions/runs/34210661993): Ubuntu, macOS and Windows formatting, builds, Clippy and tests, plus the Rust 1.91 minimum-version check.
- [Dependency security](https://github.com/NIPE-Solutions/permesh/actions/runs/34210662040): pinned cargo-deny and cargo-audit checks with fresh advisory data.
- [Native candidates](https://github.com/NIPE-Solutions/permesh/actions/runs/34210668223): all five targets built, passed tests and offline demo smoke checks, and uploaded archives. Downloaded archives independently passed SHA-256 and four-file allowlist verification. Artifacts expire after seven days; these are unsigned candidates, not published releases.
- Local macOS Apple Silicon Keychain: a unique synthetic credential was stored through `auth login --token-stdin`, read through `auth status`, independently compared using the OS credential facility, deleted through `auth logout`, and confirmed absent. Configuration remained unchanged and captured command output contained no credential. No provider requests were needed for this check.
- Live GitHub invalid-credential handling: `doctor --json` returned exit 3 with an incomplete report; a mixed demo/GitHub user query returned exit 4 while preserving demo access. Both used a deliberately invalid synthetic token and retained no token in output. This tests authentication rejection, not actual token revocation or insufficient scopes.

At that revision, Windows Credential Manager and Linux Secret Service integration, interactive terminal behavior, dedicated live team/pagination/permission/revocation exercises, vulnerability-reporting setup and distribution review remained open. Current CI now includes synthetic Windows/Linux credential-store round trips and Linux unavailable-service redaction; interactive desktop acceptance is still pending. Private reporting and alpha distribution policy are addressed above. Hosted runner success does not establish native credential-store behavior. Live credentials and access reports are excluded from this evidence.

An [initial live smoke test](getting-started.md#live-validation) passed on macOS Apple Silicon on 2026-09-08 using an uncommitted development build. This supplies early integration evidence for connectivity and observed access, but does not qualify a release revision or complete the broader credentialed exercise in gate 3. Public demo documentation contains no live identities, resource identifiers, access counts, credentials, or raw reports.

## Candidate dependency inventory

Each new native candidate includes an adjacent
`permesh-VERSION-TARGET.dependencies.json` and its own `.sha256` file. This is
Permesh inventory format version 1, not a CycloneDX/SPDX SBOM or an attestation.
The archive contents remain the same four files. Existing published alpha assets
are not retroactively changed.

The inventory records the executable SHA-256, Cargo.lock SHA-256, target, version,
and normal/build dependency edges from the same locked, target-filtered Cargo
metadata used for dependency notices. Dev-only dependencies are excluded. Each
package has a name, version, source category (`local` or `crates.io`), and, for
registry packages, the crate checksum from Cargo.lock. Output is deterministic
for identical inputs; metadata paths, authors, descriptions, arbitrary package
metadata, environment values and registry URLs are excluded. Unknown registries
and Git sources require a separate review and currently fail packaging. The
packager rejects a changed lockfile, missing registry checksums, a mismatched
candidate version, linked inputs, and existing output directories.

This is a Cargo dependency-resolution inventory with default features. Build
edges are explicitly labeled, but proc macros and Cargo feature unification can
make the graph broader than the machine code retained by the linker. It does not
inventory system libraries, external provider executables or their dependencies,
or establish vulnerability freedom, reproducible Rust builds or publisher
identity. Complete dependency license texts still ship separately in the archive.

### Standards generator evaluation (2026-09-08)

[cargo-cyclonedx 0.5.9](https://docs.rs/crate/cargo-cyclonedx/0.5.9) is Apache-2.0,
released 2026-03-19, with Rust 1.85 minimum according to its
[changelog](https://github.com/CycloneDX/cyclonedx-rust-cargo/blob/main/cargo-cyclonedx/CHANGELOG.md).
[cargo-sbom 0.10.0](https://docs.rs/crate/cargo-sbom/0.10.0) is MIT, released
2025-06-17; its package manifest declares no Rust minimum. Neither is adopted.

Auditing the registry-supplied tool lockfiles with cargo-audit 0.22.2 and fresh
RustSec database revision `bf25f6575a93a35f30796c65c0ed91bee7fa19fd` found:

- cargo-cyclonedx: time 0.3.36
  ([RUSTSEC-2026-0009](https://rustsec.org/advisories/RUSTSEC-2026-0009.html)),
  anyhow 1.0.80
  ([RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190.html)),
  rand 0.8.5
  ([RUSTSEC-2026-0097](https://rustsec.org/advisories/RUSTSEC-2026-0097.html)),
  and yanked xml-rs 0.8.19.
- cargo-sbom: anyhow 1.0.98 (RUSTSEC-2026-0190). Default audit exit status was
  zero because this is an informational unsoundness warning; the JSON warning
  records were reviewed as well as the exit status.

These findings concern the generators' own locked dependencies, not Permesh's
production dependency graph. No advisory exceptions, floating tool dependency
resolution, tooling fork, or generator install was added to CI. Reevaluate a
maintained generator release with a clean reviewed lockfile before completing the
standards SBOM gate. Also account for the documented
[target-feature overapproximation](https://github.com/CycloneDX/cyclonedx-rust-cargo/issues/871)
when describing Cargo-derived inventories. Platform signing and notarization remain
open; alpha.2 provenance verification is required by the publication gates above.

## Automated Unix terminal acceptance

Native candidate jobs run `scripts/check_terminal.py` against the optimized CLI
on all four macOS/Linux targets using a real controlling pseudo-terminal. The
checks exercise interactive organization input, JSON mode without prompts even
when attached to a terminal, hidden credential entry, EOF handling, restoration
of terminal echo, and exclusion of synthetic input from captured terminal output.
The credential check types synthetic text, clears it, and sends EOF without ever
submitting a nonempty credential. Each test uses a temporary workspace and a
unique credential instance; an unconditional logout cleanup guard handles an
unexpected synthetic write. No provider request is made and captured terminal
output is not uploaded or included in assertion failures.

Run locally after building a native binary:

```sh
python3 scripts/check_terminal.py target/debug/permesh
```

These checks supply runner-specific terminal evidence only after the corresponding
native job succeeds. They do not qualify Windows terminal behavior, interactive
desktop credential stores, or manual accessibility/terminal-emulator acceptance.
These PTY checks do not generate SBOMs or signatures. The separate attested
workflow supplies provenance; platform signing and notarization remain open.

## Earlier tool evaluation (2026-09-08)

[cargo-dist](https://axodotdev.github.io/cargo-dist/book/) 0.32.0 is the candidate for future cross-platform archives, installers, and release orchestration. Defer adopting its generated workflow until repository identity, target support, signing strategy, and the gates above are settled. Generating an installer now would imply a distribution promise the project has not validated. Review generated workflows and pin their actions; do not execute downloaded installer scripts in CI.

[cargo-nextest](https://nexte.st/docs/running/) 0.9.143 provides process isolation and richer scheduling/reporting. Keep `cargo test --workspace --locked` as the required runner while the suite is small: no additional runner installation, simple contributor parity, and doctests run through Cargo. Reevaluate nextest when timing data shows a benefit; preserve a separate `cargo test --doc --workspace --locked` step when changing runners.

CI installs [cargo-deny](https://embarkstudios.github.io/cargo-deny/) and [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) from exact reviewed versions using `cargo install --locked`. The action revisions were resolved from the upstream GitHub tag API on 2026-09-08: checkout v4 `11d5960a326750d5838078e36cf38b85af677262`, setup-python v5 `a26af69be951a213d495a4c3e4e4022e16d87065`. Renovation of these pins needs review; a SHA is immutable identity, not a security audit. Hosted runner images and stable Rust intentionally track maintained versions; the minimum-version job detects baseline drift.

## Native candidate builds without platform signing

[Native candidates](../.github/workflows/candidates.yml) runs on pull requests or manual `workflow_dispatch`; it has no tag, publication, registry, or GitHub Release step. Repository permissions are `contents: read`, checkout does not persist credentials, and artifacts expire after seven days. Each job verifies that the Rust host matches its target, runs locked native workspace tests, builds the optimized binary, runs the offline demo smoke check and Unix terminal acceptance where supported, and packages only that binary, `LICENSE` (MIT license text), `THIRD-PARTY-NOTICES.txt`, and static `INSTALL.txt`. The archive, dependency-inventory JSON, and their adjacent SHA-256 checksums enter the uploaded artifact. No workspace, credentials, configuration, access report, or runner log is uploaded by this workflow.

| Candidate target | Native GitHub runner | Archive |
| --- | --- | --- |
| `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |
| `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-2025` | `.zip` |

Runner labels and architectures were checked against [GitHub's hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) on 2026-09-08. Availability and quotas depend on repository/account settings. These jobs establish behavior on the actual runner image only: they do not establish compatibility with older macOS, Windows, or GNU/Linux versions. Record the runner image version from each successful run during qualification. Linux builds dynamically link the runner's system libraries; no portable-musl claim is made.

The [artifact action](https://github.com/actions/upload-artifact/releases/tag/v7.0.1) is pinned to v7.0.1 commit `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a`, verified via the [upstream tag API](https://api.github.com/repos/actions/upload-artifact/git/ref/tags/v7.0.1) on 2026-09-08. The existing checkout v4 and setup-python v5 pins above were rechecked against their upstream tag APIs. Pin review remains a maintenance responsibility.

The Python standard-library packager is intentionally limited to these five targets, one executable, the project license, collected dependency notices, and installation instructions. It rejects input symlinks and unsafe target/version names and requires a new output directory. Tar, gzip and zip metadata are fixed, including executable permissions; identical input bytes produce identical archives under the same Python/compression implementation. This does **not** claim reproducible Rust builds or byte identity across compressor versions. Checksums detect corruption, not publisher authenticity. Dependency notices are included separately; the project license does not replace their terms.

To exercise a local candidate, substitute the host target and workspace version:

```sh
cargo test --workspace --locked --target aarch64-apple-darwin
cargo build --release --locked --package permesh-cli --bin permesh --target aarch64-apple-darwin
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/smoke_candidate.py target/aarch64-apple-darwin/release/permesh
python3 scripts/package_candidate.py --target aarch64-apple-darwin --version 0.1.0-alpha.2 --output candidate-output
```

Use `python` and `permesh.exe` on Windows. `candidate-output` must not already exist, and its parent path must be free of symlinks (use a canonical path if your temporary directory is aliased). The smoke helper uses an automatically removed synthetic workspace; it performs no live-provider or credential-store validation. Candidates include local invocation and uninstall instructions. Candidate executables remain without platform code signatures or notarization. Standalone candidate jobs create no provenance attestations; the downstream job in the separate attested workflow signs and verifies its same-run subjects. Candidate jobs alone do not authorize promotion: complete the alpha publication checks above. Stable native credential-store and live-provider qualification remains open.

Keep cargo-dist deferred while releases use manual promotion of these bounded, reviewed artifacts. Reevaluate it when adding installers or package-manager distribution; do not grow this helper into a release framework.

For the manual same-run provenance workflow, public verification commands, and remaining platform-signing prerequisites, see [artifact attestations](artifact-attestations.md).

## Next access-review candidate checklist

The access-review source program is unpublished. Its local source version still
matches alpha.2; do not name or upload new bytes as an existing published asset.
Select a new prerelease version explicitly before packaging. No next version,
release date or catalog publication is implied by this checklist.

- Integrate and validate the exact intended source revision, including identity,
  inventory, snapshot/offboard, resource/policy, contributor tooling, remote
  credentials and temporary AWS-profile slices. Re-run combined regressions;
  earlier local 503-test and synthetic-demo results are supporting evidence,
  not qualification of a later release commit.
- Review independent access JSON, control output, snapshot, diff and advisory
  plan/report versions and their migration notes. New commands and local
  artifacts do not implicitly change provider wire contracts or stabilize the SDK.
- Build compatible CLI and provider candidates for all supported native targets.
  Verify checksums, dependency inventories, license notices and attestations
  against exact source revisions and build workflows. Qualify GitLab, Entra and
  Identity Center separately from the original four providers; binary count is
  not package or live-acceptance evidence.
- Exercise the exact packaged installation, native trust, declarative setup,
  scoped approval, explicit credentials and read-only queries together. Verify
  negotiated metadata and portable exact-version target pins survive catalog
  selection; missing or incompatible entries must fail before credential delivery.
- Record actual live/API, desktop credential-store and signing prerequisites in
  the [delivery gap map](audits/architecture-hardening-backlog.md#access-review-delivery-evidence-and-remaining-gates).
  Keep unsupported or untested combinations explicit in release notes.
- Review immutable archive names, signatures/provenance and catalog entries before
  authorized publication. Never overwrite alpha.2 or existing provider releases;
  do not describe source-only candidate setup as available through a public catalog.
