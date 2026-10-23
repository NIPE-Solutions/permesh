# Starter: weekly access checks in CI

The starter layout gives a small IT team a configuration repository that checks
access every week and opens an issue when something needs review.

```bash
mkdir access-review && cd access-review && git init
permesh init --starter --organization "Acme"
```

This writes the same files as [`examples/starter`](../examples/starter/README.md):

| File | Purpose |
| --- | --- |
| `permesh.yaml` | Providers, identity mappings and optional notifications |
| `ci/providers.txt` | Provider versions CI installs |
| `ci/bootstrap.sh` | Trusts and approves the pinned providers on a fresh runner |
| `policies/review-policy.json` | Review rules for `permesh check` |
| `profiles/`, `people/` | Optional [access profiles](profiles.md); report-only until `profiles.enforce: true` |
| `.github/workflows/permesh-check.yml` | Weekly GitHub Actions run |
| `.github/workflows/permesh-plan.yml` | [Access plans](access-plans.md) for pull requests that change `people/` or `profiles/`, and execution plans after merge |
| `.github/workflows/permesh-verify.yml` | Started by hand with a post-merge run ID; verifies its execution plans |
| `.gitlab-ci.yml` | The weekly check for GitLab pipeline schedules, and `permesh-plan` for merge requests |

Then follow the generated README: connect providers with `provider add --portable`,
list their versions in `ci/providers.txt`, add CI secrets for each `env://`
reference, run `permesh check --out out` once locally, and push.

## How CI stays safe

Use a **private** configuration repository. Full reports are CI artifacts, and on a
public repository anyone signed in can download them. The GitLab pipeline also
limits artifacts to developers (`artifacts: access: developer`).

- CI installs the Permesh release pinned by `PERMESH_VERSION` and
  `PERMESH_ARCHIVE_SHA256`. On GitHub it also verifies the release's build
  provenance with `gh attestation verify`.
- CI installs the official packages and trusts only binaries whose digests match
  the pins committed in `permesh.yaml`; review and approval fail otherwise. Changing
  what CI runs therefore needs a reviewed change to `permesh.yaml`.
- Tokens come from CI secrets. Approval records and tokens are never committed.
- Full reports and snapshots stay as private CI artifacts for 90 days. The issue
  only ever contains the counts-only `summary.md`.
- The previous snapshot is used as a baseline only if that run completed
  collection (exit 0 or 6). On GitHub it must come from this workflow, on the
  default branch, in this repository, so a branch or fork can't plant a baseline
  that hides a new admin. On GitLab, exit 6 is an allowed failure (shown as a
  warning) so that the baseline keeps advancing while findings are open.

## Plans for people changes

`permesh-plan.yml` runs on pull requests that touch `people/` or `profiles/`. It
checks out the base commit with `git worktree`, runs `permesh access review`,
updates one PR comment with the counts-only summary and keeps the plans as the
`access-plans` artifact (30 days). After merge, the same workflow finds the merged
pull request, downloads the plans from its latest successful run, regenerates them
and keeps unchanged ones as `execution-plans` (90 days). If access changed after
review, the job fails with exit 6 and the step summary lists the counts; re-review
before carrying out any step. A push that changes people files without a pull
request fails, because nothing was reviewed.

When the steps are done, start `permesh-verify.yml` from the Actions tab with the
post-merge run ID. It verifies each execution plan, writes counts to the step
summary and fails after all plans ran if any is not done or incomplete.

Pull-request jobs from branches in the same repository receive the provider
secrets, so restrict who can push branches. Pull requests from forks get no
secrets and their plan job fails. If your default branch is not `main`, change
`branches:` in `permesh-plan.yml`.

GitLab ships the merge-request job only; run the post-merge steps by hand as in
[access plans](access-plans.md#pull-request-flow).

The workflows need a Permesh release that includes `permesh check` (and
`permesh access` for the plan workflows). The version is
set by `PERMESH_VERSION` at the top of each workflow file.
