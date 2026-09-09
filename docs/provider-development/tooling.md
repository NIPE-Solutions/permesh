# Native scaffold and offline validation

These commands are current-source development tools, not features of the
published `0.1.0-alpha.2` CLI. They do not install, trust or execute providers.

## Generate one native crate

Select a reviewed checkout of the official provider workspace with its shared
`crates/native-runtime` and SDK dependencies:

```sh
permesh provider dev scaffold /absolute/path/to/permesh-providers --id example
```

This creates `providers/example/{Cargo.toml,README.md,src/main.rs,src/tests.rs}`.
It refuses existing destinations, invalid IDs and symlinked scaffold parents.
The workspace manifest is unchanged. A failed generation attempts to remove its new files; an incomplete-cleanup error means the destination needs inspection before retrying. Review the files, add `providers/example`
to the workspace members yourself, then explicitly run:

```sh
cargo test -p permesh-provider-example
cargo build -p permesh-provider-example
```

Run those Cargo commands from the selected provider workspace. They compile and
execute normal build/test tooling; only run them after reviewing the selected
checkout. The generator itself performs no dependency resolution, builds,
workspace script execution, network requests or credential lookup. It is a
workspace-member template, not a standalone cross-language package generator.

The example uses the existing native runtime for negotiated v1 discovery/health
and frozen legacy setup. It declares only account observations and one `token`
credential slot, accepts synthetic data, emits one immutable account, and offers
`partial: true` to exercise incomplete discovery. It performs no real API
requests. Health success is synthetic, not authentication or permission evidence.
The generated tests exercise SDK validation, duplicate rejection, incomplete
visibility and an in-memory runtime/decoder exchange without credential echo.

Replace synthetic data only after specifying exact tenants, stable IDs,
least-privilege permissions, authentication, bounded pagination and honest
completeness. Follow the [normalization guide](normalization.md). Enable optional
network support only when every request honors the approved context. Never send
secret values through graph records, limitations or errors.

## Validate captured responses offline

```sh
permesh provider dev validate discovery.ndjson --provider example --instance example-main --operation discover
permesh provider dev validate health.ndjson --provider example --instance example-main --operation check
permesh provider dev validate setup.ndjson --provider example --instance example-main --operation describe --discovery-protocol legacy
```

Input is a saved provider **response** transcript containing handshake and final
operation frames, not a command or executable path. Validation uses the existing
production protocol decoders, including EOF, event ordering, capability/record
consistency, duplicate-key detection and domain reference checks. Discovery and
health default to negotiated v1; `legacy` selects frozen operation-specific
contracts (discovery/health version 2, setup version 3). No automatic downgrade
occurs, and unsolicited optional features are rejected.

Files must be regular files, not symlinks, and fit the existing 64 MiB transcript
and 1 MiB frame bounds. Opened-file metadata is checked again, including Unix device/inode identity, to narrow replacement races. These local path checks are not a sandbox or protection against every hostile same-user filesystem race. Malformed input produces curated errors without echoing
frames. `--json` uses the control envelope with `development_version: 1`, a valid
flag and count/completeness summary; an incomplete but structurally valid
snapshot remains `complete: false`. Invalid input exits 2; structural success
exits 0 and does not imply complete API visibility. Provider-reported text and
records are not included in the validation summary.

Offline validation does **not** qualify an executable, prove secret safety or
establish API correctness. Running the built native executable through Permesh
still requires the normal [inspect/trust and workspace approval](../external-providers.md)
flow. Scripts and interpreter invocations remain outside production registration.
The protocol can be implemented in other languages; the existing Python examples
are explicit interoperability tests, not an installation shortcut.

## Reuse the conformance suite

| Failure or semantic case | Existing shared boundary and additional adapter work |
| --- | --- |
| Duplicate IDs, missing/cross-page references, graph cycles | SDK snapshot validation and core/protocol fixtures; accumulate bounded pages before final reference validation. |
| Incomplete observations | Protocol fixtures and generated partial example; test which API failures make your scope incomplete. |
| Pagination and rate limits | Official adapters' local mock-API fixtures; add endpoint-specific continuation, throttling and retry limits. |
| Cancellation, floods, crashes and malformed frames | Native-runtime and external-host process tests; retain bounded shutdown in the generated entry point. |
| Credential reflection | Runtime/host redaction tests and generated sentinel test; add reflection cases for your adapter's own HTTP/errors. |

Run the existing protocol tests and explicit interoperability checks described in
[provider development](../provider-development.md#test-the-full-boundary). This
command adds an offline entry point to those decoders, not a second conformance
implementation. Synthetic fixtures, authorized live-tenant acceptance and
published native-artifact qualification are separate gates. The generated README
contains a permissions/capabilities and qualification checklist to complete before
claiming a real integration.
