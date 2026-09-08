# 6. Versioned subprocess protocol direction

Status: accepted

## Decision and consequences

Adopt NDJSON stdin/stdout with independent protocol version and read-only capabilities. No foreign runtime embedding or workspace auto-execution. Implement and test execution only after explicit user-local trust storage exists. The pure protocol crate now validates finite discovery transcripts and the Python sample emits normalized records. The draft deliberately changed before any plugin loader was enabled. No subprocess host or trust store exists yet; successful offline validation is not authorization to execute code.
