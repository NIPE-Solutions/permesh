# 6. Versioned subprocess protocol direction

Status: accepted

## Decision and consequences

Adopt NDJSON stdin/stdout with independent protocol version and read-only capabilities. No foreign runtime embedding or workspace auto-execution. Implement and test execution only after explicit user-local trust storage exists. Specification and sample are documentation, not an enabled plugin loader.
