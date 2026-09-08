# Getting started

Install from a checkout with stable Rust 1.91+:

```bash
cargo install --path crates/permesh-cli --locked
```

For development without installing, use `cargo run -p permesh-cli -- --help`. Installation builds a single `permesh` executable. Build-time dependency downloads are Cargo traffic, not application telemetry.

## Evaluate offline

```bash
mkdir access-demo
cd access-demo
permesh init --demo --organization Example
permesh doctor
permesh user alice@example.com
permesh user alice@example.com --json
permesh admins
```

Alice has write access to the synthetic payments API through `team/backend` and read access to infrastructure through a synthetic account grant. Bob has an administrative grant, a former user is inactive, and a build bot is a service identity. None refers to a real person or resource. Observation timestamps in demo grants are fixed fixture times, not live observations; execution windows use current UTC.

## Live validation

A read-only smoke test against a real GitHub organization passed on macOS Apple Silicon on 2026-09-08 using the then-bundled adapter in a local development build:

- `doctor --json` verified authentication and active organization membership.
- `user <login> --json` returned observed organization and repository access.
- `admins --json` returned privileged account/resource/privilege combinations that matched an independent GitHub API comparison, with no missing or unexpected combinations.
- Commands returned exit code 0, valid JSON, and no provider failures. Configuration remained unchanged, and the credential was absent from captured stdout and stderr.

Organization and account names, resource identifiers, access counts, credentials, and raw reports are omitted. The offline demo continues to use only synthetic fixtures.

This historical bundled-adapter smoke test does not qualify the separate external executable. It is not release qualification or proof of exhaustive effective access. Live team inheritance, pagination, denied permissions, token revocation, and native credential-store behavior still require dedicated acceptance exercises. See the [release gates](releasing.md).

## Connect GitHub

```bash
permesh init --organization Acme
permesh provider install github --version 0.1.0
permesh provider external trust /ABSOLUTE/PATH/TO/INSTALLED/provider \
  --id github --sha256 REVIEWED_EXECUTABLE_SHA256 \
  --capability accounts --capability resources --capability groups \
  --capability memberships --capability grants --accept-risk
permesh provider setup github --id github-main
permesh auth login github-main
permesh provider external review github-main
permesh provider external approve github-main --fingerprint REVIEWED_FINGERPRINT --accept-risk
permesh doctor
permesh user YOUR_GITHUB_LOGIN
```

Replace the executable path and digest with the installed package's reviewed
values, and `REVIEWED_FINGERPRINT` with the
workspace review result. During setup, supply organization names and a token
reference such as `keychain://github-main/token`; `auth login` then stores the token
in the native keychain. For `env://PERMESH_GITHUB_TOKEN`, inject that variable through
existing secret tooling and skip `auth login`. Never put token values in arguments
or configuration.

Downloading does not trust or execute code. Setup requires explicit binary trust;
queries require a separate workspace approval. The catalog advertises only
qualified external releases. See [packages](provider-packages.md)
and the [provider's permissions and visibility documentation](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/github.md).

Existing `type: github` workspaces must use [explicit migration](github-migration.md).
Legacy parsing remains available, but health checks, queries and credential commands
refuse legacy GitHub instances before accessing credentials or the network.

The GitHub adapter cannot prove public profile email ownership. Use `permesh user LOGIN` to select one unique account; explicitly map `alice@example.com` to the account's numeric immutable ID for canonical lookup. Login changes preserve the native identity; configured login-based aliases are not supported.

Run `permesh --help`, `permesh provider --help`, or `permesh auth --help` to discover commands. `--config FILE` selects another workspace. Parent search makes the same config available in nested directories.

No query writes files or changes provider state. `provider add` explicitly rewrites YAML formatting; review its diff. `init` never overwrites an existing file. Do not edit a config simultaneously with `provider add`.

The external packaged GitHub adapter separately passed credential delivery, health
and privileged-access parity against the bundled adapter in a read-only exercise.
No live reports or credentials were retained. This confirms the exercised paths,
not exhaustive visibility of every GitHub access mechanism.
