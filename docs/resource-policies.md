# Resource inspection and local review policies

These unpublished access-review candidate commands run read-only discovery by default. `--snapshot FILE` instead reads an explicitly chosen [saved snapshot](snapshots.md) offline, without a workspace or provider credentials. Neither command saves an artifact or mutates provider access. Saved evidence is historical; policies evaluate that observation window, not current remote authorization.

## Resource-first inspection

```sh
permesh resource
permesh resource 'acme/payments-api'
permesh resource --instance github-work --id IMMUTABLE_RESOURCE_ID
permesh resource --instance github-work --id IMMUTABLE_RESOURCE_ID --snapshot review.json --json
```

Labels match exactly and are never identity keys. An ambiguous label returns all matching stable provider-instance/resource-ID candidates and exit 2; select one with `--instance` and `--id`. An unknown instance is an input error. A missing resource returns exit 1 only with complete collection; incomplete collection retains exit 3/4 rather than asserting absence.

The result includes unmapped accounts, original source grants, direct or nested membership paths, and group grants with no resolved account paths. Group grants are also marked unresolved when their provider's visibility is incomplete, even if some members are visible. Each path retains source certainty and evidence kind; a provider's effective/derived role is never relabeled as a direct observed grant. Resource containment alone does not invent authorization inheritance.

The summary separates unique accounts, path count, and unique source grants. Different group routes remain separate paths but do not multiply permission counts. Resource traversal indexes only the selected resource's grants, reuses the graph engine's 100,000-step/path and 256-group-depth limits, and enforces copy budgets. Resource lists are bounded to 100,000 records; additional account/grant/group output copies have 64 MiB budgets. Limit errors are explicit rather than silently truncated results.

## Policy file

Policies are versioned local JSON data, not programs or provider facts. Unknown fields, duplicate fields/rules/scopes, unsupported values and malformed expiries fail before fresh collection. The shared artifact loader bounds input to 64 MiB and structural decoding limits. There are at most five distinct rules, 256 required providers, 1,000 owners and 1,000 exceptions. Identifiers/owner references are nonempty text of at most 256 UTF-8 bytes; reasons allow 1,024 bytes. Neither permits controls or surrounding whitespace.

```json
{
  "version": 1,
  "rules": [
    "inactive_access",
    "external_privileged",
    "machine_owner",
    "expired_exception",
    "required_coverage"
  ],
  "required_providers": [
    {"instance": "github-work", "capabilities": ["accounts", "grants"]},
    {"instance": "directory", "capabilities": ["identities"]}
  ],
  "owners": [
    {
      "instance": "github-work",
      "account": "IMMUTABLE_MACHINE_ACCOUNT_ID",
      "owner": "security:responsible-owner-001",
      "reason": "Team-approved responsibility for the deployment account"
    }
  ],
  "exceptions": [
    {
      "id": "review-ticket-123",
      "rule": "external_privileged",
      "instance": "github-work",
      "account": "IMMUTABLE_ACCOUNT_ID",
      "resource": "IMMUTABLE_RESOURCE_ID",
      "grant_id": "IMMUTABLE_SOURCE_GRANT_ID",
      "owner": "security:responsible-owner-001",
      "reason": "Approved temporary project access pending scheduled review",
      "expires_at": "2026-10-01T00:00:00Z"
    }
  ]
}
```

`rules` is required and nonempty. Annotation lists default to empty. `required_coverage` requires at least one explicit required provider with nonempty capabilities. Capability names are `accounts`, `identities`, `resources`, `groups`, `memberships` and `grants`. Ownership and exception owners are explicitly recorded organizational assertions; the command does not verify the owner exists or infer responsibility from provider metadata.

```sh
permesh policy check --rules review-policy.json
permesh policy check --rules review-policy.json --snapshot review.json --json
```

The five checks are deliberately narrow:

- `inactive_access`: an explicitly authoritative identity classified `inactive` retains observed/derived access evidence. Missing or conflicting identity evidence is not evaluable. `suspended` is distinct from `inactive` and does not trigger this rule; a pass is not proof that access was revoked or employment remains active.
- `external_privileged`: an external authoritative identity or source-classified external account has a known elevated/admin/owner permission or assignment. Unknown privilege/classification and unsupported evidence remain not evaluable. Native role text is never parsed to guess privilege.
- `machine_owner`: a service/bot identity or account has known privileged permission/assignment evidence and lacks an exact provider-instance/account-ID owner assertion. Human/nonprivileged cases do not trigger it. An owner for another account cannot satisfy the check.
- `expired_exception`: each documented exception expires at its UTC RFC3339 timestamp, with equality considered expired, even if its source grant is no longer observed.
- `required_coverage`: each named instance must have complete captured observations and the listed capabilities. Missing coverage remains a finding and makes the overall evaluation incomplete.

A `pass` applies only to the declared check and observation scope. Unknown privilege never proves low privilege; uncertain/inferred access evidence cannot establish an assignment finding. Unresolved groups produce not-evaluable access checks. Partial observations may establish specific findings while preventing a clean overall result. Access checks require at least one source declaring `grants`, with usable account/grant coverage from each such collector. Complete account/identity inventories without a grant capability are not treated as access collectors. Failed captures still leave coverage unresolved, and an account inventory alone cannot establish complete access coverage. Inactive review separately requires explicit complete identity authority coverage. Explicit `required_coverage` declarations always apply to the named provider and capabilities, including directory sources.

An exception matches exactly one access rule, provider instance, immutable account, immutable resource and source grant. There are no wildcards or label matches. It applies only before its expiry, only to an established finding, and never suppresses uncertainty. An accepted exception produces `excepted` with the original finding state retained. Expired exceptions do not apply. Coverage/expiry rules themselves cannot be excepted. The source grant and account evidence are unchanged.

Reports are deterministic for the same artifact, policy and review time, collapse duplicate paths to one account/grant check per rule, and enforce 100,000-check and 64 MiB encoded-check budgets. Expiry uses the current review time; the underlying saved access evidence remains historical. No recurring scheduler or general expression engine is included.

## Exit contract

Policy uses exit 6 for findings with otherwise complete evaluation. Invalid input uses 2, all-provider failure 3, partial collection or not-evaluable checks 4, internal/output failure 5, and cancellation 130. After errors/cancellation, precedence is 3 > 4 > 6 > 0: findings remain visible even when collection uncertainty wins. A complete result with only accepted exceptions can exit 0, but `clean` stays false and the exceptions remain explicit. Existing inspection commands do not turn observed privileged access into exit 6.
