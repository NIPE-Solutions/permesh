# Concepts

A provider **type** names an adapter, such as GitHub. An **instance** is a configured connection with a stable local ID, such as `github-main`. Native account IDs are scoped to that instance, so an identical ID in another system cannot collide.

An account is not necessarily a person. Human, external, service, bot, and unknown kinds are separate from active/inactive status. A canonical identity can be an authoritative source record or an explicit configured mapping. Mapping an email-shaped name alone does not prove employment, active status, or mailbox verification.

Access observations link accounts or groups to resources through grants. A path retains each membership edge and the final grant. A nested group path can explain why an account appears; cycle detection prevents malformed membership graphs from hanging traversal. Multiple paths remain distinct.

Privileges retain the native role and separately annotate standard/elevated/admin/owner/unknown. Unknown semantics stay unknown. `observed` means an API or fixture supplied a fact, not that every condition in the provider's authorization engine has been evaluated.

A collection can succeed within its documented scope yet still have provider visibility limits. `complete: false` means some requested observations failed. `complete: true` means collection completed within adapter scope, **not** complete authorization coverage across a tenant. Read the provider limitations alongside every security conclusion.

The in-memory snapshot exists for one command. It is not transactionally consistent across APIs. There is no database, cache, or server. See [domain invariants](DOMAIN_MODEL.md).

`admins` shows known privileged paths while retaining unknown role semantics and unmapped or ambiguous accounts separately. It also retains privileged group grants that cannot yet be connected to a discovered account. It does not evaluate employment policy or change access.
