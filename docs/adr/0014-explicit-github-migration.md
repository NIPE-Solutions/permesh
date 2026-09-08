# ADR 0014: Explicit migration of legacy GitHub instances

Status: Accepted

## Decision

Provide `provider migrate INSTANCE --sha256 DIGEST` to rewrite one existing legacy
GitHub configuration using an already trusted, exactly pinned `github` binary and
its five expected capabilities. Preserve IDs, organization arrays, token references,
aliases and other providers. The command performs local validation and an explicit
workspace replacement; it does not execute a provider, resolve credentials, change
trust or grant workspace execution approval.

## Consequences

Users review the normalized YAML diff. The full-workspace approval fingerprint
changes, so all affected external instances require renewed explicit approval.
Incompatible legacy IDs and organization bounds fail without automatic renaming
or truncation. Captured-input comparison and a synced same-directory temporary file
prevent routine stale overwrites; same-user filesystem races remain possible.

The bundled GitHub adapter remains available until the separate release qualifies.
Future adapter removal must retain legacy parsing long enough to migrate existing
workspaces and must refuse legacy execution before resolving credentials.

The later [external-only GitHub decision](0015-external-only-github.md) supersedes
the temporary bundled-execution compatibility period; legacy parsing is retained.
