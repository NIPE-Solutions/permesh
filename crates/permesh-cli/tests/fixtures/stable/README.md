# Stable format fixtures

These files freeze the formats declared stable by the second stability
declaration ([stability](../../../../../docs/stability.md)). `tests/stable_formats.rs` builds each
format from the synthetic demo and compares it exactly: field names, types, enum
spellings, array order and `null`s.

Only run-specific values are replaced with `"<normalized>"`: string values of keys
ending in `_at`, `sha256` or `digest`, numbers under keys ending in `_ms`, and
strings starting with the temporary workspace path. Human-readable text, such as
the `profile suggest` draft, is normalized too and checked for validity instead.

A failing fixture means a stable format changed. Additive changes (new optional
fields) are allowed; anything else needs a version bump and a migration guide in
`docs/migrations/`. After such a reviewed change, regenerate with
`PERMESH_BLESS=1 cargo test -p permesh-cli --test stable_formats` and review the
diff. Never regenerate to make a test pass.
