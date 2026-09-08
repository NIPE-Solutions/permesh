# Getting started

Download the [alpha binaries](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.1) and follow the [checksum and installation instructions](installation.md).

Alternatively, install from a checkout with stable Rust 1.91+:

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
permesh provider add github
permesh auth login github-main
permesh doctor
permesh user YOUR_GITHUB_LOGIN
```

Add downloads the newest compatible official package, asks whether to trust its
native code, collects settings and credential references, then asks you to approve
that instance. Authentication remains separate. Use `--version VERSION` to choose
an exact release and `--id ID` to choose an instance name.

During setup, supply organization names and a token reference such as
`keychain://github-main/token`; `auth login` then stores the token in the native
keychain. For `env://PERMESH_GITHUB_TOKEN`, inject that variable through existing
secret tooling and skip `auth login`. Never put token values in arguments or
configuration.

Checksums verify bytes against the public catalog; this does not verify publisher
signatures. Native providers run as your user and are not sandboxed. The workspace
pins the current host's executable digest; the same pin does not automatically
select packages for other operating systems or architectures. See [guided setup
and automation](provider-setup.md) and [packages](provider-packages.md).

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
