# Joiner, mover and leaver plans

When someone joins, moves to another project or leaves, `permesh access` turns the
change in their [access profiles](profiles.md) into a plan file you can review. A
person carries out each step in the provider; Permesh never changes access. Later,
`permesh access verify` checks what was done. Everything here is
[experimental](stability.md).

The examples use the demo workspace (`permesh init --demo`) with two profiles:

```yaml
# profiles/base.yaml
profile: base
grants:
  - {instance: demo, role: Read, resource: repo-21}
```

```yaml
# profiles/backend.yaml
profile: backend
includes: [base]
grants:
  - {instance: demo, group: team-10}
```

and `people/alice.yaml` and `people/bob.yaml` assigning `backend`.

## Join

Carol is new and has no account yet:

```bash
permesh access join carol@example.com --profile backend --output carol.plan.json
```

```text
Access plan: join for carol@example.com
  3 steps; 0 cannot tell; 1 accounts needed; 0 kept
  - account needed in demo (create or invite) — profiles backend
  - add membership team-10 in demo — profiles backend
  - add role Read on repo-21 in demo — profiles backend
```

Each add step lists the `account_needed` step as a prerequisite. An identity no
provider knows is fine for a join, but a move or leave for it is incomplete
(exit 4, `state: not_found`), so a typo never reads as "nothing to remove". Once her account
exists and is mapped to `carol@example.com` (verified email or an
[identity mapping](identity-mapping.md)), verification can confirm the rest.

## Move

Alice leaves the backend project but keeps the base access:

```bash
permesh access move alice@example.com --remove backend --add base --output alice.plan.json
```

```text
Access plan: move for alice@example.com
  1 steps; 0 cannot tell; 0 accounts needed; 1 kept
  - remove membership team-10 in demo (account 100) — profiles backend
  kept: role Read on repo-21 in demo — because of base
```

Only access that no remaining profile justifies is removed. Access derived through
a group is removed at the membership that grants it. A membership stays, with the
profile that needs it, when a remaining profile names that group, a group it
reaches through nesting, or a role it grants.

## Leave

```bash
permesh access leave bob@example.com --output bob.plan.json --html bob.html
```

```text
Access plan: leave for bob@example.com
  2 steps; 0 cannot tell; 0 accounts needed; 0 kept
  - remove role Admin on repo-21 in demo (account 101, grant grant-3) — unexplained
  - review account 101 lifecycle in demo — profiles backend
```

Every access path is removed. Each step says whether an assigned profile explains
it or it is `unexplained`, which is the leftover access a review is looking for.
`--annotations` adds machine-account dependencies as in
[offboarding](offboarding.md); ownership and machine reviews come first. Use
`permesh offboard` instead when you have no profiles.

## Verify

```bash
permesh access verify bob.plan.json
```

```text
Access verification: leave for bob@example.com
  0 done; 1 not done; 1 cannot verify (1 manual review); 0 extra
  - remove role Admin on repo-21 in demo (account 101, grant grant-3): not done (still observed)
  - review account 101 lifecycle in demo: cannot verify (manual review)
```

An added membership or role is `done` once observed. A removal is `done` only when
it is absent from a complete capture of that provider; with a partial capture it is
`cannot_verify`. Join and move plans also list `extra`: access the after-profiles do
not explain. For a leave, every access path that remains is `extra` and needs action
(exit 6). An expired plan still verifies.

A plan that is structurally invalid or inconsistent (wrong version or bounds, a
step ID that does not match its operation, unknown prerequisites) is refused with
exit 2 before any provider is contacted. This is not a signature: someone who can
edit the file can also recompute the IDs. Keep plans where only reviewers can
change them, and compare an execution plan's SHA-256 with the `index.json` of the
post-merge run that wrote it.

Verification reports `identity_context_changed` when the identity authorities, or
mappings for the plan's accounts, changed since the plan was made. Mapping the
planned person's own new account (a joiner) is expected and is not a gap.

## Plan files

Plans are `permesh_access_plan` version 1 JSON ([output schema](output-schema.md#access-plans)).
They record the digests of the evidence and expire `--expires-in-hours` (1–168,
default 24) after the evidence was captured. Step IDs are the SHA-256 of the
operation, so the same change has the same IDs in every run. Plans contain
sensitive access metadata; keep them out of public places.

A step is `cannot_tell` when its provider was not fully read, or when an account
the person may own is claimed by another identity too. Either makes the plan
incomplete (exit 4).

## Pull-request flow

With the [starter](starter.md), change a `people/` file in a pull request:

1. `access review --base ../base --out plans` compares the people assignments of
   the base checkout with the head. Added profiles become join plans, removed ones
   move plans, a deleted people file is a move with nothing left, and
   `status: left` is a leave. It writes `plans/NNN.json`, `plans/index.json` and
   `plans/summary.md`.
2. CI posts `summary.md` as a PR comment. It holds counts and step types only, no
   names or resources (`--detail` opts in). The full plans are a CI artifact: the
   review copy.
3. After merge, CI regenerates the plans with `--reviewed reviewed`. A plan whose
   step IDs match the reviewed one becomes `NNN.execution.json`, carrying
   `reviewed_plan_sha256`. If access changed in between, no execution plan is
   written for that person; the result lists the added and removed steps and exits 6.
4. A person carries out the execution plan's steps.
5. The verify workflow (or `permesh access verify`) confirms the result, and every
   scheduled [check](check.md) keeps watching.

A profile definition change that changes nobody's assignment produces no plans;
the summary says "Profile definitions changed; run permesh profile check".

On GitLab, the `permesh-plan` job runs for merge requests. Run the post-merge steps
by hand: download the merge request's `plans/` artifact into `reviewed/`, check out
the target branch's previous commit as `../base`, and run
`permesh access review --base ../base --out execution --reviewed reviewed`.

## Exit codes

| Command | 0 | 2 | 3 | 4 | 6 |
| --- | --- | --- | --- | --- | --- |
| `join`, `move`, `leave` | plan written, complete | invalid input, unknown profile, existing output | no provider returned data | plan written, incomplete | — |
| `verify` | all steps done, complete | invalid plan | no provider returned data | incomplete or a step cannot be verified | a step is not done |
| `review` | plans written; all reviewed plans matched | invalid input | no provider returned data | incomplete | re-review required |
