# ADR 0036: Second stability declaration

Status: accepted for the unreleased source candidate

## Decision

The formats added after the first declaration (Phases 1–3 of the access
operations program) are stable. They change only with a version bump, a migration
guide and a release that still reads the previous version, as described in
[stability](../stability.md). Additive changes stay allowed.

- **Stable now:** the `check` and `profiles` sections of `permesh.yaml`;
  `profiles/` and `people/` files, including `status`; policy file `version: 2`;
  check report version 1; `profile_check` and `profile_suggest` results version 1;
  the `permesh_access_plan` version 1 file and execution plans; the
  `access_plan`, `access_verify` and `access_review` results and `index.json`
  version 1; and the command names `check`, `profile`, `access` and
  `init --starter`.
- **Still experimental:** `permesh mcp` ([ADR 0035](0035-read-only-mcp-server.md)),
  which is new and built on an MCP revision whose client support is still
  arriving; the provider protocol and SDK; `provider dev`; and the starter
  workflow templates, which belong to the user once written.
- **Human-readable output is not frozen:** terminal text, `summary.md`, HTML views,
  notification wording and `profile suggest` draft comments. The counts-only and
  "missing observation is not proof of absence" promises still hold.
- **Two version markers, added before freezing.** People and profile files accept
  an optional `version: 1`; absent means 1, so hand-written files stay minimal.
  Another value is an input error that says to remove the line or upgrade.
  `index.json` becomes `{"access_review_index_version": 1, "plans": [...]}`.
- **Golden fixtures** in `crates/permesh-cli/tests/fixtures/stable/` pin every
  frozen output: the check report (also with notifications), the webhook body,
  `profile check` and `profile suggest`, the join, move and leave plans and the
  execution plan, and the `access` results and review index. Only times, durations,
  digests, temporary paths and human-readable text are normalized, and their shapes
  are still checked. Frozen inputs (people and profile files, policy version 2 and
  the new `permesh.yaml` sections) are pinned by parsing tests instead.

## Why

Small teams build CI jobs, scripts and reviews on these files and reports. A
format that can change silently makes every upgrade a risk. Freezing them now,
while there are no users, costs nothing, and the version markers make a later
migration detectable instead of guessed. Keeping people files free of required
boilerplate keeps them easy to write by hand.

## Consequences

Stage 2 (applying reviewed plans, [ADR 0028](0028-separate-opt-in-remediation.md))
may now be designed; access plan version 1 is its input. Changing a frozen format
now needs a fixture update reviewed as a breaking change. Live tenant acceptance
is still tracked separately per provider and does not affect these formats.
