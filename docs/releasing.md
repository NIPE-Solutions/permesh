# Release qualification

The project is pre-release. No publication workflow or automatic tag release is enabled. A successful local build is not a qualified release.

## Required gates

1. Run locked build, fmt, clippy, unit/integration/doc tests, and the Python example tests on the reviewed revision. Require Ubuntu, macOS, and Windows CI, plus the declared Rust minimum check.
2. Run cargo-deny 0.20.2 and cargo-audit 0.22.2 with fresh advisory data. Review duplicates, licenses, dependency changes, and every exception. Do not suppress unexplained advisories to make a release green.
3. Complete a credentialed GitHub fixture-organization exercise: member/team/direct observations, private visibility, denied permissions, token revocation, pagination, and explicit completeness limitations. Keep tokens and private output out of CI artifacts.
4. Verify native credential stores and terminal behavior on the supported platforms. Headless Linux keychain failure must be actionable and redacted.
5. Review security-sensitive paths and confirm the advertised CLI/JSON matches actual behavior. Record known limitations, supported targets, version, and release notes.
6. Establish a private vulnerability-reporting route and confirm maintainers can receive reports. Verify the MIT and Apache-2.0 texts ship with binary archives.
7. Before publication, prepare reproducible target builds where possible, archive contents, checksums, provenance/attestation policy, installation/uninstallation instructions, and an independently reviewed release workflow with minimal scoped write permissions. Signing/notarization and Windows reputation remain explicit distribution decisions.

No release gate is marked complete by this document. Record actual revision, commands, date, platforms, and acceptance results when qualifying a release.

## Qualification evidence (2026-09-08)

Revision `9ad15045394e69f20d2541949730b806608641e8` passed the following checks:

