# 6. Versioned subprocess protocol direction

Status: accepted

## Decision and consequences

Adopt NDJSON stdin/stdout with independent protocol version and read-only capabilities. No foreign runtime embedding or workspace auto-execution. Implement and test execution only after explicit user-local trust storage exists. The pure protocol crate now validates finite discovery transcripts and the Python sample emits normalized records. The draft deliberately changed before any plugin loader was enabled. [ADR 0009](0009-native-provider-trust.md) adds explicit native trust and supervision; successful offline validation alone is never authorization to execute code.
