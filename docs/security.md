# Security

Permesh reads provider access metadata. It does not change accounts, roles, sessions, memberships or grants. Prefer narrowly scoped read-only tokens and revoke them using the provider's own controls.

Shared YAML stores references, never credential values. Environment references are appropriate for CI; native OS credential stores are appropriate for interactive local use. `auth login` and `auth logout` mutate only the selected local credential entry and require an explicit command. Querying never stores credentials.

Workspace configuration cannot select executable paths, shell commands,
executable secret resolution, YAML includes or remote configuration. An external
instance names a locally registered native provider and pinned digest; it cannot
execute or resolve its credentials until this canonical workspace path and selected provider security context have matching
local approval. Review endpoint settings,
credential references, aliases and identity authority before approving. A clone or change to the approved provider context requires a fresh review;
unrelated provider edits preserve approval. Official GitHub uses its fixed HTTPS
origin with redirects disabled.

Approved external discovery participates in ordinary access queries; approved
health checks run for doctor/status. Native code runs as your user and is not
sandboxed. Credentials go over stdin only after strict identity/capability handshake matching (and required-operation validation
for explicitly selected wire 5);
cleared environment, discarded stderr and response-echo rejection do not prevent
malicious code from retaining or exfiltrating them. See the
[external trust workflow](external-providers.md).

Errors are curated, secret wrappers redact Debug and zeroize on drop, and terminal controls from provider data are escaped. No raw HTTP logging is implemented, even under `-vv`. These choices reduce accidental exposure; they do not protect against a compromised OS, debugger, provider, or malicious executable dependency.

Configuration reads have size, nesting, node and expansion limits. Nonregular files are rejected before opening; a process capable of racing file replacement is outside the safe-workspace assumption. Init uses create-new semantics and restrictive Unix permissions. Explicit provider add writes an atomic temporary replacement and refuses a symlink target; simultaneous editing is not supported. Windows uses native inherited ACLs, which administrators should review.

JSON reports can expose account names, infrastructure names, access relationships, and immutable IDs. Protect redirected files with appropriate directory permissions/ACLs. There is no export-to-file command yet, and Permesh uploads no reports.

See [threat model](threat-model.md), [security boundaries](SECURITY_MODEL.md), [secret handling](secrets.md), and [reporting policy](../SECURITY.md). This pre-release milestone is not a third-party security assessment; real credential/keychain/platform release qualification is pending.
