# Set up Permesh for a small team

This guide sets up Permesh for a team of about ten people who use GitHub and
Google Workspace. At the end, a weekly CI job reports departed people who still
have access, new admins and access nobody's role explains, and every join, move or
leave goes through a reviewed pull request.

Only the one or two administrators who look after access use Permesh. The other
people don't install anything: they appear as small files in a private Git
repository. Permesh only reads access. People carry out every change in GitHub or
Google, and Permesh checks the result.

You need a Permesh release that includes `permesh check` and `permesh access`
(see [installation](installation.md)), admin access to the GitHub organization and
read access to the Google Workspace directory.

## 1. Create the configuration repository

Create a **private** repository: CI keeps full access reports as artifacts, and on
a public repository anyone signed in can download them.

```bash
mkdir access-review && cd access-review && git init
permesh init --starter --organization "Example Ltd"
```

This writes `permesh.yaml`, the CI workflows, a review policy and empty `profiles/`
and `people/` folders ([starter layout](starter.md)).

## 2. Get read-only credentials

CI runs unattended every week, so use credentials that last:

- **GitHub:** a token with read access to the organization
  ([GitHub permissions](providers/github.md)).
- **Google Workspace:** an OAuth client and a refresh token for an administrator
  allowed to read users, with the read-only directory scope
  ([Google permissions](providers/google.md) and the provider's
  [setup guide](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/google.md)).
  An access token expires within an hour, which is too short for CI. Make the OAuth
  app internal to your organization: Google expires refresh tokens of apps in
  testing after seven days.

`permesh.yaml` names each credential by reference, never by value, and the same
file serves your machine and CI. Use environment variables, which work in both
places:

| Secret | Environment variable |
| --- | --- |
| GitHub token | `PERMESH_GITHUB_TOKEN` |
| Google OAuth client secret | `PERMESH_GOOGLE_CLIENT_SECRET` |
| Google refresh token | `PERMESH_GOOGLE_REFRESH_TOKEN` |

Set them in your shell (or your password manager's shell integration) before
running Permesh. Never put the values in the repository.

## 3. Connect GitHub and Google Workspace

```bash
permesh provider add github --portable
permesh provider add google --portable --authoritative
permesh doctor
```

When `provider add` asks for a credential reference, answer with the variables
above: `env://PERMESH_GITHUB_TOKEN` for GitHub; for Google, choose refresh-token
mode, give the OAuth client ID (it isn't secret), then
`env://PERMESH_GOOGLE_CLIENT_SECRET` and `env://PERMESH_GOOGLE_REFRESH_TOKEN`.
`permesh doctor` then checks that both providers answer.

- `--portable` pins the provider for every platform, so CI and your colleague's
  machine run exactly the same binary.
- `--authoritative` makes the Google directory the source of truth for who is
  employed: a suspended or archived Google account counts as departed. GitHub only
  shows access; it can't say whether someone still works for you.

The Google provider reads directory users only, not Google groups or Drive
sharing. Expected access below is therefore described with GitHub teams and
repository roles.

## 4. Link each person's GitHub account

Each person's ID is their Google directory identity, written
`google:CUSTOMER_ID:USER_ID`. It stays the same when their email changes. GitHub
doesn't report verified emails to Permesh, so link each GitHub account once:

```bash
permesh identity unresolved
```

lists GitHub accounts that belong to nobody yet, with the directory identities you
can link them to. For each one, propose the link, check it, then save it with the
fingerprint the proposal printed:

```bash
permesh identity map github-main 987654321 --identity google:C01234567:123456789
permesh identity map github-main 987654321 --identity google:C01234567:123456789 \
  --fingerprint REVIEWED_FINGERPRINT
```

`987654321` is the GitHub account's numeric ID, which survives login renames. For
ten people this takes a few minutes, once. Commit the change to `permesh.yaml`.
Details: [identity mapping](identity-mapping.md).

To look up someone's identity ID by email:

```bash
permesh user anna@example.com --json | jq -r .result.identity.id
```

## 5. Describe the roles

A profile lists what one role should have. Most teams need two to four, for
example everyone, developers and admins. Let Permesh draft one from a team that
already has the right access:

```bash
permesh user anna@example.com --json | jq -r '.result.access[].groups[]? | "\(.key.id) \(.name)"' | sort -u
permesh profile suggest --group github-main:4712 --name developer > profiles/developer.yaml
```

The first command shows the GitHub team IDs Anna is in; `4712` is one of them.
Review the draft before committing: it describes what the team has today, not what
it should have. Finished profiles look like this:

```yaml
# profiles/everyone.yaml
profile: everyone
grants:
  - {instance: github-main, group: "4711"}                    # team everyone
```

```yaml
# profiles/developer.yaml
profile: developer
description: Developers
includes: [everyone]
grants:
  - {instance: github-main, group: "4712"}                    # team developers
  - {instance: github-main, role: Write, resource: "98765"}   # repository api
```

## 6. Add one file per person

```yaml
# people/anna.yaml
identity: google:C01234567:123456789   # anna@example.com
profiles: [developer]
```

That's ten files of two or three lines. The comment is for people reading the
file; Permesh uses only the identity ID. Run `permesh profile check` to see who is
missing expected access and which access no profile explains. Fix the files or
the access until the result is what you expect.

Profile findings are report-only in the weekly check until you set
`profiles: {enforce: true}` in `permesh.yaml`.

## 7. Turn on CI

1. List the provider versions from step 3 in `ci/providers.txt`.
2. Add the three variables from step 2 as repository secrets with the same names.
   The starter workflows already pass them to Permesh.
3. In each workflow file, set `PERMESH_VERSION` and `PERMESH_ARCHIVE_SHA256` (the
   `x86_64-unknown-linux-gnu` archive digest from the release page).
4. Run `permesh check --out out` once locally, then commit and push.

Restrict who can push branches: pull-request jobs from branches in the same
repository receive the provider secrets.

## Day to day

| When | What you do | What Permesh does |
| --- | --- | --- |
| Every week | Read the issue if one opens | Checks everything and keeps one issue updated with counts only |
| Someone joins | Open a pull request adding `people/NAME.yaml` | Comments the plan: accounts to create, teams and roles to add |
| Someone moves | Change `profiles:` in their file | Plans only what to add and what no remaining profile needs |
| Someone leaves | Set `status: left` in their file | Plans the removal of everything they still have |
| After merging | Carry out the steps, then start the verify workflow with the post-merge run ID | Confirms each step is done, or says what is not |

Until the person has a Google account, `people/NAME.yaml` can't name their
identity ID: add the file once their directory account exists. Plans, verification
and the weekly check are described in [access plans](access-plans.md) and
[scheduled checks](check.md). To be told in chat or by email, add notifications to
the `check` section ([notifications](notifications.md)).

## A second administrator

A colleague clones the repository, installs the same provider versions, trusts and
approves them on their machine, and logs in with their own credentials. Approvals
and credentials are never shared through Git. See [team workflows](team-workflows.md).
