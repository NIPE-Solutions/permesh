# Inspecting privileged access

```bash
permesh admins
permesh admins --json
```

Permesh discovers the configured providers and shows account paths annotated as **elevated**, **admin**, or **owner** by the adapters. It preserves the native role, resource, certainty, and each observed membership edge. A service account or bot with privileged access is included just like a human account.

In the demo workspace, Bob (`bob-admin`, `bob@example.com`) has the native `Admin` role on `acme/infrastructure`. Alice's standard read/write observations are excluded from this report. GitHub organization owners retain native organization-role semantics; custom roles remain unknown unless the adapter can reliably classify them.

## What the sections mean

**Privileged access** lists known privileged paths. A single account or grant may have more than one path; path count is not a count of unique permissions. Account labels remain provider-scoped.

**Unknown privilege** lists observed paths with roles the adapter cannot classify. These may or may not be administrative. Review the original provider role and permissions; absence from the known-privilege section does not establish low privilege.

**Grants without an observed account path** retains group grants with no observed membership path to an account. The group might be empty, membership visibility might be restricted, or data might have changed during collection. Permesh does not decide which explanation is true.

An identity may be resolved, unmapped, or ambiguous. Unmapped and ambiguous accounts retain their access in this report. Exact verified email evidence and explicit immutable-ID aliases still govern correlation; an administrator listing never relaxes matching rules or chooses an ambiguous person silently. A single candidate ID can remain ambiguous if authorities disagree on its kind or status.

## Scope and exit status

This command inspects observations; it does not evaluate policy. Exit 0 means the configured discovery completed within the adapters' documented scope, including when administrators were found. Some failed observations return 4 and all failed providers return 3. A report with no known privileged paths does not prove there are no administrators: inspect unknown privileges, unresolved groups, provider limitations, and completeness.

GitHub visibility depends on token permissions, selected repositories, membership and SSO authorization. Native collaborator roles may already include inheritance; the report does not falsely label them direct. See [GitHub limitations](providers/github.md) and [output schema](output-schema.md).

No authoritative employee directory is required for `admins`. That requirement belongs to a later orphaned-account assessment. There are no mutations, policy-engine calls, local graph persistence, or additional network destinations in this command.
