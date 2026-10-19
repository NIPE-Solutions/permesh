# ADR 0032: Explicit check notifications

Status: accepted for the unreleased source candidate (experimental format)

## Decision

`permesh check` may send its result to destinations listed in an optional
`check.notifications` section of `permesh.yaml`: Slack and Microsoft Teams incoming
webhooks, a generic JSON webhook, and email over SMTP. Nothing else sends
notifications, and inspection commands (`user`, `admins`, `resource`, `snapshot`,
`policy`, `offboard`, …) never contact a notification endpoint.

- **Summary by default.** A message carries counts per finding type, completeness
  and an optional CI run link. Account names, logins, emails and resource names are
  included only when a channel sets `detail: true`.
- **Credentials only by reference.** Webhook URLs and SMTP passwords are
  `env://`, `keychain://` or approved remote references, resolved at send time.
  Resolved values never appear in reports or errors.
- **Transport.** Webhooks require `https`; plain `http` is accepted only for
  loopback hosts. Email requires TLS (STARTTLS or implicit); there is no plaintext
  fallback. Redirects are not followed.
- **One attempt, bounded.** Each channel gets one attempt with a 10 second timeout.
  A failed or skipped delivery is recorded in the report. It never changes the
  access result or the exit code, which are fixed before any delivery starts.
- **Issues live in CI.** Creating or updating an issue in the configuration
  repository is done by the shipped CI workflows using `summary.md` and the forge's
  own CLI and CI token. The Permesh binary needs no forge write credentials.

## Why

Small teams use different channels, and many already treat a red CI run as the
alert. Making notification destinations explicit configuration keeps the
local-first promise: Permesh never chooses a destination and sends nothing by
default (ADR 0002). Counts-only messages keep sensitive access metadata out of
chat and email history unless a team deliberately opts in. Keeping delivery
failures out of the exit code prevents a broken webhook from hiding or faking an
access problem.

## Consequences

The configuration gains an optional, experimental section; existing workspaces
are unchanged. Teams without the starter workflows have no issue channel. Teams
Adaptive Cards and retries are not supported.