- [CI](https://github.com/NIPE-Solutions/permesh/actions/runs/34210661993): Ubuntu, macOS and Windows formatting, builds, Clippy and tests, plus the Rust 1.91 minimum-version check.
- [Dependency security](https://github.com/NIPE-Solutions/permesh/actions/runs/34210662040): pinned cargo-deny and cargo-audit checks with fresh advisory data.
- [Native candidates](https://github.com/NIPE-Solutions/permesh/actions/runs/34210668223): all five targets built, passed tests and offline demo smoke checks, and uploaded archives. Downloaded archives independently passed SHA-256 and four-file allowlist verification. Artifacts expire after seven days; these are unsigned candidates, not published releases.
- Local macOS Apple Silicon Keychain: a unique synthetic credential was stored through `auth login --token-stdin`, read through `auth status`, independently compared using the OS credential facility, deleted through `auth logout`, and confirmed absent. Configuration remained unchanged and captured command output contained no credential. No provider requests were needed for this check.
- Live GitHub invalid-credential handling: `doctor --json` returned exit 3 with an incomplete report; a mixed demo/GitHub user query returned exit 4 while preserving demo access. Both used a deliberately invalid synthetic token and retained no token in output. This tests authentication rejection, not actual token revocation or insufficient scopes.

Windows Credential Manager and Linux Secret Service integration, interactive terminal behavior, dedicated live team/pagination/permission/revocation exercises, vulnerability-reporting setup and distribution review remain open. Hosted runner success does not establish native credential-store behavior. Live credentials and access reports are excluded from this evidence.

An [initial live smoke test](getting-started.md#live-validation) passed on macOS Apple Silicon on 2026-09-08 using an uncommitted development build. This supplies early integration evidence for connectivity and observed access, but does not qualify a release revision or complete the broader credentialed exercise in gate 3. Public demo documentation contains no live identities, resource identifiers, access counts, credentials, or raw reports.

## Tool decisions (2026-09-08)

[cargo-dist](https://axodotdev.github.io/cargo-dist/book/) 0.32.0 is the candidate for future cross-platform archives, installers, and release orchestration. Defer adopting its generated workflow until repository identity, target support, signing strategy, and the gates above are settled. Generating an installer now would imply a distribution promise the project has not validated. Review generated workflows and pin their actions; do not execute downloaded installer scripts in CI.

[cargo-nextest](https://nexte.st/docs/running/) 0.9.143 provides process isolation and richer scheduling/reporting. Keep `cargo test --workspace --locked` as the required runner while the suite is small: no additional runner installation, simple contributor parity, and doctests run through Cargo. Reevaluate nextest when timing data shows a benefit; preserve a separate `cargo test --doc --workspace --locked` step when changing runners.

CI installs [cargo-deny](https://embarkstudios.github.io/cargo-deny/) and [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) from exact reviewed versions using `cargo install --locked`. The action revisions were resolved from the upstream GitHub tag API on 2026-09-08: checkout v4 `11d5960a326750d5838078e36cf38b85af677262`, setup-python v5 `a26af69be951a213d495a4c3e4e4022e16d87065`. Renovation of these pins needs review; a SHA is immutable identity, not a security audit. Hosted runner images and stable Rust intentionally track maintained versions; the minimum-version job detects baseline drift.

## Unsigned native candidates

[Native candidates](../.github/workflows/candidates.yml) runs on pull requests or manual `workflow_dispatch`; it has no tag, publication, registry, or GitHub Release step. Repository permissions are `contents: read`, checkout does not persist credentials, and artifacts expire after seven days. Each job verifies that the Rust host matches its target, runs locked native workspace tests, builds the optimized binary, runs the offline demo smoke check, and packages only that binary, `LICENSE` (both license texts), and static `INSTALL.txt`. Only the archive and adjacent SHA-256 checksum enter the uploaded artifact. No workspace, credentials, configuration, access report, or runner log is uploaded by this workflow.

| Candidate target | Native GitHub runner | Archive |
| --- | --- | --- |
| `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |
| `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-2025` | `.zip` |

Runner labels and architectures were checked against [GitHub's hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) on 2026-09-08. Availability and quotas depend on repository/account settings. These jobs establish behavior on the actual runner image only: they do not establish compatibility with older macOS, Windows, or GNU/Linux versions. Record the runner image version from each successful run during qualification. Linux builds dynamically link the runner's system libraries; no portable-musl claim is made.

The [artifact action](https://github.com/actions/upload-artifact/releases/tag/v7.0.1) is pinned to v7.0.1 commit `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a`, verified via the [upstream tag API](https://api.github.com/repos/actions/upload-artifact/git/ref/tags/v7.0.1) on 2026-09-08. The existing checkout v4 and setup-python v5 pins above were rechecked against their upstream tag APIs. Pin review remains a maintenance responsibility.

The Python standard-library packager is intentionally limited to these five targets, one executable, and two static documents. It rejects input symlinks and unsafe target/version names and requires a new output directory. Tar, gzip and zip metadata are fixed, including executable permissions; identical input bytes produce identical archives under the same Python/compression implementation. This does **not** claim reproducible Rust builds or byte identity across compressor versions. Checksums detect corruption, not publisher authenticity. The project license texts do not replace the dependency notice review required before public distribution.

To exercise a local candidate, substitute the host target and workspace version:

```sh
cargo test --workspace --locked --target aarch64-apple-darwin
cargo build --release --locked --package permesh-cli --bin permesh --target aarch64-apple-darwin
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/smoke_candidate.py target/aarch64-apple-darwin/release/permesh
python3 scripts/package_candidate.py --target aarch64-apple-darwin --version 0.1.0-dev --output candidate-output
```

Use `python` and `permesh.exe` on Windows. `candidate-output` must not already exist, and its parent path must be free of symlinks (use a canonical path if your temporary directory is aliased). The smoke helper uses an automatically removed synthetic workspace; it performs no live-provider or credential-store validation. Candidates include local invocation and uninstall instructions. All candidates remain unsigned and unnotarized; successful jobs do not qualify a public release or complete live GitHub, keychain, dependency-notice, signing, attestation, or vulnerability-reporting gates.

Keep cargo-dist deferred for this bounded candidate phase. Its installer and release orchestration would add publication policy that these jobs deliberately do not implement. Reevaluate adoption before public distribution rather than growing this helper into a release framework.
