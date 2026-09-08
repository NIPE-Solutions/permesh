# ADR 0017: Provider-scoped workspace approval

Status: Accepted

## Context

ADR 0010 binds every provider to the complete normalized workspace. Adding an
unrelated instance therefore revokes permission to run already-reviewed code.
The execution boundary should track the context delivered to, or interpreted
for, that provider.

## Decision

Keep the existing private approval store, canonical file binding, registration
verification and approval-before-secret-resolution ordering. Fingerprint an
explicit context tagged `permesh-provider-context-v2`, containing workspace
schema, the complete selected ProviderConfig (including digest, settings and
credential references), identity-source entries for that instance, and explicit
identity aliases whose account bindings reference that instance. The existing
outer fingerprint also includes canonical workspace path, instance ID and the
complete registered provider ID/digest/capabilities.

Exclude organization display name, other provider definitions, other authority
declarations and unrelated aliases. Alias account order remains significant;
JSON object order is canonicalized by the existing fingerprint implementation.
Future network/security inputs must live in the selected provider context or
be added explicitly before they can affect execution.

## Compatibility and consequences

The tagged context intentionally invalidates previous whole-workspace approvals
once. Review and approve again; never migrate consent silently. Store schema,
CLI JSON schema, wire versions and workspace schema do not change. Review still
shows the complete identity configuration as useful surrounding context, but
only the selected instance's portion is fingerprinted.

Changing the binary, registration, settings, credential reference, selected
authority or selected alias still requires review. Credential rotation behind
an unchanged reference does not. Moving/cloning the workspace still requires
approval. Changing another provider or organization name no longer does.

This refines approval scope, not native-process isolation. Trusted executables
continue to run with the user's OS privileges.
