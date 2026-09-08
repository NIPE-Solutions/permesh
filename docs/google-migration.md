# Migrate a Google directory instance

`permesh provider migrate INSTANCE --sha256 DIGEST` converts one existing
`type: google` instance to the separate external Google provider. Migration is an
explicit local configuration edit: it never downloads, trusts, launches or approves
code and never resolves credentials.

Obtain a qualified Google executable and review it using the [provider trust
workflow](provider-packages.md). Its registration must be named `google` with
exactly `accounts` and `identities`. Downloading a package alone does not establish
trust. Use its reviewed executable digest:

```sh
permesh provider migrate google-main --sha256 REVIEWED_EXECUTABLE_SHA256
permesh provider external review google-main
```

The original instance ID, customer ID, token reference, aliases, authoritative
source declarations and other providers remain unchanged. For example:

```yaml
- id: google-main
  type: google
  customer_id: C01234567
  auth:
    token: keychain://google-main/token
```

becomes:

```yaml
- id: google-main
  type: external
  external:
    provider: google
    sha256: REVIEWED_EXECUTABLE_SHA256
    configuration:
      customer_id: C01234567
      auth_mode: access_token
    credentials:
      token: keychain://google-main/token
```

The identity source remains `provider: google-main` with its original authority
flag. The adapter preserves canonical IDs such as `google:C01234567:123456789`.
Migration does not turn access tokens into refresh tokens or introduce any new
OAuth authorization. Configure refresh-token authentication separately through
the external provider's documented setup when required.

Review the Git diff. YAML formatting/comments are normalized. All external
execution approvals for this workspace require a new review, because approvals
bind the full configuration. Existing approval records are not removed or
silently renewed. Approve the new fingerprints explicitly before querying.

The command refuses missing/wrong registrations, missing identity capability,
tampered executables, incompatible instance IDs, malformed or changed
configuration, symlink workspaces and already external instances. A refusal
leaves the configuration unchanged. Instance IDs must begin with an ASCII letter
and contain at most 64 bytes; incompatible legacy IDs need an explicit rename
with their aliases and credential references before migration.

Successful conversion uses exit 0 and JSON command `provider_migrate` with
`provider: google`. Validation/trust errors use 2, registration timeout 3, local
write errors 5, and cancellation 130. The command reports no credential references
or values. The [shared output contract](output-schema.md#provider-migration)
and [filesystem caveats](github-migration.md) also apply.

The external host has its own bounded transcript and 60-second operation deadline.
A successful migration does not establish directory completeness or live tenant
qualification. Health and query checks follow explicit workspace approval.
