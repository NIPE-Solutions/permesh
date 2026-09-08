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

Download the **[0.1.0-alpha.2 prerelease](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.2)** for macOS, Linux or Windows. Verify its provenance and checksum before extracting; see [installation](docs/installation.md). This evaluation release has [known limitations](docs/releases/0.1.0-alpha.2.md).

The five target archives, dependency inventories and checksum files have signed
GitHub provenance from [the reviewed release run](https://github.com/NIPE-Solutions/permesh/actions/runs/34273757910), built from
[`aad5839`](https://github.com/NIPE-Solutions/permesh/commit/aad58397e139ed755fcf983da8e9f045e7e5a556).
The release includes the verified attestation bundle. Executables still have no
Apple notarization or Windows Authenticode signatures; build provenance does not
replace platform signing. The published alpha.1 assets remain unchanged.

Or build from a checkout with Rust 1.91 or newer:

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
and automation](docs/provider-setup.md) and [packages](docs/provider-packages.md).

Existing `type: github` workspaces must use [explicit migration](docs/github-migration.md).
Legacy parsing remains available, but health checks, queries and credential commands
refuse legacy GitHub instances before accessing credentials or the network.

GitHub discovery covers organization members and owners, repositories, teams,
memberships, and observed roles. Results retain their source and visibility
limitations. A login lookup selects an account; correlating it with an email
requires an explicit mapping or verified identity evidence.

The packaged GitHub executable has also passed a read-only credentialed acceptance
check: health and privileged-access observations matched the bundled adapter.
Only aggregate results were retained. [Validation scope](docs/getting-started.md#live-validation).
Google Workspace directory discovery is available as an [identity source](docs/providers/google.md).
The official provider repository also contains Google with refresh-token and
browser-login support, Cloudflare account-access observations, and an AWS IAM
policy-attachment inventory using named credential references. These source
implementations still need live qualification and publication. Google 0.1.1,
Cloudflare 0.1.0 and AWS 0.1.0 are unpublished drafts, absent from the installable
catalog. GitHub is currently
the only published official provider package. See [provider scope and status](docs/providers.md).

[External native providers](docs/external-providers.md) can be explicitly trusted
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
    type: external
    external:
      provider: github
      sha256: REVIEWED_EXECUTABLE_SHA256
      configuration:
        organizations: [acme]
      credentials:
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
- [GitHub provider](docs/providers/github.md) and [explicit legacy migration](docs/github-migration.md)
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
the catalog lists qualified releases for the supported native platforms.
Google and the offline demo remain bundled; GitHub requires an external binary.
Explicit Google migration is available for a reviewed external binary. Removing
the bundled Google adapter follows external release qualification.
