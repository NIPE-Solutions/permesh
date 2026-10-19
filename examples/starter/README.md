# Access review with Permesh

This repository runs a weekly [Permesh](https://github.com/NIPE-Solutions/permesh)
access check: who has access to what, including access inherited through teams and
groups, who left but still has access, and who became an admin.

## Set up (about 15 minutes)

Use a **private repository**: CI artifacts hold the full access report, and on
public repositories anyone signed in can download them.

1. **Connect providers on your machine.** In this directory:

   ```bash
   permesh provider add github --portable
   permesh provider add google --portable --authoritative
   permesh auth login github-main
   permesh doctor
   ```

   `--portable` pins the provider binary for every platform, so CI can verify it.

2. **List the provider versions** you just added in `ci/providers.txt`.
3. **Add CI secrets** for each `env://NAME` reference in `permesh.yaml`
   (for example `PERMESH_GITHUB_TOKEN`). Use read-only, least-privilege tokens.
4. **Pin the Permesh release** in the workflow file: set `PERMESH_VERSION` and
   `PERMESH_ARCHIVE_SHA256` (the `x86_64-unknown-linux-gnu` archive digest from the
   release page). CI refuses to run until both are set.
5. **Try it locally:** `permesh check --out out`. Exit 0 means clean, 6 means
   something needs review, 4 means a provider could not be fully read.
6. **Commit and push.** The workflow in `.github/workflows/permesh-check.yml`
   (or `.gitlab-ci.yml`) runs every Monday and can be started by hand.

## What the scheduled run does

- Installs the pinned Permesh release (digest and, on GitHub, build provenance
  checked) and the providers in `ci/providers.txt`,
  trusting only binaries that match the digests pinned in `permesh.yaml`.
- Compares with the previous run's snapshot, when one is available.
- Turns the run red when something needs review (a warning on GitLab, so the
  baseline still advances), and keeps one open issue labeled
  `permesh-check` updated with a counts-only summary.
- Stores the full report as a CI artifact for 90 days. It contains sensitive access
  metadata; snapshots are never committed.

Edit `policies/review-policy.json` to choose review rules, and the `check` section of
`permesh.yaml` to send summaries to Slack, Teams, a webhook or email.
