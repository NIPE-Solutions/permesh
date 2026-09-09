# Dependency decisions

Reviewed against upstream documentation and crates.io metadata on **2026-09-08**. These are compatibility selections, not a claim that every latest version is locked. `Cargo.lock` is the build's exact resolution; review it with `cargo tree --locked -e features` and `cargo metadata --locked`. Registry publication dates are evidence of activity, not proof of security or a maintenance guarantee.

| Dependency | Selected line; latest observed | License | Reason and feature boundary |
| --- | --- | --- | --- |
| [clap](https://docs.rs/clap/4.6.6/clap/) | 4.6; 4.6.6 | MIT OR Apache-2.0 | Derived typed CLI, help and parsing. Enable derive; avoid custom parser machinery. |
| [clap_complete](https://docs.rs/clap_complete/4.6.9/clap_complete/) | 4.6.9; 4.6.9 | MIT OR Apache-2.0 | Static generators maintained in the clap repository. Defaults and unstable dynamic features disabled; only the existing clap dependency is required. Rust 1.85 minimum fits the workspace floor. Generate into memory before fallible stdout writes because upstream generators panic on writer errors. |
| [serde](https://docs.rs/serde/1.0.229/serde/) | 1; 1.0.229 | MIT OR Apache-2.0 | Shared typed serialization; derive enabled. Secret values must never derive Serialize. |
| [serde_json](https://docs.rs/serde_json/1.0.151/serde_json/) | 1; 1.0.151 | MIT OR Apache-2.0 | Versioned machine output and bounded provider response parsing. Keep output deterministic in domain ordering. |
| [serde-saphyr](https://docs.rs/serde-saphyr/1.2.0/serde_saphyr/) | 1.2; 1.2.0 | MIT OR Apache-2.0 | Maintained YAML/Serde path with configurable parser budgets. Bound file bytes and parser expansion separately; strict structs reject unknown fields. Newly released line warrants hostile-input tests. |
| [keyring](https://docs.rs/keyring/4.2.0/keyring/) | 4.2; 4.2.0 | MIT OR Apache-2.0 | Native credential store abstraction. Verify platform backend selection against v4 API, including headless Linux. Never silently fall back to plaintext files. |
| [secrecy](https://docs.rs/secrecy/0.10.3/secrecy/) | 0.10; 0.10.3 | Apache-2.0 OR MIT | Deliberate exposure, redacted formatting and zeroization. Keep optional serialization disabled. Latest observed release is 2024-10-09; age alone is not an advisory, but monitor upstream. |
| [reqwest](https://docs.rs/reqwest/0.13.4/reqwest/) | 0.13; 0.13.4 | MIT OR Apache-2.0 | Async HTTP. Disable defaults, enable rustls/json/query. Redirect, origin, size, retry and timeout policy remain application responsibilities. |
| [tokio](https://docs.rs/tokio/1.53.1/tokio/) | 1; 1.53.1 | MIT | Runtime, macros, signals, time, sync and bounded collection; net/io-util for adapter/mock I/O. Test-util supports deterministic timer tests; review moving it to dev-only use as the suite settles. Avoid the blanket full feature. |
| [time](https://docs.rs/time/0.3.55/time/) | 0.3; 0.3.55 | MIT OR Apache-2.0 | Observation timestamps with parsing, formatting and serde. No local-offset feature needed. Respect current minimum Rust requirement. |
| [zeroize](https://docs.rs/zeroize/1.9.0/zeroize/) | 1; 1.9.0 | MIT OR Apache-2.0 | Clear temporary CLI token buffers on success and failure; already underpins secrecy. |
| [rpassword](https://docs.rs/rpassword/7.5.4/rpassword/) | 7; 7.5.4 | Apache-2.0 | Explicit hidden terminal credential entry. Do not accept tokens through positional arguments. |
| [tempfile](https://docs.rs/tempfile/3.27.0/tempfile/) | 3; 3.27.0 | MIT OR Apache-2.0 | Isolated test workspaces and atomic explicit configuration replacement; not a persistent snapshot store. |
| [thiserror](https://docs.rs/thiserror/2.0.20/thiserror/) | 2; 2.0.20 | MIT OR Apache-2.0 | Typed errors. Curated outward messages must not interpolate raw provider errors or secret-bearing YAML. |

[zeroize](https://docs.rs/zeroize/latest/zeroize/) 1 (MIT OR Apache-2.0) is also a direct CLI dependency so temporary stdin/terminal credential buffers are cleared on success and error paths. It already underpins secrecy.

All except secrecy have observed 2026 publications at review time. Upstream links and exact license/version metadata can be reproduced using `https://crates.io/api/v1/crates/NAME` or `cargo info NAME@VERSION`. Check the source repository and changelog as well as the registry before updates. No runtime analytics, updater, automatic plugin downloader or embedded
foreign-language engine is selected. Explicitly trusted native execution uses the
separately reviewed host dependencies below.

## Supply-chain policy

`deny.toml` rejects unknown registries, Git sources, registry wildcard requirements (private workspace paths are allowed), unapproved licenses, and unsuppressed security advisories. Duplicate versions warn because platform and HTTP dependency trees may legitimately need them; inspect warnings before release. License allow entries cover permissive transitive licenses, including Unicode data and WebPKI root data; an allow entry does not waive required attribution. Keep upstream notices when distributing dependencies and review the actual binary dependency graph. Do not add blanket advisory ignores or license clarifications without documented evidence and scope.

CI uses cargo-deny 0.20.2 and cargo-audit 0.22.2, observed published 2026-07-09 and 2026-06-05 respectively. Exact installer versions and each tool's own lockfile reduce drift; advisory databases deliberately refresh. A passing scan cannot establish absence of unknown vulnerabilities. Decisions about cargo-dist 0.32.0 and cargo-nextest 0.9.143 are in [releasing.md](releasing.md).

The MIT text was obtained from [SPDX's MIT license text](https://github.com/spdx/license-list-data/blob/main/text/MIT.txt), with the project copyright holder substituted. Project licensing is [MIT](../LICENSE); dependency licenses and their required notices remain their own. The original MIT OR Apache-2.0 choice, whose Apache text came from the [Apache Software Foundation](https://www.apache.org/licenses/LICENSE-2.0.txt), is superseded for the current project by [ADR 0013](adr/0013-mit-license.md). Earlier revisions retain their original license terms.

## Local verification record

On 2026-09-08, cargo-deny 0.20.2 reported advisories, bans, licenses and sources OK for the then-current 294-dependency lockfile. Duplicate base64 0.22/0.23 and syn 2/3 were the only warnings: they arise from independently versioned HTTP/YAML and macro dependencies, and are not suppressed. cargo-audit 0.22.2 completed successfully with `--deny warnings` against 1,242 fetched RustSec advisories. Re-run both on the final release revision; this record is not native-platform CI or a security audit.


Terminal output deliberately uses the standard library (`IsTerminal`, stdout/stderr) and a small internal renderer, rather than a table/progress framework. Vertical records remain readable in narrow terminals; no animation state needs cleanup. Styling is optional and meaning is always present in text. Test coverage uses ordinary Rust tests and local HTTP servers; bounded deterministic permutation/cycle/ambiguity fixtures cover the initial graph risk without a property-testing dependency. Reevaluate property-based generation when providers broaden the input space.

Native candidate qualification adds no runtime or Python package dependencies. Python 3.12's standard library creates allowlisted archives and checksums; [releasing.md](releasing.md#native-candidate-builds-without-platform-signing) records the verified action revision, native runner matrix, determinism limits, and remaining distribution gates. Cargo-dist remains deferred until public distribution policy is settled.

## Explicit native external providers (2026-09-08)

- [process-wrap 10.0.0](https://github.com/watchexec/process-wrap) (Apache-2.0 OR MIT, Rust 1.87): maintained Watchexec command supervision. Select only Tokio, Unix process groups, Windows job objects and kill-on-drop support; disable default tracing and unused session/creation wrappers. Explicit termination and reaping remain host responsibilities, and Unix process groups are not a sandbox against deliberate `setsid` escape.
- [etcetera 0.11.0](https://github.com/lunacookies/etcetera) (MIT OR Apache-2.0, Rust 1.87): native platform locations with only cfg-if and Windows system bindings. Use the native strategy, selecting Local AppData explicitly on Windows. This avoids directories 6's MPL-2.0 option-ext transitive dependency while preserving the existing permissive-license policy. The CLI exposes an explicit absolute `PERMESH_DATA_DIR` override for isolated local environments.
- [sha2 0.11.0](https://github.com/RustCrypto/hashes) (MIT OR Apache-2.0, Rust 1.85): SHA-256 identity for explicitly reviewed native binaries; disable optional defaults. It already exists in the resolved dependency graph.
- [rustix](https://github.com/bytecodealliance/rustix) (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT): safe Unix effective-user lookup to verify ownership of managed trust files; already used transitively. Unix permission checks require no project unsafe code.

These selections were checked using upstream source and `cargo info`. The runtime adds no analytics, update service, plugin marketplace or downloads. Third-party native providers are separately trusted code: their behavior is not established by these dependency checks.

Windows storage uses official [windows-sys](https://github.com/microsoft/windows-rs)
0.61 bindings already present in the dependency graph. Protected ACL creation and
validation require a narrow, documented FFI exception in the private Windows trust
module; [ADR 0009](adr/0009-native-provider-trust.md) records why an additional
unqualified ACL wrapper was not selected. Native Windows tests are required.
process-wrap 10's Windows completion-port waiter can return on a nonterminal job
notification; the host therefore promises termination requests and direct-child
reaping, not confirmation that every descendant has exited.

## Explicit provider distribution (2026-09-08)

- [semver 1.0.28](https://docs.rs/semver/1.0.28/semver/) (MIT OR Apache-2.0): exact stable version parsing and numeric precedence; enable serde for strict release records. It is already common in Rust tooling and avoids a bespoke parser.
- [zip 8.6.0](https://docs.rs/zip/8.6.0/zip/) (MIT): maintained native ZIP reader, MSRV 1.88. Disable default features; select only `deflate-flate2-zlib-rs` for stored/DEFLATE packages. Use bounded per-entry reads and an exact allowlist rather than general archive extraction. Its [feature documentation](https://docs.rs/crate/zip/8.6.0/features) records the selected dependency path.
- Reuse reqwest 0.13 and sha2 0.11. Distribution disables proxy discovery, redirects by default, decompression, retries and referer headers; the narrowly allowlisted archive redirect loop and total deadlines belong to Permesh.

Earlier no-download dependency notes describe the pre-distribution milestone.
Runtime downloads now occur only through explicit install/update commands; there
is still no telemetry, scheduled updater or remote configuration service.

The selected DEFLATE backend adds `zlib-rs` 0.6.7, licensed under
[Zlib](https://github.com/trifectatechfoundation/zlib-rs/blob/main/LICENSE),
copyright Trifecta Tech Foundation. Its source notice must be preserved and
modified source must be marked. `deny.toml` permits Zlib only for this exact
reviewed package version; future versions require another review. This keeps the
rest of the license policy unchanged. The remaining new ZIP dependencies
(crc32fast, flate2 and typed-path) use MIT OR Apache-2.0.
