# Threat model

Status: pre-release design and review checklist. The security model is a requirement; test and release evidence must establish each implementation claim.

## Assets and boundaries

Assets include provider tokens, organizational identities and access metadata, workspace files, trust decisions, and the integrity of reports. Boundaries are untrusted workspace YAML to the application, the application to the native credential store, the HTTP client to the fixed provider origin, provider observations to the domain validator, and structured values to human/JSON output. The installed binary and dependencies, OS, and explicitly selected provider are trusted within their stated limits.

| Threat | Required control | Remaining boundary / validation |
| --- | --- | --- |
| Malicious cloned configuration | Strict schema/bounded parsing; no executable paths/includes/command interpolation; bind external approval to canonical config path, selected provider settings and relevant identity context before credential resolution | Parsing must not echo embedded credentials; test aliases, duplicate keys, oversized and malformed data |
| Symlink or path replacement during initialization | Create-new files, restrictive Unix permissions, no overwrite of existing entries | Parent-directory replacement and OS ACLs require platform review; no promise against an attacker controlling the user's filesystem |
| Credential leakage | References only in shared YAML; redacted, nonserializable secret wrapper; curated errors; no raw response/credential-store diagnostics | Memory inspection, shell history and environment visibility remain OS/user concerns |
| Provider response redirects or pagination exfiltration | Fixed HTTPS origin, redirects disabled, validate pagination origin and endpoint before sending authorization | Test crafted Link headers and redirects with sentinel credentials |
| Misleading or malicious provider data | Scoped immutable IDs, validate graph references, bound input/pages/traversal, escape terminal control characters | A provider can lie or omit objects; observations cannot establish complete effective access |
| Provider impersonation or malicious plugin | Digest-bound managed copies, protected local registration and exact workspace approval, strict version/identity/capability handshake before credential delivery | Claimed provider name is not authenticity; trust registry must live outside repository configuration |
| External credential reflection or endpoint change | Review configuration and reference slots; invalidate changed approvals; send values on stdin only after handshake; reject decoded response strings/keys containing supplied values | Trusted code can transform, persist or exfiltrate values; this is not a sandbox |
| Hung or flooding plugin | Frame/total-output bounds, request deadline, cancellation, process-group/job termination and reap, bounded discarded stderr | A subprocess is not a sandbox; same-user file/network access and deliberate Unix process-group escape remain possible |
| Identity confusion | Explicit mappings or exact verified email evidence; retain ambiguity | Public GitHub emails do not prove identity; test duplicates and cross-provider same native IDs |
| Export disclosure | No automatic persistence; explicit bounded snapshot/plan/report writes use private create-new files by default (snapshot replacement requires explicit `--overwrite`); offline inspection does not execute providers | Exports contain sensitive access metadata; inherited Windows ACLs, backups, terminals, shell redirects and consumers can leak data |
| Dependency compromise | Committed lockfile, reviewed features/licenses, pinned workflow actions, advisory scans, least CI permissions | Scanners only find known issues; build scripts and updates need human review |

## Acceptance exercises

Use synthetic sentinel secrets and verify they never appear in errors, stderr, Debug output, or JSON. Exercise malformed configuration and output-control characters. Use mock HTTP servers for timeouts, 401/403/429/5xx, unexpected redirects, off-origin pagination, oversized responses, and partial results. Run native keychain behavior on supported platforms without making ordinary tests depend on user credentials. Review token permissions and real visibility with a dedicated GitHub fixture organization before release.

The subprocess runtime must be tested with hostile native fixtures that never finish, fork descendants, flood both streams, send malformed UTF-8/JSON, exceed limits, lie in handshakes, and change on disk after registration. Also test cloned config paths, configuration/alias/authority/reference edits,
changed registration capabilities, revoked approvals, and escaped credential
echoes. Failed approval must not resolve external secrets or start a process.
The Python example remains an offline draft-1 fixture; native cross-platform tests
exercise execution and local permission boundaries.

## Discovery contract changes

A malicious workspace cannot silently switch a trusted provider to a different
discovery contract: the explicit selector is covered by provider-scoped approval.
Legacy serialization omits the default, preserving existing approval inputs.
Negotiated protocol 1 requires the selected operation in a bounded advertised operation set
before invocation delivery. Unknown optional operation names cannot grant
execution or alter the exact trusted record-capability set. A failed handshake
does not trigger fallback to another protocol or another process.

New record DTOs are mapped explicitly and checked as a complete graph before
returning data. Missing lifecycle/evidence dimensions, cycles, cross-instance
parents, unknown fields and unnegotiated records fail closed. Structured diagnostic
codes are validated but not displayed: syntactic restrictions alone cannot prove
that provider-controlled text is not a secret.

## Explicit networking and shared pins

Network policy is instance-scoped approved input, never inherited shell state.
The host validates bounded proxy/bypass syntax and reads a workspace-relative,
digest-pinned public CA bundle before credential resolution. It delivers checked
bytes rather than a mutable path. The negotiated `network_v1` feature must be
acknowledged before invocation data is sent; unsupported peers never trigger a
direct-network fallback. TLS consumers parse certificate DER, retain hostname
verification and augment existing trusted roots. A configured proxy or CA changes
the network trust boundary and belongs in the same configuration review as
credential references. Approved native code is still capable of ignoring policy;
transport configuration is not a sandbox.

A portable `sha256_by_target` map is shared configuration, not a trust record.
The running CLI selects one exact native digest; missing entries fail before
credentials, and mutable registry selection does not override it. The complete
map and selected target/digest bind instance approval. Guided catalog grouping
uses one exact version with consistent protocol/capability metadata. Only the
native artifact is downloaded and locally trusted. Catalog metadata does not
verify a foreign executable on behalf of other administrators or authenticate a
publisher signature. Updating packages does not update workspace pins.

Synthetic coverage includes unknown/duplicate protocol features, missing feature
acknowledgement, credential withholding, CA substitution and path escape,
malformed/private-key PEM, real TLS trust/hostname rejection, proxy bypass,
foreign/native pin changes, missing native targets, catalog contract disagreement,
and configuration changes during consent. Actual tenant permissions, enterprise
proxy deployments and release artifacts still require separate qualification.

## Verification immediately before launch

Production CLI entrypoints pass the verified registration digest into a final
protected-path, private-file and bounded native-byte hash check immediately before
spawn. This catches deterministic replacement between preparation and launch,
including credential-store delays. Setup/auth descriptions and expert discovery
use the same pinned launch boundary. Host-only digest metadata is excluded from
wire input. Already-observed cancellation prevents child creation; cancellation
arriving after the final poll remains subject to normal process cleanup.

Path-based spawning still leaves an OS-level race after verification. Rehashing
is not atomic executable-handle binding and cannot defeat a hostile same-user
process. Symlink/reparse/path checks, protected storage and explicit trust reduce
other attack surfaces but do not justify a stronger claim. The synchronous hash
is bounded by the existing executable-size limit; filesystem IO itself cannot be
cancelled. Tests should prove deterministic prelaunch substitution rejection and
absence of startup side effects, not claim to prove impossible race freedom.
