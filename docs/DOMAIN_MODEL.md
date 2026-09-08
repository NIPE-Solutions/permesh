# Domain language and invariants

- `EntityKey`: provider instance ID plus immutable native ID. Names are labels, never graph keys.
- `CanonicalIdentity`: configured or authority-sourced identity, kind (human/external/service/bot/unknown), status (active/inactive/external/service/unknown), and verified emails.
- `ProviderAccount`: native account with login, kind, and only genuinely verified emails.
- `Resource`, `Group`: scoped IDs and display labels.
- `Membership`: account or group member of another group. Nested membership can cycle; query traversal must terminate.
- `AccessGrant`: subject, resource, native role, normalized privilege (standard/elevated/admin/owner/unknown), certainty (observed/inferred/unknown), and source provenance.
- `AccessPath`: account, ordered groups, and grant. Multiple paths remain distinct.
- `Provenance`: provider observation method and UTC RFC3339 time; identifiers already carry provider origin.
- `ProviderSnapshot`: one provider's observations, limitations, and collection status. Cross-provider reads are not transactional.

An alias names a provider instance and immutable account ID, not an unscoped login. Alias configuration can explicitly establish a canonical identity; it does not assert employment status. Exact verified email matching is case-sensitive except that email domain normalization may be added only with tests and a schema decision. Display-name and fuzzy matching are forbidden. A login lookup may select an account if unique; it does not silently merge it into a human identity. An identity query with multiple conflicting canonical candidates fails explicitly.

Records must be unique and references valid inside each provider snapshot before queries run. Provider-scoped relationships cannot jump to another provider. Provider warnings distinguish limited visibility from transport failures. An absent account in a partial provider cannot prove absence of access.

When authorities report the same canonical ID with matching kind and status,
verified emails are combined into a sorted, deduplicated union. Conflicting
kinds or statuses remain ambiguous regardless of provider order. Explicit aliases
do not override a conflicting verified-email identity match. Every membership
and grant requires a nonempty observation method and a valid UTC RFC3339
timestamp.
