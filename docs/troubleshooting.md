# Troubleshooting

Start with `permesh doctor`. It validates the loaded schema and secret references and performs lightweight provider health checks. It does not enumerate the access graph or certify full discovery permissions.

| Symptom | Next step |
| --- | --- |
| No workspace found | Run `permesh init`, move inside a workspace, or use `--config FILE` |
| Invalid YAML/config | Check reported line/column, schema version, unknown fields, duplicate IDs, and reference-only auth; raw source is deliberately omitted |
| Keychain unavailable | Unlock the native store; on headless Linux use an env reference supplied by your existing secret tooling |
| Authentication unavailable | Set the configured env variable or run `permesh auth login INSTANCE` for a keychain reference |
| Legacy GitHub migration required | Install and explicitly trust the external GitHub binary, run `provider migrate INSTANCE --sha256 DIGEST`, then review and approve the changed workspace |
| GitHub 401 | Replace/revoke expired token; verify it belongs to the intended user |
| GitHub 403/404 | Check token approval, organization membership, SSO authorization, and endpoint permissions; private resources may be deliberately hidden |
| Rate-limit/deadline failure | Wait before retrying; reduce the organizations per instance; collection is bounded rather than hanging indefinitely |
| No canonical email match | Use a unique login lookup or add a reviewed immutable-ID alias; public GitHub email is not verification |
| Ambiguous lookup | Use `INSTANCE:ACCOUNT_ID` for a unique account or resolve contradictory authoritative mappings; Permesh will not choose silently |
| Partial results, exit 4 | Review every failed/partial provider and its limitations; absence in partial data is not proof of no access |
| Auth login with env reference | Set the variable through your shell/secret tooling; login/logout only manage native keychain entries |

`-v` adds a curated UTC observation window. It never enables raw headers or payload logging. `--json` produces a schema-versioned object suitable for CI. `--color never`, `NO_COLOR`, and `TERM=dumb` suppress styling; `TERM=dumb` also uses ASCII structural separators. Redirected output has no animations or ANSI by default.

When filing a bug, remove identities, resource names, provider configuration and credentials. Provide tool version, OS, exit code and a minimal synthetic reproduction. Do not post raw access exports publicly.

## Detailed doctor output

Run `permesh doctor --details` for a curated explanation of each provider's
first observed failure and the next local action. Use `--json` with it for
versioned stage/code rows. This uses the same approved health check as ordinary
doctor; it does not enumerate access or run a second provider probe.

A missing native target pin stops before local credential access. Missing or
changed binary trust requires inspecting and trusting the intended installation.
A missing/stale approval requires reviewing and approving that instance's current
fingerprint. Network validation failures require reviewing proxy settings and
pinned CA bytes. Credential failures require configuring the declared environment
reference or keychain slot. Permesh does not check secrets before approving an
external execution context merely to improve a diagnostic report.

A final launch-integrity failure means the executable no longer passes the
trusted pin/path checks immediately before spawn. Investigate its installation
before trusting it again. A protocol failure does not necessarily mean expired
credentials: the host deliberately withholds raw remote errors. Check the
configured discovery contract and compatible provider version. Network feature
negotiation never falls back to direct connections.

A successful health row does not prove complete discovery visibility. Review
provider limitations and run the relevant query. Cleanup failure uses exit 5;
investigate any remaining provider processes before retrying.
