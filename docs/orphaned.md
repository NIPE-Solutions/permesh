# Orphaned account review

`permesh orphaned` reviews provider accounts that cannot be confidently associated
with an active authoritative identity. It is read-only and does not disable
accounts, remove grants, or evaluate an employment policy.

```sh
permesh orphaned
permesh orphaned --json
```

Configure at least one explicit authoritative source in `identity.sources`.
`permesh init --demo` provides a synthetic source for offline evaluation.
For a real directory, see [Google Workspace](providers/google.md).
An alias establishes a mapping, not active status; alias-only identities retain
unknown status unless an authoritative source supplies the canonical identity.

## Classifications

| JSON reason | Meaning |
| --- | --- |
| `inactive_identity` | The resolved authoritative identity is inactive, including an inactive service, bot, or external identity. |
| `unknown_identity` | No confident identity match exists in the available complete authority observations. |
| `unknown_status` | An identity was resolved, but its status does not establish active or known service/external status. |
| `ambiguous_identity` | Conflicting or multiple identity candidates remain unresolved. |
| `service_account` | A known service account or service identity, listed separately. |
| `bot` | A known bot, listed separately. |
| `external_identity` | A known external identity, listed separately. |
| `unassessed` | At least one configured authority is missing or incomplete; no orphan classification is made. |

Ambiguity takes precedence over exemptions. Resolved inactivity takes precedence
over service, bot, and external classification. Recognized non-human and external
accounts are listed separately rather than automatically treated as orphaned;
that does not certify ownership or approved access. Ordinary accounts associated
with an active identity are omitted when all authorities completed collection.

Provider kinds and authoritative identity kinds are observations. Google Directory
accounts remain unknown kind; their account state is not proof of employment.
GitHub public email is not verified evidence. Use explicit immutable account
mappings to link GitHub to a directory identity.

## Incomplete observations

If any configured authoritative source fails or returns a partial snapshot,
**every observed account is unassessed**, including ones that appear active or
inactive in another source. Available identity evidence and access paths remain
visible, but there is no definitive orphan finding. This prevents a failed
source from making accounts look orphaned or hiding a possible conflicting
identity. The result identifies the configured authorities and whether all
completed collection.

A failed access provider also makes the overall report incomplete. If authorities
completed, classifications of observed accounts remain available; undiscovered
accounts and grants cannot be assessed. `complete` never guarantees unrestricted
provider visibility or transactional consistency.

## Access and output

Accounts without observed grants are included when they require review. No
observed path is not proof of no access. Paths retain resource, role, privilege,
certainty, memberships, and provenance using the existing access-path schema.
Grants with no observed account path are not classified here: group ownership
and missing membership evidence need separate analysis. `admins` exposes
unresolved privileged group grants.

Human output groups accounts by reason and separates service, bot and external
identities. JSON uses [output schema 1](output-schema.md) with deterministic
account and path ordering. The operation does not write configuration or persist
an access graph.

Exit 0 means inspection completed, even when accounts need review. Exit 2 means
invalid configuration, including no authoritative source. Provider failures
retain exits 3 (all failed) and 4 (partial results). This command is an inspection,
not a policy gate; a future audit policy can decide whether findings should fail CI.
