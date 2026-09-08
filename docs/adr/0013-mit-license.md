# ADR 0013: One MIT license for the main repository

Status: Accepted

## Decision

License the main Permesh repository under MIT, with one root `LICENSE` file.
Use `MIT` consistently in workspace package metadata and project SPDX headers.
Preserve the existing copyright notice.

MIT provides a short, permissive license for the CLI, SDK and protocol. One
license makes the contribution and distribution terms easier to understand.
This supersedes the original choice of `MIT OR Apache-2.0` for new revisions
of this repository. Previously published revisions retain their original terms.

## Consequences

Contributions to this repository use MIT. Dependency licenses and notices remain
independent and must be preserved when distributing binaries. This decision does
not relicense third-party dependencies or the separate providers repository.
