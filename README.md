# Permesh

**Know who has access to what.**

Someone leaves the team. A contractor finishes a project. A developer moves from
Payments to Platform. Who still has access, and through which account, team or role?

Direct grants are easy to see in an admin console. The access that lingers is
**inherited**: Alice isn't a member of `payments-api`, but she's in `team/backend`,
which has write access. Permesh collects access metadata from the systems you
connect, links accounts using verified evidence or explicit mappings, and shows
the path behind every grant.

It is a read-only command-line tool. **Your access data stays between your machine
and your providers.** No backend, no account to create, no telemetry.

[Try the demo](#try-it-in-two-minutes) · [Install](#install) ·
[Connect your systems](#connect-your-systems) · [Documentation](docs/README.md)

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

*Actual excerpt from the offline demo; all identities are fictional.* Alice's write
access comes through a team, so removing her from the repository alone would not
remove it.

## Try it in two minutes

The demo runs offline with synthetic data. No signup, token or API connection.

```bash
mkdir access-demo && cd access-demo
permesh init --demo
permesh doctor
permesh user alice@example.com
permesh admins
permesh orphaned
```

You'll see team-inherited access, an administrator, and an inactive identity that
still has access.

## Check someone's access

```bash
permesh user alice@example.com      # everything one person can reach, and why
permesh resource 'acme/payments-api' # everyone who can reach one resource
permesh admins                       # accounts with known administrative access
```

Every result says how it knows: whether a grant was observed or derived through a
group, and which providers could not be fully read. Unknown is never reported as
"no access". Add `--json` for versioned output in scripts and pipelines.
See [identity matching](docs/identity-resolution.md) and [privileged access](docs/admins.md).

## Review access regularly

Run one command on a schedule. It takes a snapshot, compares it with the last run,
applies review rules and writes a report:

```bash
permesh check --out out --previous last/snapshot.json
```

It exits 6 when someone departed but still has access, someone became an admin
(including through a team), or a rule finds something, and 4 when a provider
couldn't be fully read. Summaries can go to Slack, Teams, a webhook or email,
with counts only unless you opt in. `permesh init --starter` sets up a
configuration repository with weekly GitHub Actions or GitLab CI runs. See
[scheduled checks](docs/check.md), [notifications](docs/notifications.md) and the
[starter](docs/starter.md).

Declare what each role should have in `profiles/` and `people/`, and
`permesh profile check` lists access nobody explains, including access through
nested groups, with the route that grants it. `permesh profile suggest` drafts a
profile from an existing team. See [access profiles](docs/profiles.md).

To compare by hand, take a snapshot, take another later, and compare:

```bash
permesh snapshot create --output before.json
permesh snapshot create --output after.json
permesh diff before.json after.json
permesh policy check --rules review-policy.json
```

Policies flag inactive identities that still have access, external accounts with
admin rights, bots without an owner, and providers that weren't fully read.
Exceptions are explicit and expire. See [snapshots](docs/snapshots.md),
[policies](docs/resource-policies.md) and [orphan review](docs/orphaned.md).

## When someone joins, moves or leaves

With [access profiles](docs/profiles.md) in place, turn a change in someone's
profiles into a plan you can review:

```bash
permesh access join carol@example.com --profile backend --output carol.plan.json
permesh access move alice@example.com --remove payments --output alice.plan.json
permesh access leave bob@example.com --output bob.plan.json
# A person carries out the steps, then:
permesh access verify bob.plan.json
```

Moves remove only access that no remaining profile justifies and keep shared
access, saying which profile needs it. Joiners without accounts get "account
needed" steps. In the starter, a pull request that changes `people/` gets a
counts-only plan summary, and after merge an execution plan bound to the reviewed
copy. See [joiner, mover and leaver plans](docs/access-plans.md).

Without profiles, use the departure review:

```bash
permesh offboard assess alice@example.com
permesh offboard plan alice@example.com --output departure.plan.json --html departure.html
# Remove access in each system, then:
permesh offboard verify departure.plan.json --html verification.html
```

The plan lists every account and path, including access through groups. Verification
reports what is gone, what remains and what can't be checked. Permesh never treats
"not seen" as "removed", and it makes no changes itself. See the
[departure workflow](docs/offboarding.md).

## Ask an AI assistant

`permesh mcp` lets an AI assistant on your machine ask the same questions,
read-only, with the same completeness warnings:

```bash
claude mcp add permesh -- permesh mcp --config "$PWD/permesh.yaml"
```

It can prepare plan files but never changes access. See [AI assistants](docs/mcp.md).

## Connect your systems

In a new directory, separate from the demo:

```bash
mkdir company-access && cd company-access
permesh init --organization Acme
permesh provider add github
permesh auth login github-main
permesh doctor
permesh user YOUR_GITHUB_LOGIN
```

`provider add` installs the official provider, shows what it will run, asks for
its settings and asks you to approve it. Tokens go into your OS keychain or come
from environment variables; they are never written to `permesh.yaml`.
See [provider setup](docs/provider-setup.md) and [GitHub permissions](docs/providers/github.md).

| Provider | What it shows | Live tested |
| --- | --- | --- |
| GitHub | Organization owners, repository roles, teams and membership paths | Yes (limited) |
| Google Workspace | Directory identities and lifecycle, as the source of who is employed | Not yet |
| Cloudflare | Account members, groups and scoped role assignments | Not yet |
| AWS IAM | Users, roles, groups and policy attachments | Not yet |
| GitLab | Group and project membership with native access levels | Not yet |
| Microsoft Entra ID | Directory identities, groups and memberships | Not yet |
| AWS Identity Center | Directory identities and permission-set assignments | Not yet |

"Not yet" means the provider is tested against recorded fixtures but not yet
against a real tenant. `permesh provider list` shows the same label. Review
[coverage and limitations](docs/providers.md) before relying on a report. Need an
internal system? [Build your own provider](docs/provider-development.md).

## Install

Download a [prerelease binary for macOS, Linux or Windows](docs/installation.md),
or build from source with Rust 1.91 or newer:

```bash
git clone https://github.com/NIPE-Solutions/permesh.git
cd permesh
cargo install --path crates/permesh-cli --locked
```

Permesh is alpha software; the current release is 0.1.0-alpha.3. See
[release status and upgrade notes](docs/status.md) and what is
[stable](docs/stability.md).

## How it stays safe

- **Read-only.** Permesh inspects and explains access. Changes happen in each
  provider's own tools.
- **Local.** No persistent access database, uploads or hidden update checks. Only
  explicit install and update commands contact GitHub.
- **Reviewable.** One `permesh.yaml` describes providers and identity mappings and
  can be shared in Git; each administrator uses their own credentials.
- **Explicit trust.** Providers are separate programs. You approve each one before
  it runs, and again when it changes.

Exported reports and snapshots contain sensitive access metadata; handle them
accordingly. Read the [privacy guarantee](docs/privacy.md),
[security model](docs/security.md) and [team workflow](docs/team-workflows.md).

## Go deeper

The [documentation index](docs/README.md) has paths for first-time users, team
administrators and provider authors.

- [Getting started](docs/getting-started.md), [configuration](docs/CONFIGURATION.md)
  and [credentials](docs/secrets.md)
- [JSON and exit codes](docs/output-schema.md), [troubleshooting](docs/troubleshooting.md)
  and [shell completion](docs/completion.md)
- [Architecture](docs/ARCHITECTURE.md) and [roadmap](docs/ROADMAP.md)

## Contribute

Help improve a provider, reproduce an API edge case with synthetic fixtures, or
make the docs clearer. Start with [CONTRIBUTING.md](CONTRIBUTING.md).
Report vulnerabilities using [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE). No provider limits, paid feature gates or required cloud service.
