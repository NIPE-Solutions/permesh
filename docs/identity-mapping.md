# Explicit identity mapping review

These commands are available on current main after the review-program changes; they are not included in the published `0.1.0-alpha.2` CLI. They run fresh, read-only discovery through the existing provider trust and workspace approval boundaries.

```sh
permesh identity unresolved
permesh identity inspect github-work IMMUTABLE_ACCOUNT_ID
permesh identity map github-work IMMUTABLE_ACCOUNT_ID --identity CANONICAL_ID
```

`unresolved` lists unmapped and ambiguous accounts, verified email evidence, explicit mappings, and observed canonical identities from configured authoritative sources. Use the exact canonical ID and the account's stable provider-instance/native-ID pair. Logins and display labels are never mapping keys. Known configured tenant selectors are shown as configured scope, not independently verified tenant observations; other scopes remain unknown.

`inspect` also shows current mappings for an account no longer observed. An empty account list is not proof of deletion, particularly when a provider failed or reported incomplete visibility.

The first `map` command proposes a change and does not write. Inspect the account/target evidence, exact alias delta, original/proposed configuration digests, and fingerprint. To accept that exact proposal, repeat the same command with its fingerprint:

```sh
permesh identity map github-work IMMUTABLE_ACCOUNT_ID --identity CANONICAL_ID --fingerprint REVIEWED_FINGERPRINT
```

The approval accepts both the mapping and full configuration formatting normalization: comments are removed, while unrelated configuration values are retained. Proposals never print the complete configuration or arbitrary provider settings. A changed file, selected account label/evidence, canonical target, or conflicting evidence requires a new proposal. Fresh collection timestamps are excluded from the fingerprint; the approved run still repeats discovery. The final write uses the existing atomic replacement and captured-revision check, which does not claim protection against a hostile same-user filesystem race.

All configured providers must complete fresh discovery within their advertised scope before a mapping can be proposed or saved. Permanent scope limitations remain visible and are bound into the proposal fingerprint; a complete scoped API result does not claim global visibility. Failed or partial providers remain visible in read-only review results and cannot be treated as evidence of absence. Results are bounded to 100,000 total accounts and authoritative/non-authoritative identity records across the collected snapshots.

A canonical target must exist in an explicitly authoritative source and have unambiguous classification/evidence. Contradictory verified evidence remains ambiguous; mappings never override it. If an account already has a different explicit mapping, remove that exact binding before proposing another. Directory lifecycle state does not establish employment, and service/bot/external classifications remain separate.

To remove a mapping, supply its exact current canonical ID and stable account reference, review the proposal, then repeat with its fingerprint:

```sh
permesh identity unmap github-work IMMUTABLE_ACCOUNT_ID --identity CANONICAL_ID
permesh identity unmap github-work IMMUTABLE_ACCOUNT_ID --identity CANONICAL_ID --fingerprint REVIEWED_FINGERPRINT
```

Removal can clean up an old binding even when its account or canonical target is no longer observed, provided discovery is complete. It does not transfer the binding to a replacement account reusing the old login. Provider-side accounts and permissions are never modified.

Changes use existing `identity.aliases` semantics and invalidate relevant external execution approvals. Review and explicitly approve those provider instances again before querying; these commands never refresh approvals automatically.

JSON results use the existing control envelope with `identity_review_version: 1`; mapping proposals additionally use `proposal_version: 1`. The `applied` flag distinguishes a proposal from a saved local edit. Current-source [pinned local JSON inventories](identity-inventory.md) provide an explicit file-backed source; configuring an inventory does not automatically make it authoritative.
