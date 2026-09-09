# ADR 0031: Explicit temporary AWS profiles

Status: accepted for the unreleased source candidate

## Decision

Support a strict subset of the AWS shared-credentials file as a host-owned source:
one absolute file path and one named profile, selected in the provider instance's
approved configuration. Read the three temporary session fields once per operation
and deliver them through the existing named-credential protocol. Reject static keys,
other credential sources in the same instance, helper execution and SDK fallback.
The provider verifies the explicitly configured caller account and assumed-role name
before discovery. Neither the source path nor the profile name is sent to the child.

## Why

AWS SDK default chains include ambient environment, shared files, metadata services
and potentially executable profile helpers. Restoring those defaults would weaken
the existing sanitized process boundary. A mature shared-file format provides a
small useful integration for existing temporary-session workflows without taking
ownership of login, role assumption or SSO token refresh.

We evaluated the official Rust AWS configuration components. Their general chain
and SSO provider are not used here: the supported source must not discover additional
files or refresh tokens outside an explicitly selected context. AWS service adapters
continue using official SDK signing and service clients with explicit credentials.
This is a bounded profile subset, not a replacement AWS credentials framework.

## Consequences

The file is local machine state and may contain other profiles. Input memory is
bounded and zeroized; only the selected three fields become provider inputs.
Changing source path, selected profile, caller account/role or network context
requires fresh provider approval. Rotating values within that context does not.
Queries never modify AWS files, invoke helpers or persist a second credential copy.

Session validity is checked by AWS, since shared-credentials files have no standard
expiry field. Native SSO cache handling and automatic role refresh remain unsupported;
future additions require separately reviewed authentication contracts. File reads
retain ordinary same-user filesystem race and OS credential-storage limitations.

See [configuration and operational limits](../aws-profiles.md) and the official AWS
references linked there.
