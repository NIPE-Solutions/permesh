# 5. Normalized provider snapshots

Status: accepted

## Decision and consequences

Use async boxed futures and explicit capability metadata. Providers own HTTP/pagination/normalization; core owns identity correlation and path queries. Validate snapshots before use, bound records and execution. Buffered snapshots simplify v0.1; streaming is reserved for the versioned protocol.
