# ADR 0009: Explicit native provider trust and supervision

Status: Accepted

## Context

A versioned JSON protocol validates records but cannot authorize executable code
or prevent a hung process. Repository configuration is untrusted input. Native
execution must require an independent local decision and must work across the
three supported operating systems.

## Decision

Add a separate external-provider crate and explicit `provider external` commands.
Inspecting a binary only computes its digest. Registration requires a reviewed
SHA-256, provider ID, capability set and risk acknowledgment, then copies the bytes
into protected user-local storage. Registrations are create-only and verified
before each launch. Workspace configuration cannot refer to executables yet.

Support self-contained native binaries first. Do not pass arguments, workspace
configuration, environment credentials or secret values. Match handshake identity
and capability set before requesting discovery. Validate frames incrementally,
drain/discard bounded stderr concurrently and enforce fixed deadlines. Supervise
Unix process groups and Windows jobs through process-wrap. Await cleanup after
cancellation and after normal completion, including remaining descendants.

Use native permission checks for registration storage. Unix uses ownership and
mode checks on the storage and ancestors. Windows needs protected DACLs at object
creation and validation of owners and access entries. No sufficiently mature safe
wrapper covered this boundary, so the external crate has one private Windows FFI
module using official windows-sys bindings. Unsafe code is denied elsewhere in
that crate and forbidden elsewhere in the workspace. Every FFI call documents
pointer ownership, buffer lifetimes and error handling; native Windows tests are
required. This exception is narrower than adding an unqualified ACL dependency.

## Consequences

This is permission to execute local code, not a sandbox or an authenticity claim.
Libraries and the operating system remain trusted. Same-user replacement races
and deliberate Unix process-group escape are outside containment guarantees.
Scripts require a separate trust design covering interpreter and dependencies.
Credential transport, ordinary query integration and identity-source authority
remain separate milestones. Registration and output schemas have their own
versions; the wire protocol remains draft 1.

## Subsequent extension

[ADR 0010](0010-approved-external-workspace-invocations.md) adds exact local
workspace approval, draft-2 health/discovery and named credential slots while
preserving the original standalone draft-1 boundary and built-in token handling.
