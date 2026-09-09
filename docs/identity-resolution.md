# Identity resolution

Permesh never fuzzy-matches display names or guesses that similar logins identify the same person.

Rules in this milestone:

1. Native account identity is `(provider instance, immutable account ID)`.
2. An authoritative source supplies canonical IDs, status/kind and verified email evidence. Only providers explicitly listed with `authoritative: true` contribute canonical records.
3. Exact, case-sensitive verified email equality links accounts to canonical candidates. Public profile addresses do not qualify. There is no automatic Gmail-style punctuation normalization or case folding.
4. Explicit YAML aliases link a canonical identifier to one or more immutable account IDs per instance.
5. If evidence links an account to more than one canonical identity, the query fails as ambiguous. An alias does not override contradictory authoritative evidence. Conflicting authoritative status or kind also remains ambiguous.
6. A unique account login, or `INSTANCE:ACCOUNT_ID`, may select an account. It does not manufacture a canonical identity or verified email. A login shared by multiple instances is ambiguous; use the scoped key.

```yaml
identity:
  aliases:
    alice@example.com:
      github-main: ["123456"]
```

This is a reviewable statement about GitHub's immutable numeric user ID. It survives login renames. GitHub public email is not used for correlation. For an explicit alias with no authoritative record, kind and status remain unknown. An alias that cannot be found may be stale or outside the token's visibility; an empty result does not prove absence of access.

Exact matches are deterministic. Authoritative records with the same canonical ID and compatible kind/status union their verified addresses in stable order. Different canonical IDs sharing an address remain separate candidates. Discovery sorting is independent of provider response order. Input order cannot decide identity ownership.

The demo includes an authoritative synthetic source. [`permesh orphaned`](orphaned.md) reviews inactive, suspended, unmatched and ambiguous accounts while keeping service, bot and external identities distinct. It requires an explicitly configured authoritative source; incomplete authority observations leave accounts unassessed. Current Google Workspace source candidates can supply directory identity evidence through the external-provider workflow; see [provider availability](providers.md) before selecting an installable release. Directory account state does not establish employment.

Google directory identities use `google:CUSTOMER_ID:USER_ID` as their canonical
ID. Their primary address is directory-attested and used for exact lookup; email
renames preserve identity IDs. Explicit cross-provider aliases should name that
canonical ID. Directory account state does not establish employment or human
identity. See [Google identity semantics](providers/google.md#identity-semantics).

Use the [explicit identity mapping commands](identity-mapping.md) to inspect stable
account evidence and review an exact mapping proposal before saving it. These
current-main commands preserve the same conservative matching rules.
