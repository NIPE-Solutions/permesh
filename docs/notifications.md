# Check notifications

`permesh check` can send its result to channels your team already uses. A red CI run
is always the baseline signal; notifications are optional and **experimental**
([ADR 0032](adr/0032-explicit-check-notifications.md)).

```yaml
check:
  notifications:
    - id: slack
      type: slack_webhook
      url: env://PERMESH_SLACK_WEBHOOK
      on: [departed_with_access, new_admin]
    - id: teams
      type: teams_webhook
      url: env://PERMESH_TEAMS_WEBHOOK
    - id: siem
      type: webhook
      url: keychain://permesh/siem-hook
      detail: true
    - id: email
      type: email
      smtp_host: smtp.example.com
      smtp_port: 587
      username: permesh@example.com
      password: env://PERMESH_SMTP_PASSWORD
      from: Permesh <permesh@example.com>
      to: [it@example.com]
```

| Field | Meaning |
| --- | --- |
| `type` | `slack_webhook`, `teams_webhook` (plain text), `webhook` (JSON) or `email` |
| `url`, `password` | An `env://` or `keychain://` reference, never the value itself |
| `on` | Send only when one of these counts is nonzero: `departed_with_access`, `new_admin`, `removed_admin`, `no_longer_observed`, `policy_finding`, `incomplete`, `provider_failed`. Empty means send when any count is nonzero. |
| `detail` | `false` (default): counts only. `true`: also logins and resource names. |

## Privacy and safety

- By default, messages contain only counts, completeness and the run link. Turn on
  `detail` only for channels where access details may be stored.
- Webhooks must use `https`; plain `http` is accepted only for `localhost`,
  `127.0.0.1` and `::1`. Redirects are not followed. Proxies are not supported yet.
- Email always uses TLS: implicit TLS on port 465 and required STARTTLS on any other
  port. There is no plaintext fallback.
- Each channel gets one attempt with a 10 second timeout. Failures are listed in the
  report and output with a short reason, never with the URL or secret, and they never
  change the exit code.
- Only `permesh check` sends notifications; other commands never do.

The generic webhook body is
`{"check_report_version":1,"status":…,"complete":…,"counts":{…},"run_url":…}`, plus
`departed`, `changes` and `findings` when `detail` is true.

Issues in the configuration repository are created by the [starter](starter.md)
workflows, not by Permesh itself.
