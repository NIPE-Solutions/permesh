# Contributing

Permesh is pre-release. Start with [product scope](docs/PRODUCT.md), [architecture](docs/ARCHITECTURE.md), and [security model](docs/SECURITY_MODEL.md). The current slice is the demo and GitHub observed-access CLI. Do not add mutation APIs, telemetry, persistent access storage, or executable workspace hooks.

Use stable Rust; the workspace declares its minimum version in Cargo.toml. Install rustfmt and clippy with rustup. From the repository root run:

```sh
cargo build --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 -m unittest discover -s examples/external-provider -p 'test_*.py'
```

After dependency installation, Rust tests can run with `--offline`. Tests use synthetic fixtures and local mock servers; do not require real tokens or an unlocked keychain. Keep credentials, personal account data, and local workspace configuration out of commits and test snapshots. Changes to credentials, paths, parsing, pagination, subprocesses, and output need adversarial regression coverage.

Keep domain code free of network and filesystem dependencies. Providers return structured issues when observations are incomplete. An empty result must not hide an error. Document permission and completeness limitations with each provider. Treat versioned JSON changes as public interface changes, even before release.

A pull request should state the behavior changed, its reason, the checks actually run, and any remaining limitations. Update the changelog for user-visible changes. Dependency updates include the lockfile and a license/advisory review; see [dependency decisions](docs/DEPENDENCIES.md). Native-platform CI and credentialed provider acceptance are separate gates, not results implied by local tests.

Contributions are accepted under [MIT](LICENSE), matching the repository license. Follow the [code of conduct](CODE_OF_CONDUCT.md). Report sensitive findings using [SECURITY.md](SECURITY.md).
