# ADR 0034: Joiner, mover and leaver access plans

Status: accepted for the unreleased source candidate; format stable since [ADR 0036](0036-second-stability-declaration.md)

## Decision

A change in someone's [access profiles](../profiles.md) becomes a reviewable plan
file, `permesh_access_plan` version 1, that a person carries out and Permesh
verifies afterwards. Plans are data and are never interpreted as commands; Permesh
makes no provider changes ([ADR 0028](0028-separate-opt-in-remediation.md)).

- **Commands.** `permesh access join PERSON --profile P…`, `access move PERSON
  --remove P… [--add Q…]`, `access leave PERSON [--annotations FILE]`, `access verify
  PLAN` and `access review --base DIR --out DIR [--reviewed DIR]`. They are
  experimental. `offboard` is unchanged.
- **After and removed profiles.** "After" is the person's current assignment
  (people file plus directory groups) with added profiles and without removed ones.
  Unknown profile names are an input error. Removing a profile the person does not
  hold is allowed. An unresolved directory assignment makes the plan incomplete.
- **Operation-shaped steps.** Each step is one operation on immutable provider IDs:
  `account_needed`, `add_membership`, `add_role`, `remove_membership`,
  `remove_role`, `review_account_lifecycle`, `review_ownership` and
  `review_machine_dependency`. Names appear only in display output.
- **Join.** Each unsatisfied entry of an after-profile becomes an add step. An
  instance where the person has no attributed account gets one `account_needed`
  step, which the instance's add steps list as a prerequisite. An identity unknown
  to every provider is not an error.
- **Move.** Only access justified by a removed profile and by no after-profile is
  removed. Removal acts on the first membership hop of each route: a direct
  membership is kept, with `because`, when a remaining profile names that group,
  a group it reaches through nesting, or a role reached through it. Direct grants
  of a removed role become `remove_role`. Entries that an after-profile also holds
  are listed under `kept`.
- **Leave.** Every observed or derived access path is removed at its first hop,
  plus direct memberships that grant nothing observed, and every attributed account
  gets `review_account_lifecycle`. Each step says which assigned profiles explain it
  or that it is `unexplained`. Ownership and machine-dependency reviews come from
  the offboarding assessment and are prerequisites of every other step.
- **Missing observation is never proof of absence.** A step in an instance that
  was not captured completely with the needed capability is `cannot_tell`; so are
  steps where an account the person may own is claimed by someone else. Either
  makes the plan incomplete (exit 4).
- **Stable step IDs.** A step ID is the SHA-256 of its operation's JSON, so the same
  operation has the same ID in every run.
- **Bound to evidence.** A plan records the digests of the snapshot, the identity
  context and the profile definitions, and expires 1–168 hours (default 24) after
  the evidence started, as offboarding plans do. Validation refuses wrong versions,
  bounds, step IDs that do not match their operation, duplicate IDs and unknown
  prerequisites before any provider is contacted. It checks consistency, not
  authenticity: plans are not signed, so their integrity rests on where they are
  stored and on the digests in the post-merge `index.json`.
- **Verification.** An add step is `done` once observed. A removal is `done` only
  when absent from a complete capture of its instance, `not_done` while observed,
  and `cannot_verify` otherwise. Reviews are manual. Join and move also list
  `extra`: access unexplained by the after-profiles; for a leave, every remaining
  path is `extra` and counts as needing action. An expired plan still verifies.
  Only changes to authorities or to mappings of the plan's accounts are an
  identity-context gap; a joiner's own new mapping is expected. Exit 0 when
  complete and done, 6 when a step is not done or access remains after a leave,
  4 when incomplete. A move or leave for an identity no provider knows is
  incomplete.
- **Pull-request binding.** `access review` compares the people assignments of a
  base and a head checkout and plans each changed identity. After merge,
  `--reviewed` matches each regenerated plan to the reviewed one by identity and
  kind: equal step-ID sets produce an execution plan carrying
  `reviewed_plan_sha256` (the SHA-256 of the reviewed file); any difference lists
  added and removed steps and exits 6. Summaries carry counts and step types only,
  unless `--detail`.
- **People `status: left`.** People files accept `status: active | left`. A `left`
  identity counts as departed in `permesh check` (`left_in_people_file`) and
  becomes a leave plan in `access review`.

## Why

The access people overlook after a move or departure is derived access. Planning
removals at the membership that grants them, and keeping what another profile still
justifies, turns a profile change into the smallest correct set of changes.
Binding the execution plan to the reviewed copy's operations means a slow review
cannot quietly become a different change. Operation-shaped steps are the input a
later, separately approved apply stage needs, without a format change.

## Consequences

Two leave workflows exist: `offboard` for a departure review without profiles, and
`access leave` for profile-aware plans. Profile definition changes alone produce
no plans; the summary says to run `permesh profile check`. GitLab pipelines plan
merge requests; the post-merge steps run by hand. Plans and execution plans contain
sensitive access metadata and stay private CI artifacts.
