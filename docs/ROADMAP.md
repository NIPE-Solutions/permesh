# Roadmap

The release is deliberately smaller than the long-term product vision.

1. **Implemented** — Foundation: accepted design, dependency research, MIT license, strict config, redacted secrets, workspace CI.
2. **Implemented** — Demo access slice: synthetic provider, validated graph, identity resolution, init/doctor/user JSON and human output, binary integration tests. Static shell completions cover Bash, Zsh, Fish, PowerShell and Elvish without workspace discovery.
3. **Implemented; initial live smoke test passed** — GitHub observed-access slice (now in the separate provider repository): read-only API adapter, pagination, bounded retries, scope diagnostics, inheritance paths, mock HTTP tests, authentication docs. The former bundled adapter passed connectivity, user JSON, and privileged-access live checks; privileged grants matched an independent API comparison. [Validation scope](getting-started.md#live-validation); full live acceptance remains pending.
4. **Alpha distribution prepared; stable qualification pending** — Release qualification: native platform CI, supply-chain checks, credentialed GitHub fixture organization acceptance, release automation and checksums. The alpha documents unqualified native credential stores and live-provider paths; stable release requires the remaining gates.
5. **Implemented** — Privileged-access inspection: `admins` retains known privileged paths, unknown role semantics, ambiguous identities, and group grants without observed accounts. It does not require an employee directory.
6. **Implemented; live tenant qualification pending** — Google Workspace directory source: explicit customer and authority, immutable identity IDs, conservative status mapping and OAuth access-token references. `orphaned` now reviews observed accounts with conservative source-completeness handling and separate service/bot/external categories. Additional authentication flows follow separately.
7. **Native workspace integration implemented; release qualification pending** — External providers: draft-1 standalone discovery, draft-2 approved workspace discovery/health, digest-bound local registration, exact workspace/configuration approvals, named env/keychain credentials over stdin after handshake, explicit source authority and ordinary user/admins/orphaned integration. Native supervision retains bounded streams and awaited cancellation cleanup. Declarative setup provides typed conditional CLI-owned prompts or strict answer files using draft-3 description exchanges; setup does not approve configured execution. Explicit catalog installation/update commands use bounded GitHub downloads and private side-by-side packages. Trust retains old digest versions so existing workspace pins remain usable. Explicit legacy GitHub migration is implemented and the bundled GitHub adapter is removed. GitHub provider 0.1.0 is published for all five targets; dynamic provider-driven steps, automatic updates and interpreted-provider trust remain deferred.
8. Additional providers: AWS and Cloudflare incrementally, with documented limitations and identical contract tests.

Release acceptance criteria are in [releasing.md](releasing.md). Future scopes are intentionally unsupported, not fake adapters.

## Next priorities

1. Complete Windows/Linux native credential-store and interactive terminal acceptance, GitHub fixture exercises, and live Google directory qualification before stable v0.1.
2. Move Google into the official provider repository with its own setup and release qualification. Keep the offline demo available without a download.
3. Improve provider onboarding ergonomics while preserving separate download, trust and workspace approval decisions. Declarative conditional questions work; dynamic API-driven setup and richer validation remain future work.
4. Add provider-native authentication (Google refresh/browser flows, later AWS credential chain), then AWS and Cloudflare with contract and API tests.
5. Add package-manager distribution, signing/notarization and attestations/SBOMs; evaluate cargo-dist as distribution grows. CLI self-update is not implemented.
6. Later: deterministic config composition/local overrides, schema migrations, policies/audit and snapshot diff. No collaboration backend is planned.
