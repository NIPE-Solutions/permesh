# Permesh

**Know who has access to what.**

Someone leaves the team. A contractor finishes a project. A repository changes
hands. Who still has access—and through which account, team, or role?

Permesh helps you answer without checking every admin console by hand. It is a
read-only command-line tool that collects access metadata from the systems you
connect, links accounts using verified evidence or explicit mappings, and shows
the paths behind their access.

**Your infrastructure data stays between your machine and your providers.**
Permesh has no backend, no account to create, and no telemetry.

[Try the demo](#try-it-without-credentials) · [Connect GitHub](#connect-github) ·
[Providers](#available-providers) · [Documentation](docs/README.md)

## From “who is this?” to “why do they have access?”

```console
$ permesh user alice@example.com
Permesh

Identity
  alice@example.com
  Identity classification: human / active / internal
  demo · alice-dev
  Account classification: human / active / internal

Access evidence

demo
  acme/payments-api
    Write (standard)
    via team/backend
    evidence: assignment
    grant certainty: observed
    path certainty: derived
```

*Actual excerpt from the current source demo. All identities and resources are
fictional.* Alice’s access comes through a team. That distinction matters when
you are reviewing membership or investigating unexpected access.

## When would I use it?

| You need to answer… | Start with… |
| --- | --- |
| What access does this person have across connected systems? | `permesh user alice@example.com` |
| Which accounts have known administrative access? | `permesh admins` |
| Which accounts need review against our identity directory? | `permesh orphaned` |
| Can I connect, and what needs fixing? | `permesh doctor` |
| How can I process this report in my existing tools? | `permesh user alice@example.com --json` |

Use it for access reviews, investigating lingering accounts, and understanding
inherited roles. Start with one GitHub organization; add provider instances as
your infrastructure grows. Teams share configuration and identity mappings in
Git, while each administrator uses their own credentials.

Permesh shows **observed access evidence**, including provider limitations and
incomplete results. It does not guess identity matches, prove every possible
authorization decision, or remove anyone’s access. Orphan review needs an
explicit authoritative identity source; service accounts, bots, and external
identities are distinguished from inactive identities.

## Try it without credentials

Install a [prerelease binary for macOS, Linux, or Windows](docs/installation.md),
then run:

```bash
mkdir access-demo
cd access-demo
permesh init --demo
permesh doctor
permesh user alice@example.com
permesh admins
permesh orphaned
```

The demo runs entirely offline. Explore team-inherited access, an administrator,
and an inactive identity that still has access. No signup, token, or API
connection is needed.

**Release status:** Permesh is alpha software. The published CLI is
[0.1.0-alpha.2](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.2).
This README describes current `main`, including unreleased identity semantics and
JSON schema 2; published output differs. See the [release notes](docs/releases/0.1.0-alpha.2.md)
and [migration guide](docs/migrations/domain-schema-2.md).

To try the exact source version shown here, use Rust 1.91 or newer:

```bash
git clone https://github.com/NIPE-Solutions/permesh.git
cd permesh
cargo install --path crates/permesh-cli --locked
```

Then run the demo commands above in a new directory. Installation instructions
cover checksums and build provenance; platform signing remains a release limitation.

## Review changes and departures

Current source builds also support an explicit local review workflow:

```bash
permesh snapshot create --output before.json
# Later, collect another observation of the same reviewed scope.
permesh snapshot create --output after.json
permesh diff before.json after.json

permesh offboard plan alice@example.com --output departure.plan.json
permesh offboard verify departure.plan.json --html verification.html
```

Try this with the demo first. Departure plans require an unambiguous canonical
identity; real accounts may need [reviewed mappings](docs/identity-mapping.md).
Permesh records what remains visible and what it cannot verify. These commands
perform no offboarding actions, and a missing observation is never proof of revoked
access. Snapshots and reports stay local and contain sensitive metadata.

Read the [snapshot guide](docs/snapshots.md), [departure workflow](docs/offboarding.md)
and [resource/policy examples](docs/resource-policies.md). These commands are
**unreleased** and are absent from the published `0.1.0-alpha.2` binary.

## Connect GitHub

In a separate directory from the demo:

```bash
mkdir company-access
cd company-access
permesh init --organization Acme
permesh provider add github
permesh auth login github-main
permesh doctor
permesh user YOUR_GITHUB_LOGIN
```

`provider add` guides you through installing the official provider, reviewing its
local execution, choosing organizations, and approving its configuration. Choose
`keychain://github-main/token` during setup to store your token with `auth login`.
For CI or existing secret tooling, choose an environment reference instead.

Start with a GitHub login. To look up an email across providers, add an
[explicit identity mapping](docs/identity-resolution.md) using the immutable
account ID; public GitHub profile emails are not verified identity evidence.
See [GitHub permissions and setup](docs/providers/github.md) before creating a token.

Providers execute locally with your user permissions. The guided flow preserves
explicit trust and credential approval; checksum verification checks catalog
bytes, not publisher signatures. [Setup and automation details](docs/provider-setup.md).

## Available providers

The CLI includes the offline demo and, in current source, a pinned local identity
inventory reader. Service integrations are independently distributed from the [official provider repository](https://github.com/NIPE-Solutions/permesh-providers).

| Provider | What it helps you inspect | Availability |
| --- | --- | --- |
| GitHub | Organization owners, repository roles, teams and membership paths | **Installable:** 0.1.0; 0.2.0 source candidate |
| Google Workspace | Directory identities and lifecycle for identity-authority checks | **Source candidate:** 0.2.0, unpublished |
| Cloudflare | Account members, groups and scoped role assignments | **Source candidate:** 0.2.0, unpublished |
| AWS IAM | Users, roles, groups and policy attachments | **Source candidate:** 0.2.0, unpublished |
| GitLab | Scoped group/project membership and native access levels | **Source candidate:** 0.2.0, unpublished |
| Microsoft Entra ID | Tenant-bound directory identities, groups and membership observations | **Source candidate:** 0.2.0, unpublished |
| AWS Identity Center | Directory identities and scoped permission-set assignments | **Source candidate:** 0.2.0, unpublished |

An implemented provider is not automatically a qualified release. AWS attachments
and Cloudflare assignments do not establish effective privilege. Review
[coverage, limitations, and availability](docs/providers.md) before relying on a report.
Need an internal system? [Build your own provider](docs/provider-development.md)
without changing Permesh core.

## Built for your existing workflow

- **Local processing.** No persistent access database, automatic uploads, or hidden
  update checks. Explicit install/update commands contact public GitHub artifacts.
- **Reviewable configuration.** One `permesh.yaml` describes providers and identity
  mappings. Tokens stay in environment variables or the OS keychain.
- **Useful in a terminal and a pipeline.** Human output explains access paths;
  versioned JSON exposes evidence, limitations, and completeness.
- **Read-only by design.** Inspect and review access; use the provider’s own tools
  to make changes.

```bash
permesh provider status
permesh user alice@example.com --json
permesh provider update github --check
```

Updates are explicit and do not silently change a workspace’s selected provider.
Exported reports contain sensitive access metadata; handle them accordingly.
Read the [privacy guarantee](docs/privacy.md) and [team workflow](docs/team-workflows.md).

## Go deeper

[Documentation index](docs/README.md) includes paths for first-time users,
team administrators, and provider authors.

- [Getting started](docs/getting-started.md) — install, explore, connect, query.
- [Configuration](docs/CONFIGURATION.md) and [credentials](docs/secrets.md).
- [Identity matching](docs/identity-resolution.md), [orphan review](docs/orphaned.md),
  and [privileged access](docs/admins.md).
- [JSON and exit codes](docs/output-schema.md), [troubleshooting](docs/troubleshooting.md),
  and [shell completion](docs/completion.md).
- [Architecture](docs/ARCHITECTURE.md), [security](docs/security.md), and [roadmap](docs/ROADMAP.md).

## Contribute

Help improve a provider, reproduce an API edge case with synthetic fixtures, or
make the docs clearer. Start with [CONTRIBUTING.md](CONTRIBUTING.md).
Report vulnerabilities using [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE). No provider limits, paid feature gates, or required cloud service.
