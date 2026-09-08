# Roadmap

The release is deliberately smaller than the long-term product vision.

1. **Implemented** — Foundation: accepted design, dependency research, dual license, strict config, redacted secrets, workspace CI.
2. **Implemented** — Demo access slice: synthetic provider, validated graph, identity resolution, init/doctor/user JSON and human output, binary integration tests.
3. **Implemented; initial live smoke test passed** — GitHub observed-access slice: read-only API adapter, pagination, bounded retries, scope diagnostics, inheritance paths, mock HTTP tests, authentication docs. Connectivity, user JSON, and privileged-access JSON passed a live exercise; privileged grants matched an independent API comparison. [Validation scope](getting-started.md#live-validation); full live acceptance remains pending.
4. **Candidate workflow implemented; qualification pending** — Release qualification: native platform CI, supply-chain checks, credentialed GitHub fixture organization acceptance, release automation and checksums. No public release before this passes.
5. **Implemented** — Privileged-access inspection: `admins` retains known privileged paths, unknown role semantics, ambiguous identities, and group grants without observed accounts. It does not require an employee directory.
6. **Implemented; live tenant qualification pending** — Google Workspace directory source: explicit customer and authority, immutable identity IDs, conservative status mapping and OAuth access-token references. `orphaned` now reviews observed accounts with conservative source-completeness handling and separate service/bot/external categories. Additional authentication flows follow separately.
7. External providers: implement the documented protocol and local trust registration; hostile-process tests before execution is enabled.
8. Additional providers: AWS and Cloudflare incrementally, with documented limitations and identical contract tests.

Release acceptance criteria are in [releasing.md](releasing.md). Future scopes are intentionally unsupported, not fake adapters.
