# Migrate a legacy GitHub instance

`permesh provider migrate INSTANCE --sha256 DIGEST` converts one existing
`type: github` instance to an external GitHub instance. The CLI no longer bundles the GitHub adapter. Migration is required before
executing legacy GitHub instances;
it never downloads, trusts or executes a provider, approves a workspace, or reads
credentials from environment variables or the native keychain.

First obtain and review the external binary using the
[explicit package and trust workflow](provider-packages.md). Its registration must
be named `github` and declare exactly `accounts`, `resources`, `groups`,
`memberships` and `grants`. Use the reviewed executable digest, including a retained
older trusted digest when that is the intended pin. A package download alone is
insufficient. Use a qualified release from the official catalog.

```bash
permesh provider migrate github-work --sha256 REVIEWED_EXECUTABLE_SHA256
permesh provider external review github-work
```

The command preserves the instance ID and its position, organization names and
order, exact token reference, aliases, identity sources and other providers. For
example:

```yaml
- id: github-work
  type: github
  organizations: [acme]
  auth:
    token: keychain://github-work/token
```

becomes:

```yaml
- id: github-work
  type: external
  organizations: []
  external:
    provider: github
    sha256: REVIEWED_EXECUTABLE_SHA256
    configuration:
      organizations: [acme]
    credentials:
      token: keychain://github-work/token
```

Existing `env://` references are also preserved verbatim. No credential copying or
login is needed for this conversion. Review the resulting Git diff: YAML comments
and formatting are normalized. The command rejects symlink/nonregular workspaces
and fails if the captured configuration changed before replacement. It validates
and writes a temporary file in the same directory before replacing the workspace.
These filesystem checks do not provide isolation from a malicious same-user
process racing the final checks and replacement.

**Review execution approvals for every external instance in this workspace.**
Approval fingerprints cover the whole configuration, so this edit invalidates
existing approvals for other external instances too. Approval records are left
untouched; each affected instance needs a fresh `provider external review` and
explicit `approve --fingerprint ... --accept-risk` before queries. Migration never
runs `doctor` or a query automatically.

Migration refuses unknown, non-GitHub or already external instances, untrusted or
tampered binaries, and incompatible capabilities without changing the workspace.
An external ID must start with an ASCII letter and be at most 64 bytes; some legacy
IDs permitted up to 128 bytes or a leading digit. Such IDs require an explicit
manual rename together with their aliases and keychain references. Names are never
silently changed. GitHub supports at most 100 organizations with names at most
100 bytes; larger legacy configurations must be edited explicitly.

The external host adds its own transcript bounds and 60-second operation deadline
(the former bundled invocation allowed 120 seconds). Migration does not establish live
provider visibility or promise identical completion timing. Run health checks and
queries only after reviewing and approving the migrated configuration.

Success uses exit 0 and JSON command `provider_migrate`; invalid configuration,
trust or compatibility uses exit 2, registration verification timeout uses 3,
filesystem write failures use 5 and Ctrl+C uses 130. JSON reports the chosen pin,
workspace path and next review command without including credential references or
values.
