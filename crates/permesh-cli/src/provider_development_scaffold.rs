// SPDX-License-Identifier: MIT
//! Source templates inherit the selected workspace's SDK/runtime versions.
pub(super) fn files(id: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Cargo.toml", MANIFEST.replace("@ID@", id)),
        (
            "src/main.rs",
            include_str!("provider_development_main.txt").replace("@ID@", id),
        ),
        (
            "src/tests.rs",
            include_str!("provider_development_tests.txt").to_owned(),
        ),
        ("README.md", README.replace("@ID@", id)),
    ]
}
const MANIFEST: &str = r#"# SPDX-License-Identifier: MIT
[package]
name = "permesh-provider-@ID@"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license = "MIT"
publish = false
[dependencies]
permesh-native-runtime = { path = "../../crates/native-runtime" }
permesh-core.workspace = true
permesh-provider-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
zeroize.workspace = true
[dev-dependencies]
permesh-provider-protocol.workspace = true
"#;
const README: &str = r#"# @ID@: synthetic native scaffold

This is an offline synthetic example, not an integration or published package.
It uses the selected workspace's SDK and native runtime. No network request is
made. The required `token` slot accepts a synthetic value for demonstrating the
credential boundary, not real authentication. It is discarded and never echoed.

1. Review these files and the workspace's Cargo dependencies/build scripts.
2. Add `providers/@ID@` to the root Cargo.toml workspace members yourself.
3. Run `cargo test -p permesh-provider-@ID@`, then
   `cargo build -p permesh-provider-@ID@`. These explicit Cargo commands execute
   compiler/build tooling; scaffold generation never does.
4. Reuse SDK snapshot validation and native-runtime protocol/process fixtures.
5. Before running the binary through Permesh, use the normal native executable
   inspect/trust, setup, workspace review and approval flow. A successful offline
   transcript validation does not create execution trust or workspace approval.

The metadata declares only `accounts`. Discovery emits one immutable synthetic
account in the host-selected instance; `partial: true` demonstrates incomplete
visibility. Health is synthetic and does not certify any API permission. Setup
contains the optional `partial` flag and one required `token` credential slot.
The shared runtime handles negotiated v1 discovery/health and frozen legacy
setup, framing bounds, cancellation and static error output. Network feature
support remains disabled until every outbound request can honor host context.

## Before replacing synthetic data

- Define a least-privilege permission table: API operation, required scope/role,
  tenant selection, authentication lifecycle and inaccessible-data behavior.
  No upstream permissions are required for this synthetic example.
- Declare only implemented capabilities. Keep native immutable IDs, tenant and
  instance boundaries, identity kind/affiliation/lifecycle, group/resource
  hierarchy, direct/derived evidence and certainty. Public email is not verified.
- Bound pages, records and time; validate cross-page references after collection.
  Preserve partial results and limitations when the intended scope is incomplete.
- Add adapter fixtures for pagination, rate limiting, denied access and malformed
  API data; these cannot be qualified by a provider with no upstream API.
- Reuse existing domain/protocol fixtures for duplicate IDs, invalid references,
  cycles, cancellation and credential reflection; test your adapter's mappings.
- Separate synthetic tests, explicitly authorized live-tenant acceptance and
  native artifact qualification. Record exact versions, scope, unsupported paths
  and tested platforms. Do not publish this synthetic scaffold in a catalog.
"#;
