# ADR 0010: Approved external workspace invocations and named credentials

Status: Accepted

## Context

[ADR 0009](0009-native-provider-trust.md) permits explicit native discovery after
local binary trust. Ordinary queries need configured instances and credentials,
but a cloned workspace must not redirect credentials or authorize native code.
Binary trust alone does not approve a tenant, endpoint, identity authority or
credential reference. Health checks must remain separate from graph discovery.

## Decision

Keep native binaries in protected user-local registration storage. Workspace
schema 1 gains an external instance containing a registered provider ID, pinned
SHA-256, bounded JSON-compatible configuration and named secret references. It
never accepts an executable path, arguments, shell command or interpreter.

Require a second protected local approval binding the canonical configuration-file
path, instance, full normalized Config and registered ID/digest/capabilities.
Review shows settings, credential references, source authority and fingerprint
without secret resolution or execution. Approval requires that exact fingerprint
and explicit risk acknowledgment. Clones and changes to configuration, aliases,
authority, references or registration fail closed before external credential
resolution or launch. Revoke removes the workspace permission. Approval stores
binding metadata and fingerprints, not resolved secrets or access data.

Retain draft 1 for standalone discovery. Approved workspace discovery and health
use draft 2 without downgrade. Match version, provider ID and exact capability set
before transmitting the operation. Pass only the selected instance's configuration
and resolved named credentials on dedicated stdin, without child arguments or
environment credentials. Bound slot count, value sizes and serialized requests;
zeroize the request buffer. Reject response strings and keys reflecting supplied
credential values after JSON decoding, and discard raw stderr.

Named keychain references bind service to instance and account to slot. Built-in
providers retain the token-only contract. Credential availability and explicit
local login/logout remain separate from permission to execute a provider.

Reuse normalized discovery, domain validation, correlation and partial-result
handling for user/admins/orphaned. External identity authority requires explicit
approved source configuration and registered `identities` capability. Doctor and
provider status use health, not discovery. Signal cancellation to supervised
operations and await group/job termination requests and direct-child reaping;
do not drop their futures as a timeout mechanism.

## Consequences

Approvals are deliberately sensitive to the complete normalized configuration;
unrelated edits can require another review. Formatting alone does not invalidate
approval. Rotating a credential behind an unchanged reference preserves approval;
changing the reference does not. Registration and approval are user-local and
cannot be transferred merely by cloning repository files.

Native code runs as the user and can read files, transform or exfiltrate supplied
credentials, or lie about observations. Echo rejection and zeroization reduce
accidental leakage; they are not a sandbox, authenticity proof or memory-erasure
guarantee. OS libraries, privileged OS principals and same-user state integrity
remain trusted. Existing process-group escape and descendant-wait limitations
remain documented in the host contract.

Catalogs, downloads, automatic updates, guided setup wizards and interpreted
provider/dependency-bundle trust remain separate work. Workspace and output schema
versions remain 1; the external wire protocol has explicit drafts 1 and 2.
