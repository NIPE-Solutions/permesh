# Security model

Permesh trusts the installed executable and its reviewed dependencies, the OS credential store, and explicitly selected provider endpoints. Workspace YAML is untrusted input. It cannot execute programs, load plugins, interpolate environment values into executable commands, include arbitrary files, or select custom credential endpoints in this milestone.

Only `env://NAME` and `keychain://provider-id/token` secret references are accepted. Resolved values have redacted Debug and zeroize on drop; no serialization implementation. Errors contain curated categories and actions, never raw HTTP bodies, headers, YAML excerpts, or credential-store diagnostics. Configuration errors must not echo a accidentally embedded plaintext credential.

GitHub uses a fixed HTTPS API origin with redirects disabled. Pagination must remain on that origin and endpoint, responses and pages are bounded, and only GET requests are sent. HTTP errors have bounded retries and deadlines. No token is sent to an endpoint selected from a provider response. Enterprise endpoints require a future local trust mechanism rather than allowing a cloned repository to redirect credentials.

Configuration size and parser expansion budgets bound hostile input. Workspace creation uses create-new semantics and restrictive Unix modes; it never overwrites symlinks. OS ACL inheritance applies on Windows. Native credential operations occur only on explicit auth commands; inspection only resolves references.

External native binaries require explicit user-local trust and a separate discovery command. Registration copies reviewed bytes and binds their digest, provider ID and capabilities. The host bounds frames and deadlines, discards stderr and terminates supervised processes. Repository presence is never authorization. Execution is not sandboxed: see [trust boundaries and residual risks](external-providers.md).

Known boundaries: an overprivileged token remains powerful outside Permesh; the process/OS can inspect live memory; a compromised provider can lie; shell redirection determines report permissions; collection is not a transaction. No software here eliminates those risks.

Offline external transcript validation rejects duplicate JSON keys, unknown fields, mismatched identities/capabilities, incorrect terminal counts and invalid graph relationships. Frame, total-byte and record budgets are enforced before accepting a snapshot. This parser is not a subprocess supervisor or a trust decision; it cannot establish the truth of provider assertions.
