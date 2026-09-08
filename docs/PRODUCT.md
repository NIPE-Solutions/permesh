# Permesh product scope

**Know who has access to what.**

Permesh is a local-first, read-only CLI for inspecting, correlating, explaining, and auditing access metadata. It has no backend, accounts, telemetry, update checks, remote configuration, or persistent access database. Only explicitly configured provider APIs are contacted.

The first implementation milestone delivers `init --demo`, `doctor`, `provider list/status/capabilities`, `user <identity>`, `admins` and versioned JSON, plus a real GitHub adapter. Environment and native keychain references keep credentials out of shared YAML. Demo data is synthetic and requires no network. GitHub is an observed-access adapter, not a claim of complete effective authorization. A release remains gated on platform CI and a credentialed acceptance exercise.

Success means a new contributor can run the four demo commands without credentials, explain a team-derived grant, observe partial failures in both output formats, and reproduce tests without Internet access after dependencies are installed.

Future slices add authoritative identity sources, orphaned-account queries, trusted external execution, and additional providers. No mutation APIs, policy engine, web UI, provisioning, HR system, SSO, password manager, SIEM, billing, cloud synchronization, AI matching, or feature gates belong in this milestone.
