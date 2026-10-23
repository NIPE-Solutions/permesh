# Access profiles

An access profile says what a role in your organization is expected to have. Permesh
compares profiles with captured access and shows two things per person:

- **missing**: an expected group or role that was not observed;
- **unexplained**: access no assigned profile explains, with the group route that
  grants it.

Profiles are **experimental** ([stability](stability.md),
[ADR 0033](adr/0033-access-profiles.md)). Permesh only reads and reports; it never
grants or removes access.

## Try it with the demo

```bash
permesh init --demo
mkdir profiles people
cat > profiles/backend.yaml <<'EOF'
profile: backend
description: Backend engineers
grants:
  - {instance: demo, group: team-10}                    # team/backend
  - {instance: demo, role: Read, resource: repo-21}     # acme/infrastructure
EOF
printf 'identity: alice@example.com\nprofiles: [backend]\n' > people/alice.yaml
printf 'identity: bob@example.com\nprofiles: [backend]\n' > people/bob.yaml
permesh profile check
```

Alice matches her profile. Bob is not in the backend team and has `Admin` instead
of `Read` on the infrastructure repository:

```text
bob@example.com (compared)
  missing (finding): member of team-10 in demo — not observed
  missing (finding): Read on repo-21 in demo — not observed
  unexplained (finding): Admin on acme/infrastructure (repo-21) in demo, account 101, direct
```

Access through a group shows its route. If Alice held a profile with only the
`Read` role, her write access to `acme/payments-api` would be reported as
unexplained `via team-10`, with certainty `derived`.

## Files

| File | Content |
| --- | --- |
| `profiles/NAME.yaml` | `profile` (letters, digits, `-`, `_`), optional `description`, optional `includes` (other profile names), and `grants` |
| `people/ANY.yaml` | `identity` (canonical identity ID, usually the directory email), `profiles`, and optional `status: active` (default) or `left` |
| `permesh.yaml` | optional `profiles:` section, below |

Each entry in `grants` is either a group membership, `{instance, group}`, or a role
on a resource, `{instance, role, resource}`. `group` and `resource` are immutable
provider IDs as shown by `permesh user --json` or `permesh resource --json`, never
display names. `role` is the provider's native role text, matched exactly. A group
entry also explains every access path that runs through that group.

```yaml
profiles:
  enforce: false        # true: profile findings fail `permesh check`
  directory_groups:     # everyone in this directory group holds these profiles
    - {instance: google, group: "03x8tuzt1abcdef", profiles: [backend]}
```

People assigned through a directory group and a people file get the union of their
profiles. `status: left` marks an explicit departure: `permesh check` counts that
person's remaining access as departed, and `permesh access review` plans a leave.
To turn a change in someone's profiles into a reviewable plan, see
[joiner, mover and leaver plans](access-plans.md). Only `*.yaml` files are read; directories that hold nothing else count as
not configured.

## What "cannot tell" means

Permesh never treats something it did not see as absent. An expected entry is
reported `missing` only when its provider was read completely, including group
memberships. Otherwise the entry is `cannot_tell` and the comparison is incomplete
(exit 4). The same applies to unexplained access when a directory group could not be
resolved: a profile Permesh could not see might explain it.

People files that name an unknown identity are reported as `not_found`, and
identities with conflicting records as `ambiguous`. Both make the comparison
incomplete instead of guessing. Accounts claimed by two identities are counted in
`ambiguous_accounts` and attributed to neither; for each claimant, entries in that
provider become `cannot_tell` (reason `ambiguous_account`) and the comparison is
incomplete, because the unattributed account could hold or hide access. Directory
group members without one identity are counted in `unresolved_members` and also
make the comparison incomplete.

## Draft a profile from a team

```bash
permesh profile suggest --group demo:team-10 --name backend > profiles/backend.yaml
```

```yaml
# Draft from demo group team/backend: 1 members (0 unresolved, excluded). Review before committing.
profile: backend
grants:
  - {"instance":"demo","group":"team-10"}  # team/backend: 1 of 1 members
  - {"instance":"demo","role":"Read","resource":"repo-21"}  # acme/infrastructure: 1 of 1 members
```

By default an entry is included only when every member has it; lower that with
`--min-percent 80`. Members include nested group members whose account resolves to
one identity. The draft describes what people have today, not what they should
have, so review it before committing.

## Accept known exceptions

A [policy file](resource-policies.md) with `version: 2` can accept unexplained access
until a date:

```json
{
  "version": 2,
  "rules": ["expired_exception"],
  "exceptions": [
    {"id": "alice-payments", "rule": "unexplained_access", "instance": "demo",
     "account": "100", "group": "team-10",
     "reason": "Pairing on payments until the migration ends",
     "owner": "it@example.com", "expires_at": "2026-12-31T00:00:00Z"}
  ]
}
```

`group` accepts every path from that account through the group. Instead of `group`,
`resource` and `grant_id` accept one direct grant. Accepted rows are shown as
`excepted` with the exception; expired exceptions no longer apply, and exceptions
never settle `cannot_tell`.

```bash
permesh profile check --policy policies/review-policy.json
permesh profile check --snapshot review.json --json
```

## Scheduled checks

`permesh check` includes the comparison when profiles are configured. Its counts
`unexplained_access` and `missing_access` are **report-only** until
`profiles.enforce: true`: they appear in the report and summary but don't change the
exit code, completeness or notifications without an explicit `on` filter. See
[scheduled checks](check.md) and [notifications](notifications.md).

## Exit codes

`permesh profile check` exits 0 when everyone matches, 6 when there are missing or
unexplained findings, 4 when collection or comparison is incomplete, 3 when no
provider returned data and 2 when no profiles are configured or a file is invalid.
Errors name the file that caused them. `profile suggest` exits 1 for an unknown
group and 4 when the capture was incomplete (the draft is still printed).
