# Pinned local identity inventories

Current-main CLI builds support an explicitly configured `inventory` provider. It is a host-local JSON data reader, not a native executable: it resolves no credentials, performs no network requests and makes no provider-side changes. This is the intentional exception to external-only production provider execution. Published `0.1.0-alpha.2` CLI builds do not include it.

Create and review an export inside the directory containing `permesh.yaml`. Compute its SHA-256 over the exact file bytes using a local checksum tool, then set the reviewed lowercase digest in configuration. The pin detects changes; it does not authenticate the inventory's author or establish that its assertions are true. Updating an export requires reviewing the new data and explicitly replacing the pin.

```yaml
version: 1
organization:
  name: Example
providers:
  - id: roster
    type: inventory
    inventory:
      path: inventories/roster.json
      sha256: REPLACE_WITH_REVIEWED_64_CHARACTER_LOWERCASE_SHA256
      max_age_seconds: 86400
identity:
  sources:
    - provider: roster
      authoritative: true
```

`path` is relative to the workspace configuration directory, never the shell's working directory. Absolute paths, drive prefixes, backslashes, traversal and symlink components are rejected. The final resolved file must remain within the workspace and be a regular file. Config validation, provider listing and capability inspection validate declarations only; doctor and discovery actually read and verify the file. Inventory providers reject authentication, external executable configuration and unrelated provider settings. The freshness limit defaults to 86,400 seconds and accepts 1 through 604,800 seconds.

Importing data alone does not make it authoritative. The explicit `identity.sources` selection above controls whether its canonical identities are used for correlation. Its only capability is `identities`; it creates no provider accounts, grants or groups. There is no CSV autodetection or header guessing.

## Version 1 JSON format

```json
{
  "version": 1,
  "scope": "engineering-directory",
  "exported_at": "2026-09-09T10:00:00Z",
  "complete": true,
  "identities": [
    {
      "id": "inventory:engineering:person-001",
      "kind": "human",
      "affiliation": "internal",
      "lifecycle": "active"
    },
    {
      "id": "inventory:engineering:service-001",
      "kind": "service",
      "affiliation": "internal",
      "lifecycle": "unknown"
    }
  ]
}
```

Export time must reflect the actual export; the example timestamp will become stale. All fields shown are required. JSON must be UTF-8 and no larger than 8 MiB, with at most 100,000 identities. Unknown or duplicate fields, duplicate canonical IDs, unsupported versions/values and malformed timestamps fail closed with errors that do not reflect file contents. Empty inventories are permitted but supply no canonical mapping targets.

- `scope`: a nonempty declared inventory scope, at most 256 UTF-8 bytes without control characters or surrounding whitespace. This is an exporter assertion, not independently verified tenant coverage.
- `exported_at`: UTC RFC3339 export time. Future timestamps are rejected. Times older than `max_age_seconds` make authority assessment incomplete.
- `complete`: whether the exporter claims the export includes its declared scope. `false` makes authority assessment incomplete even when the export is recent. `true` does not imply organization-wide completeness beyond that scope.
- `id`: a stable canonical ID, unique across this export, at most 256 UTF-8 bytes without controls or surrounding whitespace. Prefer a namespaced immutable source ID. Never reuse it for another person or machine after deletion. The reader cannot independently verify immutability across exports.
- `kind`: `human`, `service`, `bot` or `unknown`.
- `affiliation`: `internal`, `external` or `unknown`.
- `lifecycle`: `active`, `inactive`, `suspended` or `unknown`, representing asserted identity/directory lifecycle only. It is not employment status. Do not translate termination dates into account lifecycle implicitly; unsupported employment fields are rejected.

Email fields are deliberately unsupported. The reader always produces empty verified-email evidence and never promotes an arbitrary email column into a verified identity match. Use the [explicit identity mapping commands](identity-mapping.md) to associate observed immutable provider account IDs with reviewed inventory canonical IDs.

## Reading and reviewing

```sh
permesh provider capabilities roster
permesh doctor
permesh identity unresolved
permesh identity inspect github-work IMMUTABLE_ACCOUNT_ID
permesh identity map github-work IMMUTABLE_ACCOUNT_ID --identity inventory:engineering:person-001
permesh orphaned
```

Mapping commands use fresh observations and their existing proposal/fingerprint workflow. Stale or explicitly incomplete exports retain the records actually read, mark the inventory snapshot incomplete, and make orphaned authority assessment unassessed. Doctor fails with an actionable freshness/completeness message. Invalid, missing or changed files fail collection. None of these cases establish that an omitted account belongs to an unauthorized former employee. Even a fresh complete roster's unknown matches require review, not a provider-side action or an employment conclusion.

The host exposes structured source observation fields for declared scope, normalized export time and the verified content digest. Snapshot capture can retain these fields separately from arbitrary provider messages. File checks and bounded reads reduce accidental misuse; they do not claim protection against a hostile same-user filesystem race between checks and opening the file. A digest-verified read always parses the same bytes that were hashed.
