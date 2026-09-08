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

A read-only smoke test against a real GitHub organization passed on macOS Apple Silicon on 2026-09-08 using a local development build:

- `doctor --json` verified authentication and active organization membership.
- `user <login> --json` returned observed organization and repository access.
- `admins --json` returned privileged account/resource/privilege combinations that matched an independent GitHub API comparison, with no missing or unexpected combinations.
- Commands returned exit code 0, valid JSON, and no provider failures. Configuration remained unchanged, and the credential was absent from captured stdout and stderr.

Organization and account names, resource identifiers, access counts, credentials, and raw reports are omitted. The offline demo continues to use only synthetic fixtures.

This is an initial smoke test, not release qualification or proof of exhaustive effective access. Live team inheritance, pagination, denied permissions, token revocation, and native credential-store behavior still require dedicated acceptance exercises. See the [release gates](releasing.md).

## Connect GitHub

In a separate directory, run `permesh init --organization Acme`, then:

```bash
permesh provider add github --id github-main --organization YOUR_ORG
permesh auth login github-main
permesh doctor
permesh provider capabilities github-main
permesh user YOUR_GITHUB_LOGIN
```

Login prompts for a personal access token without echo. This milestone does not implement OAuth/browser login; it never asks for a GitHub password. Read [permissions and visibility limitations](providers/github.md) first. To avoid native credential storage, add the provider with `--token-ref env://PERMESH_GITHUB_TOKEN` and inject that variable using your existing secret tooling. Do not paste tokens into command arguments or config.

The GitHub adapter cannot prove public profile email ownership. Use `permesh user LOGIN` to select one unique account; explicitly map `alice@example.com` to the account's numeric immutable ID for canonical lookup. Login changes preserve the native identity; configured login-based aliases are not supported.

Run `permesh --help`, `permesh provider --help`, or `permesh auth --help` to discover commands. `--config FILE` selects another workspace. Parent search makes the same config available in nested directories.

No query writes files or changes provider state. `provider add` explicitly rewrites YAML formatting; review its diff. `init` never overwrites an existing file. Do not edit a config simultaneously with `provider add`.
