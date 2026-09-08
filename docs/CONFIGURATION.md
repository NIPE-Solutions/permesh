# Workspace schema 1

One `permesh.yaml` discovered from the current directory upwards, or explicitly selected by `--config`. Unknown fields and duplicate IDs are errors. No imports, environment interpolation, local overrides, or implicit merges are implemented. `permesh.local.yaml` is ignored by Git but is not loaded; supporting it later needs explicit precedence and diagnostics.

```yaml
version: 1
organization:
  name: Acme
providers:
  - id: github-main
    type: external
    external:
      provider: github
      sha256: REVIEWED_EXECUTABLE_SHA256
      configuration:
        organizations: [acme]
      credentials:
        token: env://PERMESH_GITHUB_TOKEN
identity:
  sources: []
  aliases:
    alice@example.com:
      github-main: ["123456"]
```

Demo configuration contains a `type: demo` provider with no authentication. External GitHub requires nonempty `external.configuration.organizations`, a token reference in `external.credentials.token`, an explicitly trusted SHA-256 pin and workspace approval. Replace the digest placeholder with 64 lowercase hexadecimal characters. Legacy `type: github` is parsed only for [migration](github-migration.md); it cannot execute or access credentials. Provider IDs use ASCII letters, digits, hyphen, underscore; duplicate IDs and references to nonexistent providers fail validation. Aliases target immutable account IDs. Source declarations reference provider instances and must be capability-compatible. The demo provider supplies synthetic identities; Google supplies directory identities when explicitly designated authoritative. GitHub cannot be an authoritative identity source. Google requires `customer_id` (an explicit `C...` customer ID) and `auth.token`, and rejects `organizations`. Other providers reject `customer_id`. See [Google configuration](providers/google.md).

CLI contract: `init [--demo] [--organization NAME]`, `provider add github [--id ID] [--version VERSION] [--answers FILE --accept-risk]`, or `provider install github --version VERSION` with explicit binary trust, `provider setup github --id ID`, `provider migrate ID --sha256 DIGEST`, `provider add google --id ID --customer-id CUSTOMER [--authoritative]`, `provider list/status/capabilities [ID]`, `auth login/status/logout`, `doctor`, `user QUERY [--json]`, `admins [--json]`, `orphaned [--json]`, and `version`. Global `--config`, `--json`, `--color auto|always|never`, `-v` flags. Commands implemented in the milestone are listed by `--help`; future commands are not stubs.

Exit codes: 0 successful operation (including a successful identity query with no grants); 1 query has no matching identity/account; 2 invalid arguments/configuration or ambiguous identity; 3 all requested providers failed; 4 useful but incomplete provider results; 5 internal/output failure; 130 cancelled. Warnings about documented provider visibility are exposed separately from failed collection.

Configuration version 1 is independent of output and provider protocol versions. Access reports use output schema 2; control reports retain schema 1. Parsed-command errors use the corresponding output version, while generic argument errors use schema 1. Setup descriptions use their own schema 1; legacy provider drafts 1–4 remain unchanged. Query results include identity/account resolution, explicit paths, provider status and completeness. See the [output contract](output-schema.md) and [migration](migrations/domain-schema-2.md).

`admins` is an inspection command: discovered privileged access does not itself cause exit 1. Ambiguous identities are retained per account rather than failing this aggregate report. Provider failure exits remain 3/4. See [admins.md](admins.md).

`orphaned` requires at least one source with `authoritative: true`; absent or partial authorities produce unassessed accounts, not definitive orphan claims. See [orphaned review](orphaned.md).

## Approved external instances

Schema 1 also accepts `type: external` referencing an existing local native
registration by provider ID and pinned SHA-256. Configuration cannot specify an
executable, arguments or a shell command.

```yaml
providers:
  - id: internal-main
    type: external
    external:
      provider: my-provider
      sha256: "<reviewed 64-character lowercase SHA-256>"
      configuration:
        endpoint: https://internal.example.com
      credentials:
        token: keychain://internal-main/token
        tenant_key: env://PERMESH_INTERNAL_TENANT_KEY
```

Replace the digest placeholder with the inspected binary digest. Provider and
external instance IDs must begin with an ASCII letter and contain only ASCII
letters, digits, hyphens or underscores, up to 64 bytes. `external.configuration`
is a JSON-compatible map with a 64 KiB encoded limit and maximum depth 16.
`external.credentials` permits at most 16 slots; slot names use the same identifier
syntax. Values must be secret references. Keychain service must match the instance
ID and keychain account must equal the slot name. External instances reject
`organizations`, `customer_id` and `auth`; use the external maps instead.

`provider add external --id ID --provider REGISTERED_ID --sha256 DIGEST` writes
this data. Repeat `--setting KEY=VALUE` for string settings and
`--credential NAME=REF` for secret references. Richer setting types require YAML.
Adding configuration does not approve execution or read credentials.

Run `provider external review ID`, then `provider external approve ID
--fingerprint FINGERPRINT --accept-risk` to approve the reviewed canonical config
path, selected provider configuration, its identity aliases/authority and native
registration. Cloned paths or changes to this context require fresh review.
Unrelated provider and organization edits do not. Previous whole-workspace
approvals require one fresh review; see [ADR 0017](adr/0017-provider-scoped-workspace-approval.md).
`provider external revoke ID` removes that workspace permission. See the
[complete workflow](external-providers.md).

External sources may be explicitly authoritative in `identity.sources` only if
the registered capability set includes `identities`; approval includes the source
selection. No identity authority is inferred from registration or record shape.

[Guided setup](provider-setup.md) adds a registered external instance using typed
questions or a strict answer file. It writes nonsecret settings and credential
references, pins the registered digest, and leaves workspace approval explicit.
