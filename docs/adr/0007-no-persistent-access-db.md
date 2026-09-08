# 7. Ephemeral access graph

Status: accepted

## Decision and consequences

Fetch, normalize, correlate, query, discard. Do not persist graph data or add SQLite. Explicit JSON output is controlled by the caller. Future caching requires local configurable retention and deletion behavior.
