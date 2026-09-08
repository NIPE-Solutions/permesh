# ADR 0011: Declarative provider setup

Status: Accepted

## Context

External providers need configuration and credential references before approved
workspace queries can run. A generic list of key/value settings cannot explain
provider-specific choices or conditional authentication requirements. Letting a
provider own terminal interaction would bypass consistent input handling and
make it harder to keep configuration and secret values separate.

## Decision

Providers own a bounded declarative setup description; the CLI owns interaction,
answer parsing, credential-reference validation and workspace writes. SDK setup
schema 1 defines typed fields, defaults and conditions on earlier scalar fields.
Reject unknown fields, ambiguous keys, invalid defaults, unsafe display controls,
excessive nesting and oversized descriptions or answers. Conditions do not execute
code or resolve network references. Structured JSON is a bounded field type.

Add a describe-only draft-3 protocol exchange. Match the registered provider,
version and capabilities before requesting one setup specification. Require a
validated terminal event, EOF and successful exit. Reuse native supervision and
await cancellation cleanup. Drafts 1 and 2 retain their existing operation
contracts; setup support does not replace draft-2 discovery or health.

An explicit setup command requires prior native binary trust. It needs no second
risk acknowledgment merely to describe that already trusted binary. Description
can run without a workspace. The child receives no answers, configuration,
credentials or workspace/answer-file paths, and inherits no ambient environment.
This limits host-supplied context; it does not sandbox code running as the user.

Require a strict bounded answer file for noninteractive or JSON mutation. Parse
syntax before launch; validate schema-dependent fields after description. The
CLI explains that all settings must be nonsecret and credential fields contain
references only. It does not resolve references or write the native credential
store. Provider-authored labels cannot prove that a user-entered string is safe.

Refuse duplicate instance IDs, symlink replacement, changed registration and a
workspace changed since setup began. Validate the final configuration before an
atomic write. Authority requires an explicit flag and registered identities
capability. Setup grants no workspace approval; existing review and fingerprint
approval remain necessary before configured execution and credential delivery.

## Consequences

Providers can evolve their questions without adding provider-specific CLI code.
The host owns a small deterministic schema rather than general JSON Schema or
arbitrary terminal programs. The schema cannot perform dynamic API lookups or
request another round of questions from the provider. Catalogs, distribution,
installation and automatic updates remain separate work.

Answer files are reproducible local inputs containing nonsecret settings and
credential locators. Workspace mutation can invalidate other external approvals
under the original full-workspace approval scope (now superseded by
[ADR 0017](0017-provider-scoped-workspace-approval.md)). Same-user filesystem
races and malicious behavior by trusted native code remain outside the protection
boundary. Workspace and output schemas remain 1; setup schema 1 and wire draft 3
are independently versioned.

Subsequent domain hardening migrates access reports to output schema 2; setup
reports and setup specifications retain schema 1. See the
[migration guide](../migrations/domain-schema-2.md).
