# ADR 0022: Guided official provider trust

Status: Proposed; orchestration not implemented

## Decision

Make `provider add github` compose the existing public catalog download, artifact
verification, local registration, declarative setup and workspace approval steps.
Display publisher, selected version, verified artifact and capabilities before
asking for execution trust. Display the resulting instance's settings and named
credential references before approving delivery. Authentication remains explicit.

Reuse validated package metadata to carry paths/digests internally. Do not require
users to copy hashes, repeat capabilities or create low-level trust records for
official packages. Keep the technical third-party commands unchanged. Downloads
do not imply execution trust; rejection before trust must execute no provider.

## Consequences

Each persisted step must be resumable and report a concrete continuation. A
noninteractive mode needs explicit consent flags and strict answer files, never
an assumed yes. Update downloads do not silently change instance pins/approvals.
GitHub catalog/release infrastructure remains transparent; no mandatory project
backend or query-time update lookup is introduced. Checksums bind bytes to the
reviewed manifest, not independent publisher authenticity; attestations/signatures
must be described according to the verification actually performed.
