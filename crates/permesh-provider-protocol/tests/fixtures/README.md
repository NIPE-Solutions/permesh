# Frozen draft transcripts

These synthetic complete response transcripts capture the accepted protocol at
the `f830270` architecture-audit baseline. They are checked in as literal NDJSON,
not generated from the current Rust serializers during tests. They contain no
tenant data or credentials.

Drafts 1 and 2 include every discovery record type, entity keys, both subject
variants, provenance and completion. The remaining files cover draft2 health,
draft3 setup and draft4 browser authentication descriptions. The baseline
decoder accepted all five fixtures before wire DTO extraction.

Preserve these bytes when evolving the domain or adding a new negotiated
protocol. Add separate fixtures for new versions; do not regenerate historical
expectations from the implementation. `frozen_contract.rs` checks acceptance,
mapping, exact record serialization, and hostile mutations. Other protocol tests
cover stream limits, ordering, duplicate keys and partial-result semantics.
