# Recheck launch integrity and expose curated diagnostic stages

Status: accepted for prerelease execution.

Workspace approval and initial executable verification happen before credential
resolution. A slow credential store can widen the interval before process launch.
Bind the verified registration digest to host-only invocation metadata and check
protected paths, file permissions, native format and bounded SHA-256 again at the
common launch point. Credential-free setup/authentication/expert discovery use
pinned entrypoints too. The digest is never sent as provider input.

Poll cancellation before creating process state and immediately before spawning.
The final cancellation poll has an immediately ready alternative, so no async
handoff separates the final hash and spawn. Existing deadlines, process groups,
Windows jobs, cleanup and credential-after-handshake rules remain in force.

This narrows a path-based execution race; it cannot atomically bind execution to
the exact bytes hashed on every supported OS. A hostile process with the same
user's permissions can still race a final path lookup or modify memory. We do not
claim sandboxing, protection from a compromised OS, or immutable-handle execution.
A synchronous bounded final hash trades brief executor latency for avoiding a new
scheduling gap. It does not make filesystem IO cancellable.

Doctor retains its existing one-pass health checks. Optional `--details` presents
finite CLI-owned stages, codes and remediation attached at typed boundaries.
No English-message parsing, arbitrary remote errors, extra discovery calls or
pre-approval secret probes are introduced. Default output remains unchanged.
Detailed JSON has its own diagnostics version; numeric command exit semantics
remain separate. Process cleanup failure consistently uses internal-error exit 5.
