# Host-owned remote credential reads

Published CLI `0.1.0-alpha.3` supports three explicit resolver types:
`1password_connect`, `vault_kv2`, and `openbao_kv2`. This evaluation feature was
tested with synthetic protocol fixtures; no live Connect, Vault or OpenBao
deployment/version has been qualified. Do not infer OpenBao compatibility from
the Vault label or claim arbitrary Vault-compatible products are supported.

The host performs a read-only GET after executable trust and provider-specific workspace approval. It delivers only the selected string to the named provider credential slot. It never executes `op`, a shell command, a credential helper, or an arbitrary program to resolve a secret. These resolver declarations do not grant permission to modify a remote store.

## Explicit per-instance configuration

```yaml
version: 1
organization:
  name: Example
providers:
  - id: github-work
    type: external
    external:
      provider: github
      sha256: REPLACE_WITH_REVIEWED_EXECUTABLE_SHA256
      configuration:
        orgs: [example]
      credentials:
        token: vault://github-token
      credential_resolvers:
        github-token:
          version: 1
          type: vault_kv2
          origin: https://vault.example.com
          mount: secret
          path: providers/github
          field: token
          secret_version: 7
          bootstrap: env://PERMESH_VAULT_BOOTSTRAP
```

The three reference schemes are `1password://NAME`, `vault://NAME` and `openbao://NAME`. Names are bounded ASCII identifiers local to that provider instance, not URLs or globally shared registrations. A declaration must be referenced by a credential slot and its backend must match the scheme. Unknown fields, duplicate declaration names, explicit nulls, unsupported versions, unused declarations and recursive references fail configuration validation. At most 16 declarations are accepted. Parsing, provider listing and capability inspection do not resolve remote credentials.

A bootstrap is an explicit `env://VARIABLE` or `keychain://INSTANCE/ACCOUNT` reference. The keychain service must equal this provider instance; its account may be a separately provisioned bootstrap entry. No global credential chain, alternate endpoint, inherited token, recursive resolver or automatic fallback is consulted. The bootstrap is sent only to the approved resolver origin. A returned selected field containing the full bootstrap token is rejected before provider delivery; this conservative check does not recognize arbitrary encodings or transformed fragments. Use a separately scoped read token with the least permissions below. Avoid putting a literal token in YAML, shell history or a command argument.

`origin` must be HTTPS with an optional port and no user information, path, query or fragment. Redirects are denied, including redirects within the origin. An HTTP-only Connect deployment needs an explicitly managed HTTPS front end before use here. Ambient proxy environment variables are ignored. Optional resolver `network` uses the same explicit `https_proxy`, `no_proxy` and pinned `ca_bundle: {path, sha256}` format as [provider network configuration](networking.md). Each resolver has its own network declaration; it does not inherit the provider process's network settings. CA files are bounded, confined to the workspace directory, digest checked and parsed before bootstrap resolution. Public CA bytes are not included in review output.

Run the normal workflow with the same workspace selection throughout:

```sh
permesh provider external review github-work
permesh provider external approve github-work --fingerprint REVIEWED_FINGERPRINT --accept-risk
permesh auth status github-work
permesh doctor
```

First register/trust the exact native provider binary using the [external provider workflow](external-providers.md). Review shows the resolver backend, origin, exact locator, selected field, bootstrap reference, requested version and network pins. These nonsecret inputs are included in the existing provider-specific approval fingerprint. Changing any of them requires fresh review and approval before bootstrap resolution. Old configurations omit the new empty declaration map, preserving their serialized approval context.

For remote slots, `auth status` reports `configured`, with availability explicitly **unverified** and no bootstrap or endpoint contact. Its successful completion describes configuration inspection, not authentication. `auth login`/`logout` reject remote delivery slots before reading stdin or changing native storage. Provision the declared bootstrap through its environment or native keychain tooling; these commands do not write or delete remote secrets. Doctor and live collection actually resolve the secret after approval. Failures identify the stage with curated messages without printing endpoint responses or tokens.

## 1Password Connect

Replace the reference with `1password://github-token` and the declaration with:

