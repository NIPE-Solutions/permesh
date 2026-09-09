# Read-only offboarding review

Offboarding support on the access-review branch provides assessment, advisory plans,
and repeatable verification. It is not yet a published CLI feature. It performs no
remote mutations and has no `apply` command.

## Assess, plan, verify

Use an exact canonical identity observed in an authoritative source. Labels, account
logins and unverified email guesses do not select a departure target. Conflicting
identity evidence blocks planning. Resolve mappings through the identity workflow
before proceeding.

```sh
permesh offboard assess contractor-7
permesh offboard plan contractor-7 --output departure.plan.json --html departure.html
# Perform any separately approved changes through the appropriate administrative systems.
permesh offboard verify departure.plan.json --html verification.html --json
```

These commands collect fresh read-only observations by default, using the normal
provider trust, approval, credentials, network and collection limits. A partial
provider preserves useful evidence from successful sources while the overall report
remains incomplete. No usable provider observations return exit 3. Input errors
return 2; incomplete reviews return 4; local output failures return 5.

For deliberate offline replay, pass `--snapshot FILE` to any of these commands.
Replay requires no workspace or credentials, is labeled `snapshot_replay`, and never
claims fresh discovery. Default live collection is labeled `fresh_discovery`; this
includes synthetic demo sources and does not imply a real API was queried.

Exit 0 and `complete: true` mean the collection and comparison succeeded within the
reported scope. They do **not** mean offboarding is finished. Assignments can still
be observed in a complete report. Unsupported token, session, invitation, secret-read
history and universal-denial checks remain explicit manual follow-up items.

## Evidence and review steps

Reports show the exact stable provider account IDs associated with the target,
observed direct memberships, assignment paths through groups, native roles,
evidence kind and certainty. A derived path does not establish effective access.
Plans contain only target accounts and relevant paths, not an embedded organization
wide graph. Source summaries retain scope, limitations, timestamps and digests.
Digests reference normalized typed snapshot data, not necessarily the original file
bytes. They establish comparison context, not authenticity of an imported document.

Recommendations are structured advisory data: review a direct assignment, group
membership, ownership role, machine dependency or account lifecycle. Each step has
a stable ID, reason, evidence reference, prerequisite review IDs and verification
method. Ownership and machine-dependency reviews precede potentially disruptive
changes. Imported plans are never interpreted as shell commands or authorization.

Owner privilege yields an ownership review. For a complete GitHub organization
membership collection with observed account owner roles, the report can count
**visible owner accounts** and identify a sole observed owner within that limited
scope. Other ownership models remain unknown. This is not proof of organization-wide
ownership coverage or a guarantee that a transfer is safe.

Machine responsibility requires explicit organizational input; account naming and
email similarity never establish ownership. Pass `--annotations owners.json` when
assessing or planning:

```json
{
  "version": 1,
  "owners": [
    {
      "account": {"provider": "github", "id": "99"},
      "identity": "contractor-7"
    }
  ]
}
```

The scoped account must be an observed service or bot account. This assertion is
stored separately from the person's accounts and clearly labeled organizational
input. It does not become a provider fact. Verification retains the plan assertion
and the original account-observation timestamp for manual revalidation; disappearance
of the machine account does not prove the dependency was resolved. Omitted annotations
do not establish that no ownership dependencies exist.

## Freshness and verification meaning

Plans use a default 24-hour freshness budget; `--expires-in-hours` accepts 1 through
168 hours. Expiry starts at the earliest source collection start or earlier inventory
export time, not when the plan is saved. Already expired evidence cannot create a new
plan with a refreshed lifetime. A plan has an explicit creation time, expiry and
freshness budget. A replayed snapshot older than 24 hours makes assessment incomplete.

Verification revalidates identity authority and mapping context, observed account
bindings, source types, code and configuration digests, declared capabilities,
limitations, source scope, and collection/export chronology. Expired plans, ambiguous
identity, failed authority, changed context, missing sources or partial collection
prevent directional absence conclusions. Newly associated target accounts require
additional review. Valid observations from other sources remain available, while
source-local gaps keep the overall report incomplete. If identity cannot be established,
the assessment is labeled `plan_baseline`; current source outcomes remain in
`verification_sources` and target checks cannot verify.

Checks distinguish:

- `still_observed`: the planned assignment path, membership or account remains visible.
- `no_longer_observed`: the earlier record is absent within sufficiently comparable,
  successful, chronologically ordered visible scope. It does not mean revoked or denied.
- `account_inactive_observed`: an account is explicitly inactive or suspended; this
  does not prove session revocation or removal of its assignments.
- `cannot_verify`: identity, freshness, collection or context prevents a conclusion.

An identical grant ID with changed native role remains an observed assignment; inspect
the current path details for its current role. A vanished inherited path does not prove
that the group-level grant itself was removed. Repeated verification is read-only.

## Local documents and HTML

Plan format `permesh_offboard_plan` version 1 and report format
`permesh_offboard_report` version 1 own their models independently from provider wire
versions. Loaders reject unknown/duplicate fields, unsupported enums, malformed
references, inconsistent recommendation IDs/dependencies and bounded-input violations.
Plans and reports use the snapshot storage reader's 64 MiB document limit and bounded
strings, with tighter target/path and recommendation budgets. This is not a claim that
peak process memory is 64 MiB.

`--json` and `--html FILE` render the same validated report model. HTML is self-contained,
has no scripts, external assets or clickable provider URLs, escapes all labels, and
shows gaps, collection scope, paths, manual review steps and checks. Use browser Find
to inspect resource names and IDs. No redacted or anonymous export is provided.

Output paths must be new. Writes use bounded private temporary files and atomic
no-clobber publication. Unix mode is 0600; Windows inherits the destination directory's
ACLs, so choose a directory restricted to intended readers. A plan and its optional
HTML are separate atomic writes: if HTML fails, an already written plan can remain.
Cancellation is checked before publication. Plans and reports contain sensitive access
metadata; choose destinations and recipients accordingly.

## Synthetic acceptance coverage

The CLI integration fixture contains a departing contractor, a direct grant, a
GitHub team-derived grant, an owner role, an explicitly attributed service account,
and a second provider with incomplete visibility. After replaying a later snapshot,
the direct grant is no longer observed, the inherited path remains observed, and the
incomplete source remains unresolved. The result is not an overall success. Separate
fixtures exercise fresh demo collection, ambiguity, changed scope/authority, expiry,
account suspension, failed connectors, standalone membership and HTML escaping.
These are synthetic host-workflow tests, not live provider qualification.
