# Workspace schema 1

One `permesh.yaml` discovered from the current directory upwards, or explicitly selected by `--config`. Unknown fields and duplicate IDs are errors. No imports, environment interpolation, local overrides, or implicit merges are implemented. `permesh.local.yaml` is ignored by Git but is not loaded; supporting it later needs explicit precedence and diagnostics.

```yaml
version: 1
organization:
  name: Acme
providers:
  - id: github-main
    type: github
    organizations: [acme]
    auth:
      token: env://PERMESH_GITHUB_TOKEN
identity:
  sources: []
  aliases:
    alice@example.com:
      github-main: ["123456"]
```

Demo configuration contains a `type: demo` provider with no authentication. GitHub requires nonempty organization names and an auth token reference. Provider IDs use ASCII letters, digits, hyphen, underscore; duplicate IDs and references to nonexistent providers fail validation. Aliases target immutable account IDs. Source declarations reference provider instances and must be capability-compatible. The demo provider supplies synthetic identities; Google supplies directory identities when explicitly designated authoritative. GitHub cannot be an authoritative identity source. Google requires `customer_id` (an explicit `C...` customer ID) and `auth.token`, and rejects `organizations`. Other providers reject `customer_id`. See [Google configuration](providers/google.md).

CLI contract: `init [--demo] [--organization NAME]`, `provider add github --id ID --organization ORG`, `provider add google --id ID --customer-id CUSTOMER [--authoritative]`, `provider list/status/capabilities [ID]`, `auth login/status/logout`, `doctor`, `user QUERY [--json]`, `admins [--json]`, `orphaned [--json]`, and `version`. Global `--config`, `--json`, `--color auto|always|never`, `-v` flags. Commands implemented in the milestone are listed by `--help`; future commands are not stubs.

Exit codes: 0 successful operation (including a successful identity query with no grants); 1 query has no matching identity/account; 2 invalid arguments/configuration or ambiguous identity; 3 all requested providers failed; 4 useful but incomplete provider results; 5 internal/output failure; 130 cancelled. Warnings about documented provider visibility are exposed separately from failed collection.

Output schema version 1 is distinct from configuration version 1 and protocol version 1. Errors use `{schema_version: 1, error: {code, message}}`. Query results include identity/account resolution, explicit paths, provider status, and completeness. Exact schema is documented with the executable implementation.

`admins` is an inspection command: discovered privileged access does not itself cause exit 1. Ambiguous identities are retained per account rather than failing this aggregate report. Provider failure exits remain 3/4. See [admins.md](admins.md).

`orphaned` requires at least one source with `authoritative: true`; absent or partial authorities produce unassessed accounts, not definitive orphan claims. See [orphaned review](orphaned.md).
