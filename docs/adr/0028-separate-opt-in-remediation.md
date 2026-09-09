# ADR 0028: Keep future remediation separate and explicitly authorized

Status: Proposed future design boundary; no write implementation authorized.

## Context

Permesh currently inspects access without changing provider-side state. A useful
finding or read-only verification is not authorization to suspend an account,
remove membership, revoke a session, transfer ownership or delete data. Those
actions have different impacts and upstream semantics.

## Decision

Any future remediation must use separately declared, opt-in write capabilities,
credentials, trust and approval. Existing inspection commands must not acquire a
hidden write mode. Evaluate a separate executable/package before choosing a
transport or CLI surface; keeping it separate would make installation and
credential boundaries clearer, at the cost of additional release coordination.
This ADR does not choose or implement that package.

A provider-specific operation must declare its exact stable targets, tenant,
preconditions, impact, required permissions and independent verification
contract. A reviewed plan must bind stable canonical/account IDs, relevant
identity and access evidence, and a limited freshness window. Execution must
perform fresh preflight checks and reject stale or contradictory plans.

Guard break-glass accounts, sole owners, the executing identity, service
dependencies and shared groups. Account for the authoritative provisioning
system so synchronization does not recreate the removed account or membership.
Suspension, membership removal, token/session revocation, ownership transfer,
retention and deletion remain separate reviewed actions.

Interactive execution requires approval of a concrete write plan. Noninteractive
execution needs a separately reviewed authorization contract; possession of a
read-only workspace approval is insufficient. Use upstream idempotency where
supported, bounded retries, durable execution evidence and safe resumption after
partial failure. Promise neither cross-provider transactions nor universal
rollback: some effects are irreversible or require a compensating action.

After each attempted change, run independent read-only verification and report
observed success, failure and uncertainty separately. A successful write response
alone is not proof that all access has ended.

## Future acceptance backlog

1. Evaluate package/process isolation and distinct credential/approval storage.
2. Specify one narrowly scoped provider operation and its protected-target rules.
3. Test stale plans, privilege loss, synchronization, retries and partial failure
   using synthetic fixtures before any explicitly authorized test-tenant write.
4. Review interactive and automation authorization separately, including evidence
   retention and redaction, then qualify independent verification.

No remote mutation methods, automatic execution, destructive demos or enrollment
are part of the current access-review program. The [future roadmap](../ROADMAP.md)
tracks this separately from read-only product work.
