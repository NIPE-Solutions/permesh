# Permesh

**Know who has access to what.**

Permesh is a local-first CLI for inspecting access across your infrastructure.
It connects to your providers, correlates accounts, and shows the roles and
membership paths behind their access.

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

*Example from the offline demo. All names and access records are synthetic.*

Permesh is read-only. Provider configuration and identity mappings can live in
Git; credentials stay in your environment or OS keychain. Results are processed
locally and discarded when the command exits.

## Try it

Permesh is under development. Build from source with Rust 1.91 or newer:

```bash
cargo install --path crates/permesh-cli --locked
mkdir access-demo
cd access-demo
permesh init --demo
permesh doctor
permesh user alice@example.com
permesh admins
permesh orphaned
```

The demo needs no network or credentials. Add `--json` for structured output:

```bash
permesh user alice@example.com --json
permesh admins --json
```

Shell completions for Bash, Zsh, Fish, PowerShell and Elvish are available through
`permesh completion <shell>`. See [setup instructions](docs/completion.md).

## Connect GitHub

In a separate directory:

```bash
permesh init --organization Acme
permesh provider add github --id github-main --organization acme
permesh auth login github-main
permesh doctor
permesh user alice-dev
```

Use a token with the [documented read permissions](docs/providers/github.md).
`auth login` stores it in the native OS keychain. Environment-variable references
are also supported.

GitHub discovery covers organization members and owners, repositories, teams,
memberships, and observed roles. Results retain their source and visibility
limitations. A login lookup selects an account; correlating it with an email
requires an explicit mapping or verified identity evidence.

The GitHub adapter has passed a [live validation exercise](docs/getting-started.md#live-validation).
Google Workspace directory discovery is available as an [identity source](docs/providers/google.md)
with externally supplied OAuth access tokens; live tenant qualification is pending.
AWS and Cloudflare are planned. [External native providers](docs/external-providers.md) can be explicitly trusted
and, after separate workspace approval, used for access queries and health checks
with reviewed named credential references. [Guided setup](docs/provider-setup.md)
uses provider-declared questions and keeps answers inside the CLI. Native code is
not sandboxed.

## Configuration

A workspace uses one `permesh.yaml` file:

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

Aliases reference immutable provider account IDs. Only secret references belong
in the file. Permesh searches parent directories for the workspace; `--config FILE`
selects one explicitly.

## Privacy

**Permesh has no backend. Access data is fetched directly from the providers you
configure and processed locally. Permesh never sends infrastructure data to the
project maintainers.**

There is no telemetry, update check, or persistent access database. Exported JSON
contains access metadata; keep it somewhere appropriate for your organization.
[Privacy details](docs/privacy.md).

## Documentation

- [Getting started](docs/getting-started.md)
- [Configuration](docs/CONFIGURATION.md) and [credentials](docs/secrets.md)
- [GitHub provider](docs/providers/github.md)
- [Identity resolution](docs/identity-resolution.md), [privileged access](docs/admins.md), and [orphaned accounts](docs/orphaned.md)
- [JSON schema and exit codes](docs/output-schema.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Architecture](docs/ARCHITECTURE.md) and [roadmap](docs/ROADMAP.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks, and the
[provider guide](docs/provider-development.md) for adapter development.
Report vulnerabilities through the process in [SECURITY.md](SECURITY.md).
[Release qualification](docs/releasing.md) tracks tested platforms and remaining gates.

## License

[MIT](LICENSE).

Official provider packages are maintained in
[permesh-providers](https://github.com/NIPE-Solutions/permesh-providers).
[Explicit install and update commands](docs/provider-packages.md) are implemented;
the catalog remains empty until the first external release is qualified. Bundled
providers remain available during migration.
