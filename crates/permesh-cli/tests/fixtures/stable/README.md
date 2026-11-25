# Stable format fixtures

These files freeze the formats declared stable by the second stability
declaration ([ADR 0036](../../../../../docs/adr/0036-second-stability-declaration.md),
[stability](../../../../../docs/stability.md)). `tests/stable_formats.rs` builds each
format from the synthetic demo and compares it exactly: field names, types, enum
spellings, array order and `null`s.

Only run-specific values are replaced with `"<normalized>"`: string values of keys
ending in `_at`, `sha256` or `digest`, numbers under keys ending in `_ms`, and
strings starting with the temporary workspace path. Paths under `files` compare
with `/` separators, so Windows output matches. Human-readable text (`summary`,
`message`, notification failure reasons and the `profile suggest` draft) is
normalized too; the draft is checked to load as a profile instead.

People and profile files, and `permesh.yaml` other than its `check` and `profiles`
sections, are inputs: their parsing tests in `permesh-config` keep them readable.

A failing fixture means a stable format changed. Because the comparison is exact,
an allowed additive change (a new optional field) fails too: regenerate with
`PERMESH_BLESS=1 cargo test -p permesh-cli --test stable_formats` and check that the
diff only adds. Any other difference (a renamed, removed or retyped field, a new
enum spelling, a different order) needs a version bump and a migration guide in
`docs/migrations/`. Never regenerate to make a test pass.
