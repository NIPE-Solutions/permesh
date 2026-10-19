# ADR 0033: Access profiles

Status: accepted for the unreleased source candidate (experimental format)

## Decision

A workspace may declare the access people are expected to have. Permesh compares
that declaration with captured access and reports the difference. It never changes
access.

- **Layout.** `profiles/*.yaml` declare named profiles; `people/*.yaml` assign
  profiles to one canonical identity each, named inside the file because identity
  IDs can contain characters that file names cannot. An optional `profiles`
  section in `permesh.yaml` sets `enforce` and maps directory groups to profiles.
  Only `*.yaml` files are read, so READMEs can sit beside them. Directories that
  hold no YAML count as not configured.
- **Entries reference IDs, never labels.** An entry is a group membership
  (`instance`, `group`) or a role on a resource (`instance`, `role`, `resource`),
  matched exactly. `instance` must be a configured provider. Includes are
  expanded and checked for cycles.
- **Missing is claimed only where it was observable.** An expected entry is
  `missing` only when its instance was captured completely with the needed
  capability: `memberships` for group entries; `grants` for role entries, plus
  `memberships` when the instance has groups. Otherwise it is `cannot_tell`. An
  identity with no mapped account in a complete instance is missing with reason
  `no_mapped_account`.
- **Unexplained access keeps its route.** Each observed or derived access path
  that no assigned entry explains is reported with the group route (`via`) and
  certainty. A group entry explains every path through that group.
- **Uncertain identity never assigns access.** Accounts claimed by more than one
  identity are not attributed (`ambiguous_accounts`). People files naming an
  unknown identity are `not_found`; conflicting records are `ambiguous`; both make
  the comparison incomplete. Directory-group members without a resolved identity
  are counted as `unresolved_members`. An unresolved directory mapping turns every
  unexplained entry into `cannot_tell`, because a hidden profile could explain it.
- **Exceptions are policy version 2.** `unexplained_access` is valid only in
  `exceptions`: a membership scope `{instance, account, group}` accepts every path
  from that account through the group; a direct scope `{instance, account,
  resource, grant_id}` accepts one direct grant. Exceptions expire, apply only to
  findings and never to `cannot_tell`. Version 1 documents are unchanged.
- **Report-only by default in `check`.** `permesh check` reports
  `unexplained_access` and `missing_access` counts, but they affect the exit code,
  completeness and default notifications only with `profiles.enforce: true`.
  `permesh profile check` always exits 6 on findings, like `policy check`.
- **Suggestions are drafts.** `permesh profile suggest` proposes entries shared by
  a group's resolved members, with counts as comments, for a person to review.

## Why

Small teams rarely have a written access model, but they know what a role should
have. Declaring that in Git turns "who has access" into "who has access they are
not expected to have", which is the question a review actually asks. Derived
access through nested groups is the access people overlook, so its route is part
of every finding. Claiming "missing" only for complete observations keeps the
project rule that missing observation is never proof of absence.

## Consequences

The configuration gains an optional, experimental section and two directories;
existing workspaces are unchanged. Role names are provider-native text, so a
profile written for one provider does not transfer to another. A broken
directory read hides unexplained access for that run, which is reported as
incomplete rather than clean.
