# Security

Permesh reads provider access metadata. It does not change accounts, roles, sessions, memberships or grants. Prefer narrowly scoped read-only tokens and revoke them using the provider's own controls.

Shared YAML stores references, never credential values. Environment references are appropriate for CI; native OS credential stores are appropriate for interactive local use. `auth login` and `auth logout` mutate only the selected local credential entry and require an explicit command. Querying never stores credentials.

No workspace configuration can enable plugins, shell commands, executable secret resolution, arbitrary API endpoints, YAML includes, or remote configuration in this milestone. GitHub.com is the fixed HTTPS origin; redirects are disabled. Review a cloned config nevertheless: it selects which provider organizations are queried and which named credential reference is resolved.

Errors are curated, secret wrappers redact Debug and zeroize on drop, and terminal controls from provider data are escaped. No raw HTTP logging is implemented, even under `-vv`. These choices reduce accidental exposure; they do not protect against a compromised OS, debugger, provider, or malicious executable dependency.

Configuration reads have size, nesting, node and expansion limits. Nonregular files are rejected before opening; a process capable of racing file replacement is outside the safe-workspace assumption. Init uses create-new semantics and restrictive Unix permissions. Explicit provider add writes an atomic temporary replacement and refuses a symlink target; simultaneous editing is not supported. Windows uses native inherited ACLs, which administrators should review.

JSON reports can expose account names, infrastructure names, access relationships, and immutable IDs. Protect redirected files with appropriate directory permissions/ACLs. There is no export-to-file command yet, and Permesh uploads no reports.

See [threat model](threat-model.md), [security boundaries](SECURITY_MODEL.md), [secret handling](secrets.md), and [reporting policy](../SECURITY.md). This pre-release milestone is not a third-party security assessment; real credential/keychain/platform release qualification is pending.
