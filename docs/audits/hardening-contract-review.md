# Hardening contract and security review

This review covers the implemented enterprise network, portable-pin and launch
integrity/diagnostic slices. It does not declare the provider protocol stable or
qualify live provider tenants. The original [architecture audit](architecture-hardening.md)
remains the baseline; this document records the resulting boundaries.

## Contracts retained

| Boundary | Result |
| --- | --- |
| Domain to provider wire | Independent closed DTOs and explicit mappings; completed snapshots validate references and scope before exposure |
| Domain to CLI JSON | Independent access/control DTOs; new portable and detailed diagnostic fields are deliberate and documented |
| Wire compatibility | Negotiated `protocol_version: 1` remains separate from legacy `protocol` drafts; no fallback |
| Optional networking | Requested/acknowledged `network_v1` feature; configuration and credentials withheld after failed negotiation |
| Identity | Independent principal kind, affiliation and lifecycle; unknown evidence remains unknown |
| Access | Native role and normalized privilege retained; policy attachments/assignments do not claim effective authorization |
| Resource | Provider-owned kinds and validated containment; stable scoped identifiers |
| Local state | Credentials, trust, approvals and installed artifacts remain local; configuration and reviewed pins can live in Git |

Setup and browser descriptions retain their frozen legacy operation drafts.
Future compatible operations belong in negotiated operation capabilities, not
new protocol versions. Existing providers are never probed or downgraded after a
failure. Historical fixtures remain compatibility requirements, including default
no-feature handshakes and scalar-pin output/approval serialization.

## Security conclusions

Network context is explicit per instance, bounded and included in approval.
Public CA bytes are checked against a workspace pin before credential resolution,
then delivered as bytes after feature negotiation. TLS consumers validate DER
and retain hostname checks. Approved native code can still ignore routing policy;
it runs with the user's OS permissions. AWS networking, host browser networking,
custom endpoints and proxy authentication remain unsupported.

Portable maps select an exact digest for the running target, independently of the
registry's mutable selected version. All target pins participate in review and
approval. Guided setup groups one exact catalog release with matching contracts
and installs/trusts only the native artifact. Catalog checksums are integrity
metadata, not a publisher-signature claim. Install/update never silently adopts
workspace changes.

Launch verification now narrows the interval after credential preparation by
rehashing the protected managed executable at the spawn boundary. Cancellation
is checked before process state and again without an async handoff before spawn.
The path lookup remains an OS-level race against hostile same-user code; no
cross-platform atomic executable-handle guarantee is claimed. Existing bounded
IO, deadlines, process-group/job cleanup, cleared environments and credential
reflection defenses remain required.

Detailed diagnostics use typed local boundaries and finite static codes. They
neither reflect arbitrary remote errors nor infer authentication failures from a
protocol error. A successful doctor health check does not prove discovery
completeness. Default doctor/status JSON remains unchanged; requested details use
a separately versioned output object. Cleanup failures consistently return 5.

## Verification and remaining gates

Permanent tests cover malformed/duplicate/unknown wire data, completion and EOF,
network feature rejection before credential delivery, CA/path substitution,
proxy routing/bypass and real TLS trust/hostname behavior, native/foreign pin
mutation, retained versions, cancelled consent and concurrent config edits,
pre-cancel startup prevention and final launch substitution rejection.

Each merge still requires its exact PR source tree to pass formatting, strict
Clippy, complete tests, MSRV, dependency checks and the native target matrix.
Run IDs and outcomes belong to the PR checks and release evidence rather than a
blanket claim that every subsequent revision is qualified.

Before publication, qualify the actual matching CLI and provider archives through
installation, trust, setup, approval, credentials, health and discovery. Preserve
original artifact bytes and verify checksums/inventories/provenance for the
reviewed release revision. Published packages and catalog entries do not change
when source PRs merge. Live Google/Cloudflare/AWS acceptance, broader GitHub
visibility, interactive desktop credential UX and platform signing remain
separate gates. No backend, telemetry, remediation or new provider count is
needed to complete these checks.
