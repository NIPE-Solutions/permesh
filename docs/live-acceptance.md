# Live tenant acceptance

Fixture tests prove behavior against recorded responses, not against real tenant
permissions or API visibility. This runbook checks an official provider against a
real tenant. A provider is labeled "live tested" in `permesh provider list` and the
README only after its run below passes and is recorded.

Run each step yourself with least-privilege credentials for a test organization or
tenant you administer. Never paste tokens, real names, emails, tenant IDs or raw
output into this file; record outcomes and fictional placeholders only.

## Before you start

- Build or install the exact CLI under test and record its version and binary
  SHA-256 (`permesh --version`; `shasum -a 256 "$(command -v permesh)"`).
- Work in a new, empty directory with its own workspace.
- Prepare in the test tenant: one test user in one test team or group, and one
  resource that team can reach. You will add and remove this user during the run.
- Keep each provider's admin console open to compare results.

## GitHub

Credentials: follow [GitHub permissions and setup](providers/github.md). Use a
fine-grained token with read-only organization and repository metadata access.

| # | Command | Compare against the admin console |
| --- | --- | --- |
| G1 | `permesh init --organization Test` then `permesh provider add github` | Setup form shows your organization; trust and approval prompts appear separately |
| G2 | `permesh auth login github-main` then `permesh doctor --details` | Health passes; any reported limitation matches token scope |
| G3 | `permesh user TEST_LOGIN` | Every repository the test user reaches, with team paths shown as `via team/...` |
| G4 | `permesh resource 'ORG/TEST_REPO'` | All members, direct and through teams; outside collaborators included |
| G5 | `permesh admins` | Organization owners and repository admins match |
| G6 | `permesh snapshot create --output before.json` | Completes; scope lists the organization |
| G7 | Remove the test user from the test team in GitHub, then `permesh snapshot create --output after.json` and `permesh diff before.json after.json` | Diff shows the team-derived access as removed, nothing else changed |
| G8 | Re-add the user; `permesh offboard plan TEST_IDENTITY --output plan.json` (needs a [mapping](identity-mapping.md) to a canonical identity) | Plan lists the team membership and any direct grants |
| G9 | Remove the user from the team again; `permesh offboard verify plan.json` | Team path reported gone; anything left is listed as remaining |
| G10 | Revoke the token, then `permesh doctor` | Fails with an actionable, redacted error |

## Google Workspace

Credentials: follow [Google Workspace directory](providers/google.md#permissions-and-authentication).
Use an OAuth client in a test Workspace and an admin account with read-only
directory access.

| # | Command | Compare against the admin console |
| --- | --- | --- |
| W1 | `permesh provider add google --authoritative` | Setup form asks for the customer ID; trust and approval prompts appear separately |
| W2 | `permesh auth login google-main --browser` | Browser opens, consent completes, token is stored in the keychain |
| W3 | `permesh doctor --details` | Health passes |
| W4 | `permesh user TEST_EMAIL` | Identity shows as human / active / internal |
| W5 | Suspend the test user in Google, then `permesh orphaned` | With GitHub also connected and mapped, the user and their remaining GitHub access appear as described in [orphan review](orphaned.md) |
| W6 | `permesh policy check --rules review-policy.json` with `inactive_access` | The suspended user does **not** trigger `inactive_access`; suspended is distinct from inactive, as documented in [policies](resource-policies.md) |
| W7 | Restore the user; revoke the OAuth grant; `permesh doctor` | Fails with an actionable, redacted error |

## Evidence log

Add one entry per run. A run passes when every step matches the console or a
mismatch is explained by a documented visibility limit.

| Date | Provider and version | CLI version | OS | Result | Notes |
| --- | --- | --- | --- | --- | --- |
| alpha.3 release | GitHub 0.2.0 | 0.1.0-alpha.3 | macOS ARM64 | Limited pass: health, status and stable-account JSON queries | See [alpha.3 release notes](releases/0.1.0-alpha.3.md) |
