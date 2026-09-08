# ADR 0021: Explicit provider networking

Status: Proposed; not implemented

## Decision

Keep native environments cleared. Introduce a bounded host-validated per-instance
network context only with a negotiated provider operation contract: HTTPS proxy,
explicit no-proxy destinations, custom CA material and supported API endpoint.
Providers must declare support and reject unsupported inputs, not silently ignore
them. Default behavior remains current provider endpoints and platform trust.

Configuration stores references for proxy credentials, never embedded passwords.
CA files are local administrator inputs: bound reads, validate content and include
the loaded material digest in execution approval. A path alone must not let a
later file replacement silently alter trust. Custom endpoints need provider-owned
validation and explicit credential-destination review.

## Consequences

Network settings participate in the selected provider's approval context and are
sent only to that instance. Do not copy HTTPS_PROXY, AWS variables, SSH agents or
arbitrary shell state. Credential-process/native SSO integration is a separate
explicit authentication contract, not a reason to restore ambient inheritance.
No network sandbox is promised by the native host. Mock tests must cover proxy
bypass rules, CA changes, unsupported settings, redirects and secret redaction.
