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

Choose the discovery contract that belongs to the reviewed binary. GitHub 0.1.0
uses the legacy default, so omit the selector:

```bash
permesh provider migrate github-work --sha256 REVIEWED_EXECUTABLE_SHA256
permesh provider external review github-work
```

GitHub 0.2.0 uses negotiated discovery and requires the explicit selector:

```bash
permesh provider migrate github-work --sha256 REVIEWED_0_2_0_EXECUTABLE_SHA256 \
  --discovery-protocol negotiated-v1
permesh provider external review github-work
```

Do not add `--discovery-protocol negotiated-v1` for 0.1.0 or omit it for 0.2.0;
migration does not probe the binary or infer its contract. For 0.2.0 catalog
metadata, upgrade the CLI to alpha.3 first. Alpha.2 rejects the additive
`discovery_protocol` field even when an older provider version is requested.
Replacing the CLI does not adopt a new installed provider pin, trust a new
executable or renew workspace approval. Existing pins, trust and legacy approval
records remain stored, but alpha.3 requires one fresh review and explicit approval
before the provider can execute. GitHub 0.2.0 is now available from the public
catalog; installation still does not select it in an existing workspace.

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

**Review execution approval for the migrated instance.** Its provider context
changed, so it needs a fresh `provider external review` and explicit
`approve --fingerprint ... --accept-risk` before queries. Unrelated instances
retain their scoped approvals. Existing approval records are neither removed nor
silently renewed. Migration never runs `doctor` or a query automatically.

The command also supports [Google migration](google-migration.md). It refuses unknown, unsupported or already external instances, untrusted or
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
