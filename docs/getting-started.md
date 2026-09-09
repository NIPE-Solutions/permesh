# Getting started

Start with the offline demo, then connect a real provider in a separate workspace.
The demo needs no account, credential, or network connection.

## Install

Use the [installation guide](installation.md) to download and verify the published
0.1.0-alpha.2 binary for macOS, Linux or Windows.

This guide follows current source. Alpha.2 has earlier output and contracts;
see [release limitations](releases/0.1.0-alpha.2.md) and
[source upgrade notes](migrations/domain-schema-2.md). To build current source,
install Rust 1.91 or newer, then:

```bash
git clone https://github.com/NIPE-Solutions/permesh.git
cd permesh
cargo install --path crates/permesh-cli --locked
```

`cargo install permesh` is not available on crates.io. Build-time Cargo downloads
are dependency retrieval, not application telemetry.

## Evaluate offline

```bash
mkdir access-demo
cd access-demo
permesh init --demo --organization Example
permesh doctor
permesh user alice@example.com
permesh admins
permesh orphaned
```

In the current demo:

- Alice reaches the payments API through `team/backend` and has a separate read path to infrastructure.
- Bob has a known administrative path.
- `former@example.com` is inactive in the synthetic directory but still has access.
- A contractor, service account and bot appear separately in orphan review.

These are fictional records. Demo observation times are fixed; the execution
window uses current UTC. `orphaned` reviews identity evidence, not an employment
policy, and never removes access.

For automation, add `--json`:

```bash
permesh user alice@example.com --json
permesh admins --json
```

Inspect completeness and provider limitations as well as the access records.
An empty report is not proof that no access exists. See [JSON and exit codes](output-schema.md).

## Connect GitHub

Create a separate directory so synthetic records are not mixed with a real workspace:

```bash
mkdir company-access
cd company-access
permesh init --organization Acme
permesh provider add github
permesh auth login github-main
permesh doctor
permesh user YOUR_GITHUB_LOGIN
```

Replace `YOUR_GITHUB_LOGIN` with your known account login. Before creating a token,
read [GitHub permissions](providers/github.md).

During `provider add`:

1. Review the official release and agree to its local execution.
2. Enter the organization names to inspect.
3. Choose `keychain://github-main/token` for local credential storage, or an `env://` reference for existing secret tooling.
4. Review and approve the provider instance and its credential references.

The login command above stores a token in the OS keychain. For an environment
reference, supply the value outside Permesh and skip `auth login`. Never paste
actual credentials into setup settings, command arguments or `permesh.yaml`.

Use `--version 0.1.0` on add to select the currently published GitHub release
explicitly; `--id github-other` creates a differently named instance.
Providers run as your user, without a sandbox. Checksums verify catalog bytes;
they do not verify publisher signatures. [Guided setup and automation](provider-setup.md)
explain the review steps and noninteractive answers.

## Link accounts to a person

GitHub public profile emails are not verified evidence. A login query selects a
unique account; an email query needs a verified identity assertion or an explicit
mapping to the account’s immutable numeric ID.

Add mappings using the [identity resolution guide](identity-resolution.md).
Aliases link accounts; they do not establish active lifecycle status. For real
orphan review, configure an authoritative identity source. [Google Directory](providers/google.md)
is implemented as an unpublished source candidate; check [availability](providers.md)
before planning a deployment around it.

## Keep the workspace useful

```bash
permesh provider list
permesh provider status
permesh admins
permesh doctor --details
```

`doctor --details` is a current-source feature that adds actionable diagnostics.
Use `permesh --help`, `permesh provider --help` and `permesh auth --help` for options.
Permesh finds `permesh.yaml` in parent directories; `--config FILE` selects another
workspace explicitly.

Queries do not rewrite configuration or change provider state. Adding providers
rewrites YAML formatting, so review the Git diff. `init` never overwrites an
existing workspace. Share configuration with your team and keep credentials local;
see [team workflows](team-workflows.md) for portable platform pins and approvals.

For existing built-in configurations, use the explicit [GitHub](github-migration.md)
or [Google](google-migration.md) migration. Do not create duplicate instances to
work around migration: account aliases rely on stable instance IDs.

## Live validation

A read-only smoke test against a real GitHub organization passed on macOS Apple Silicon on 2026-09-08 using the then-bundled adapter in a local development build:

- `doctor --json` verified authentication and active organization membership.
- `user <login> --json` returned observed organization and repository access.
- `admins --json` returned privileged account/resource/privilege combinations that matched an independent GitHub API comparison, with no missing or unexpected combinations.
- Commands returned exit code 0, valid JSON, and no provider failures. Configuration remained unchanged, and the credential was absent from captured stdout and stderr.

Organization and account names, resource identifiers, access counts, credentials, and raw reports are omitted. The offline demo continues to use only synthetic fixtures.

This historical bundled-adapter smoke test does not qualify the separate external executable. It is not release qualification or proof of exhaustive effective access. Live team inheritance, pagination, denied permissions, token revocation, and native credential-store behavior still require dedicated acceptance exercises. See the [release gates](releasing.md).

The current packaged CLI and external GitHub candidate also passed read-only
health, privileged-access and account-query acceptance on macOS Apple Silicon.
No credentials or raw access reports were retained. This validates the exercised
paths, not every access mechanism or another provider’s live behavior.
