# Permesh

**Know who has access to what.**

Permesh is a local-first CLI for discovering and correlating identities and access across your infrastructure.

```console
$ permesh user alice@example.com
Permesh

Identity
  alice@example.com
  active
  demo · alice-dev

Access

demo
  acme/payments-api
    Write (standard)
    via team/backend
    certainty: observed
```

This excerpt uses synthetic demo data. Real provider results retain observation methods, native roles, identity uncertainty, and visibility limitations.

Instead of checking each admin console by hand, build the access picture locally. Keep provider definitions and explicit identity mappings in Git; each administrator resolves their own credentials. Permesh inspects access metadata and never changes access.

## Quick start

This is a development milestone, **not a qualified public release**. Rust 1.91 or newer is required; no registry package or hosted download is claimed yet.

```bash
cargo install --path crates/permesh-cli --locked
mkdir access
cd access
permesh init --demo
permesh doctor
permesh user alice@example.com
permesh user alice@example.com --json
permesh admins
```

The demo needs no credentials or network. A separate read-only live GitHub smoke test passed for connectivity, user lookup, and privileged-access JSON; observed privileged grants matched an independent GitHub API comparison. See the [anonymized validation note](docs/getting-started.md#live-validation). All demo data remains synthetic.

To connect GitHub in a separate workspace:

```bash
permesh init --organization Acme
permesh provider add github --id github-main --organization acme
permesh auth login github-main
permesh doctor
permesh user alice-dev
```

Use a fine-grained read-only token as described in the [GitHub guide](docs/providers/github.md). A GitHub login lookup does not imply a verified email match. To query a canonical email, add an explicit mapping to the account's immutable numeric ID.

`permesh admins --json` reports observed elevated, admin, and owner access. Unknown roles and grants without an observed account path are shown separately; ambiguous identities stay visible. Finding administrators is a successful inspection, not a failed policy check. [Privileged-access guide](docs/admins.md).

## Provider status

| Provider | This milestone |
| --- | --- |
| Demo | Deterministic synthetic identities, direct and group paths; fully offline |
| GitHub.com | Real read-only REST adapter; members/owners, repositories, teams, memberships and observed roles; mock-tested, initial live smoke test passed; full qualification pending |
| Google Workspace, AWS, Cloudflare | Planned; no placeholder adapters |
| External providers | Draft protocol and Python example; execution deliberately disabled |

GitHub results describe what the token can observe. They do not prove complete effective authorization. See [limitations and required permissions](docs/providers/github.md).

## Privacy

**Permesh has no backend. Access data is fetched directly from the providers you configure and processed locally. Permesh never sends infrastructure data to the project maintainers.**

There is no telemetry implementation, analytics, update check, hosted account, remote configuration, or persistent access database. Query commands do not change shared configuration. JSON reports contain sensitive access metadata; you control where stdout is redirected. [Privacy details](docs/privacy.md).

## Configuration

```yaml
version: 1
organization:
  name: Acme
providers:
  - id: github-main
    type: github
    organizations: [acme]
    auth:
      token: env://PERMESH_GITHUB_TOKEN
identity:
  aliases:
    alice@example.com:
      github-main: ["123456"]
```

Only secret references belong in Git. Native keychain references use `keychain://github-main/token`. Configuration discovery walks up from the current directory, or use `--config FILE`. Unknown fields and duplicate IDs are rejected. [Configuration schema](docs/CONFIGURATION.md) · [Secrets](docs/secrets.md).

## Documentation

[Getting started](docs/getting-started.md) · [Concepts](docs/concepts.md) · [Providers](docs/providers.md) · [Identity resolution](docs/identity-resolution.md) · [Output schema](docs/output-schema.md) · [Troubleshooting](docs/troubleshooting.md) · [Architecture](docs/ARCHITECTURE.md) · [Security](docs/security.md) · [Roadmap](docs/ROADMAP.md)

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and the [provider development guide](docs/provider-development.md). Ordinary tests need no live credentials. Release qualification is tracked in [docs/releasing.md](docs/releasing.md). Report security issues using [SECURITY.md](SECURITY.md).

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. No edition split or commercial feature gates.
