# Scheduled checks

`permesh check` runs one complete review and is designed to run unattended, for
example weekly in CI. It is an **experimental** command: its report format may
change before the next stability declaration ([stability](stability.md)).

```bash
permesh check --out out
permesh check --out out --previous last/snapshot.json --policy policies/review-policy.json
```

Each run:

1. collects a fresh snapshot from every configured provider, read-only;
2. compares it with `--previous` when given;
3. evaluates review rules from `--policy`, or by default `inactive_access`,
   `external_privileged` and `machine_owner` ([policies](resource-policies.md));
4. writes `snapshot.json`, `check.json` and `summary.md` into `--out`;
5. sends configured [notifications](notifications.md), unless `--no-notify`.

`--out` must be its own directory, not the workspace directory. Existing files there
are only replaced with `--overwrite`. `--run-url URL` adds a link (for example to the
CI run) to the summary and notifications.

## What it reports

| Count | Meaning |
| --- | --- |
| Departed people with access | An authoritative identity is inactive or suspended, or the person's [people file](profiles.md) says `status: left`, and one of their accounts still has access, directly or through groups. Left people have reason `left_in_people_file`; an account is counted once. |
| New admins / Removed admins | Accounts that gained or lost known admin or owner access since `--previous`, including through team or group membership. |
| No longer observed (needs review) | An identity seen last time is missing from a comparable, complete collection. This is **not** treated as a departure. |
| Policy findings | Findings of the selected rules other than `inactive_access`, which is already counted as a departure. |
| Providers that failed | Providers that returned nothing usable. |
| Unexplained access / Missing expected access | Differences from declared [access profiles](profiles.md). Report-only unless `profiles.enforce: true`; "not compared" when no profiles are configured. |

Departures need an authoritative identity source, such as Google Workspace with
`--authoritative`, or at least one people file with `status: left`. Without either,
the report says "Departures not assessed" instead of showing zero.

Without `--previous`, the report says there was nothing to compare with, and the
three change counts read "not compared" (they are left out of `counts` in JSON)
rather than zero. When the
previous snapshot is not comparable (different providers, configuration or capture
order), the comparison is marked **inconclusive** and the run is incomplete. It
never reports "no changes" in that case.

## Exit codes

| Exit | Meaning |
| --- | --- |
| 0 | Complete, nothing to review |
| 6 | Complete, something needs review |
| 4 | Incomplete: a provider was only partly read, or the comparison was inconclusive |
| 3 | No provider returned usable data |
| 2 | Invalid input, such as an unreadable `--previous` or policy file |

Incomplete takes precedence over findings, so a partly read run is never called
clean. Profile findings and an incomplete profile comparison count only with
`profiles.enforce: true`. Notification failures never change the exit code.

## Files

`snapshot.json` and `check.json` contain sensitive access metadata: keep them as
CI artifacts of a **private** repository, never in Git or in a public project. `summary.md` contains counts only and is safe to
post in an issue. Use `snapshot.json` as the next run's `--previous`, but only from
runs that exited 0 or 6, because incomplete runs make poor baselines.

The [starter layout](starter.md) sets all of this up for GitHub Actions and GitLab CI.
