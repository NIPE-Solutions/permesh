# Security model

Permesh trusts the installed executable and its reviewed dependencies, the OS credential store, and explicitly selected provider endpoints. Workspace YAML is untrusted input. It cannot name executables, resolve secrets through commands, include arbitrary files or authorize native code on its own. External instances reference locally registered IDs and digests; exact local workspace approval is required before their credentials are resolved or code runs. Approved external settings may select endpoints and must be reviewed accordingly.

Only `env://NAME` and `keychain://instance-id/credential-name` secret references are accepted. Built-in providers retain the token-only contract; external keychain accounts must match their declared slot and instance. Resolved values have redacted Debug and zeroize on drop; no serialization implementation. Errors contain curated categories and actions, never raw HTTP bodies, headers, YAML excerpts, or credential-store diagnostics. Configuration errors must not echo an accidentally embedded plaintext credential.

GitHub uses a fixed HTTPS API origin with redirects disabled. Pagination must remain on that origin and endpoint, responses and pages are bounded, and only GET requests are sent. HTTP errors have bounded retries and deadlines. No token is sent to an endpoint selected from a provider response. Enterprise endpoints require a future local trust mechanism rather than allowing a cloned repository to redirect credentials.

Configuration size and parser expansion budgets bound hostile input. Workspace creation uses create-new semantics and restrictive Unix modes; it never overwrites symlinks. OS ACL inheritance applies on Windows. Native credential operations occur only on explicit auth commands; inspection only resolves references.

External native binaries require explicit user-local registration of reviewed
bytes, digest, provider ID and capabilities. Standalone draft-1 discovery is an
explicit command and receives no credentials. Draft-2 workspace health and access
queries additionally require protected local approval of the canonical config
path, instance, selected provider configuration and registration. Relevant
alias, authority, settings, reference or registration changes invalidate approval; a clone
at another path cannot inherit it. Approval is checked before resolving external
credentials or launching the managed copy. The handshake must match version, ID
and capabilities before the operation and named credentials are sent on stdin.
Response reflection of supplied credential strings/keys is rejected after JSON
decoding; this does not block malicious transformations or exfiltration. The host
bounds frames and deadlines, discards stderr, requests group/job termination and
awaits direct-child reaping. Repository presence is never authorization. Execution is not sandboxed: see [trust boundaries and residual risks](external-providers.md).

Known boundaries: an overprivileged token remains powerful outside Permesh; the process/OS can inspect live memory; a compromised provider can lie; shell redirection determines report permissions; collection is not a transaction. No software here eliminates those risks.

Offline external transcript validation rejects duplicate JSON keys, unknown fields, mismatched identities/capabilities, incorrect terminal counts and invalid graph relationships. Frame, total-byte and record budgets are enforced before accepting a snapshot. This parser is not a subprocess supervisor or a trust decision; it cannot establish the truth of provider assertions.

Explicit instance proxy/CA settings use [approved network context](networking.md);
provider environments stay sanitized and unsupported transports fail closed.