```yaml
version: 1
type: 1password_connect
origin: https://connect.example.com
vault: abcdefghijklmnopqrstuvwxyz
item: zyxwvutsrqponmlkjihgfedcba
field: password
bootstrap: env://PERMESH_CONNECT_BOOTSTRAP
```

Vault/item identifiers must be exact 26-character IDs; use actual IDs from the deployment, not the example strings. `field` is an exact field ID, including a supported custom field ID, never a label or a fuzzy search. The host issues `GET /v1/vaults/VAULT/items/ITEM` with bearer authentication. **Connect returns the whole item**, including fields unrelated to the selected field. The host verifies the returned vault/item IDs and item version, rejects ambiguous matching field IDs and passes only the requested nonempty string to the provider. It does not enumerate vaults/items or follow references, linked items or sections.

Grant the Connect token read access to only the necessary vault. Connect vault-level permission may expose more items than the one selected in permesh configuration; the exact GET constrains this client's behavior, not the token's server-side powers. There is no item mutation, revision-history retrieval, password generation or generic personal-vault access. The API shape is grounded in the [official Connect API reference](https://www.1password.dev/connect/api-reference); live Connect release qualification remains outstanding.

## Separate Vault and OpenBao KV v2 subsets

For OpenBao, use `openbao://github-token` and `type: openbao_kv2`; other declaration fields are identical. Both adapters issue only `GET /v1/MOUNT/data/PATH`, with `X-Vault-Token`. Optional `secret_version` is a positive integer; omission requests the latest version. The exact returned version must match an explicit requested version. Mount is one component; path consists of explicit components. Traversal, encoded separators, percent escapes, query syntax and fragments are rejected; ordinary allowed characters are encoded once by the host URL builder.

A minimal server policy for the example path is:

```hcl
path "secret/data/providers/github" {
  capabilities = ["read"]
}
```

No `list`, metadata enumeration, write, delete, destroy, lease renewal or token-management permission is needed for these reads. **KV v2 returns the whole secret object at that path**. Only the exact top-level string `field` is selected; nested selectors and nonstring coercion are unsupported. Missing/deleted/destroyed versions, missing/nonstring/empty fields and duplicate selected object keys fail. A future scheduled deletion timestamp is treated as an expiry and checked again before provider invocation; a timestamp at or before the host clock fails. This cannot guarantee the upstream token remains valid after collection begins.

Namespaces, KV v1, dynamic leased secrets, wrapping tokens, login methods, agents and automatic lease/token renewal are unsupported. Nonempty lease IDs, renewable responses and nonzero lease duration are rejected. Use a bootstrap token already issued by an administrator-controlled method. Authentication failures do not trigger alternate credentials or another backend.

Vault and OpenBao have distinct dispatch names and run their response tests separately. The subset follows the [Vault KV v2 API](https://developer.hashicorp.com/vault/api-docs/secret/kv/kv-v2) and [OpenBao KV v2 documentation](https://openbao.org/docs/secrets/kv/kv-v2/). Separate live checks against exact server versions, TLS setup and least-privilege policies are required before labeling either deployment qualified. Supporting one does not qualify the other.

## Bounds, cancellation and data handling

Requests use a 5-second connect limit and a 15-second deadline covering GET, body reading and parsing, within the existing provider preparation deadline. Cancellation drops the HTTP operation; native keychain work already started follows the bounded worker cleanup rules. Responses are limited to 64 KiB even without a Content-Length header, selected secrets and bootstrap values to 16 KiB. Redirect/error response bodies are never parsed or displayed. There is no global cache, persisted resolved value or cross-instance resolver lookup. Repeated operations reread the approved source.

The host owns zeroizing buffers for downloaded body bytes and decoded secret strings, and marks authentication headers sensitive. This does not claim every temporary allocation inside the HTTP/TLS/JSON libraries or operating system is wiped. No resolved value, bootstrap token, raw response or resolver version metadata is inserted into CLI access records or approval storage. Approval fingerprints bind the nonsecret resolver context. Secret store administrators may audit the exact GET requests. Same-user process inspection, administrator privileges, trusted provider code and an explicitly approved TLS-intercepting proxy remain part of the trust boundary.
