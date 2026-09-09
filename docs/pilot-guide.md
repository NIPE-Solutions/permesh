# A small, read-only Permesh pilot

Start with one concrete question: which observed accounts have access to a
selected resource, which accounts have administrative access, or what remains
visible for a departing contractor? Agree on the tenant, provider scope and
reviewers before connecting real systems. A pilot measures usefulness within
that scope; it is not an access certification or a guarantee of offboarding.

## Choose the version deliberately

The published CLI is `0.1.0-alpha.2`; the published GitHub provider is `0.1.0`.
Current source and unpublished provider candidates have additional contracts and
features. Check [release limitations](releases/0.1.0-alpha.2.md),
[provider availability](providers.md) and the [roadmap](ROADMAP.md) before
choosing an exact CLI/provider pair. Do not combine arbitrary candidate binaries.

Identity mapping commands and identity inventory import are source candidates in
this development slice. Their guides do not promise that a published binary contains
them. Evaluate a candidate only after its tests pass and record the exact revision;
see [mapping](identity-mapping.md) and [inventory](identity-inventory.md). [Snapshots and diffs](snapshots.md) and [read-only offboarding](offboarding.md) are
also source candidates; these guides do not change published release availability.

## Start offline

Use a new directory and the fully synthetic demo. It needs no credentials or
network connection:

```sh
mkdir permesh-pilot-demo
cd permesh-pilot-demo
permesh init --demo --organization Example
permesh doctor
permesh user alice@example.com
permesh admins
permesh orphaned
```

These records are fictional. Read provider limitations and completeness alongside
results. An inactive directory account is not an employment decision, and an
empty result does not prove that no access exists.

## Connect one reviewed scope

Follow [getting started](getting-started.md) in a separate real workspace, using
an explicitly selected compatible provider release and its documented least
privileges. Confirm the intended tenant independently. Review the native binary,
workspace settings and credential references before approving execution. Native
providers run as the current user, without a sandbox.

Keep reviewed configuration in the team's normal change-review process and keep
credential values local. Never paste a token into YAML, a command argument, an
issue or a shared report. Each reviewer establishes local trust, credentials and
workspace approval; see [team workspaces](team-workflows.md).

Run `permesh doctor`, then inspect a known account with `permesh user LOGIN` and
compare a small sample with the source system. Record unsupported access paths,
missing permissions and misleading explanations, as well as useful findings.
Resolve an ambiguous account using authoritative evidence and stable account
IDs; a display name or public profile email is insufficient.

## Review a change or contractor departure

For a published-binary pilot, capture a small, private baseline using existing
read-only commands. Have the responsible administrator make any intended change
in the source system under its normal process. Run fresh inspection again and
compare the same stable account and tenant scope. Record exactly what remains
observed and what could not be checked. Suspension, group removal, sessions,
tokens, ownership and retention are different concerns; Permesh does not change
them for you.

The candidate [snapshot workflow](snapshots.md) makes this comparison repeatable:
explicit snapshot creation before and after, followed by offline comparison. Missing data after a partial collection is inconclusive;
absence in a comparable successful collection means no longer observed in that
scope, not proof of universal revocation. Offboarding assessment and verification are source-candidate commands; they are
not available in the published CLI.

## Troubleshoot and close the pilot

| Symptom | Practical next check |
| --- | --- |
| Insufficient API permissions | Compare requested scope with the provider's permission guide and approved tenant; do not assume a healthy token can discover everything. |
| Partial collection | Inspect each provider outcome and limitation; repeat only after resolving the cause. |
| Unresolved or contradictory identity | Inspect stable native IDs, configured authority and verified evidence; do not guess a mapping. |
| Locked credential store | Unlock the local native store or use an explicitly configured environment reference. |
| Missing compatible provider | Check the exact CLI/provider release and supported contract; do not bypass trust or silently switch protocols. |
| Expired token | Replace or refresh it through the configured authentication path, then rerun health and discovery. |

See [troubleshooting](troubleshooting.md) for commands and failure details.
Choose who may read exports and when to delete them. Access metadata can be
sensitive even when no credentials are present. Do not put raw reports into
public issues or shared Git history.

Use the optional [feedback template](pilot-feedback.md) to assess setup
completion, time to a useful finding, repeated use, confusing findings and
maintenance effort. Skip any question. There is no automatic collection or
product telemetry associated with this pilot guide. Sponsorship is optional and
separate; see [supporting Permesh](sponsorship.md).
