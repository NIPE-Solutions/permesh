# Explicit native external providers

Permesh runs explicitly trusted native providers for standalone discovery or,
after a separate local workspace approval, ordinary access queries and health
checks. This is an expert-facing draft interface. Workspace YAML references a
registered provider ID and pinned digest; it cannot name an executable, shell
command or interpreter. Nothing is downloaded or installed automatically.

## Review and register

Obtain a self-contained native binary from a source you trust. Review its code,
build provenance and required permissions before registering it. Scripts, shell
commands, interpreter arguments and PATH lookup are unsupported. Use an absolute
path without symlink components.

```bash
permesh provider external inspect /absolute/path/my-provider

permesh provider external trust /absolute/path/my-provider \
  --id my-provider --sha256 REVIEWED_SHA256 \
  --capability accounts --capability resources --capability grants \
  --accept-risk

permesh provider external list
permesh provider external discover my-provider --instance internal-main --json
permesh provider external remove my-provider
```

Replace the ID and capabilities with those implemented by your binary. The ID
must match its handshake provider type exactly; its capability set must match
registration exactly. Available record capabilities are `accounts`, `identities`,
`resources`, `groups`, `memberships` and `grants`. Omitting all capabilities
registers an empty set, not unrestricted discovery. They describe records, not OS
permissions or proof of complete access visibility.

`inspect` hashes bytes without executing or trusting them. A matching hash
establishes byte identity, not safety or publisher identity. `trust` requires
`--accept-risk`, copies the reviewed bytes into local storage, and does not run
them. Updating the original file does not update the registered copy. Registration
never overwrites an existing ID: remove it and explicitly trust the replacement.
Discovery rechecks the stored digest before launch. Removal affects local files
only, not the provider service or its access grants.

## Approve a workspace instance

Trusting a binary does not authorize a cloned workspace to run it or supply
credentials. Add a reference to the registration, review the configuration and
approve the exact fingerprint shown by the review command:

```bash
permesh provider add external --id internal-main \
  --provider my-provider --sha256 REVIEWED_SHA256 \
  --setting endpoint=https://internal.example.com \
  --credential token=keychain://internal-main/token

permesh provider external review internal-main
permesh provider external approve internal-main \
  --fingerprint REVIEWED_FINGERPRINT --accept-risk
permesh auth login internal-main --credential token

permesh doctor
permesh provider status internal-main
permesh user alice@example.com --json
permesh admins --json
permesh orphaned --json

permesh provider external revoke internal-main
```

Use the reviewed binary digest and the review command's fingerprint in place of
the uppercase placeholders. `--setting KEY=VALUE` stores a string; edit YAML for
JSON-compatible objects, arrays, numbers or booleans. `--credential NAME=REF`
stores a locator, never the credential value. Repeat either flag for more entries.
See [configuration](CONFIGURATION.md) and [named credentials](secrets.md).

Review displays the parameters, credential references, source-authority settings
and approval fingerprint without resolving secrets or executing the provider.
The approval binds the canonical configuration-file path, instance ID, selected
provider configuration, its identity aliases/authority, and the registration's
ID, digest and capabilities. A clone at another path has no approval. Editing a setting, credential
reference, relevant alias or selected identity authority invalidates the approval;
review and approve again. Formatting-only changes that leave the normalized
configuration unchanged do not change the fingerprint. Credential rotation behind
an unchanged reference does not itself change the approval. Unrelated provider
and organization edits preserve approval. Previous whole-workspace approvals
require one new review under [ADR 0017](adr/0017-provider-scoped-workspace-approval.md).

Queries verify approval and the managed binary before resolving credentials or
launching that external instance. An unapproved or changed instance fails closed;
other providers can still produce explicitly incomplete results. `doctor` and
`provider status` use the health operation rather than enumerating the access
graph. `user`, `admins` and `orphaned` use normalized discovery records, retaining
the existing correlation, provenance, ambiguity and partial-result rules.

An external identity source is authoritative only when explicitly selected in
`identity.sources` and the reviewed registration declares `identities`.
`orphaned` still requires an authoritative source; a missing or incomplete source
leaves accounts unassessed. Review source claims carefully: approval is permission
to use that source, not independent verification of its assertions.

Revocation removes the local workspace permission. It does not revoke provider
credentials, delete the native registration or undo already observed results.
`review` and `approve` load the selected configuration; `revoke` uses its canonical
path and does not require valid configuration contents. Standalone `inspect`,
`trust`, `list`, `remove` and direct `discover` do not. Direct discovery uses draft
1 and transmits no workspace parameters or credentials. Workspace operations
default to draft 2 and never downgrade to draft 1. Providers implementing
[negotiated negotiated protocol 1](provider-protocol/negotiated-v1.md) can explicitly select
`external.discovery_protocol: negotiated_v1` for workspace discovery and health.
This change requires fresh workspace approval and appears in review output.
There is no automatic probe or fallback; setup/browser-auth and standalone
discovery retain their existing contracts.

## Local storage

Registration and workspace approvals are protected user-local state, outside
shared workspace configuration. Approval records store fingerprints and binding
metadata, not resolved credentials or access snapshots. Permesh uses the
platform directories selected through `etcetera`: Application
Support on macOS, the XDG data directory on Linux and Local AppData on Windows.
Each registration contains a schema-1 manifest with ID, SHA-256 and capabilities,
and a managed native executable. Commands show the storage path.

`PERMESH_DATA_DIR` explicitly overrides the base directory; it must be absolute.
The registry lives in its `providers` subdirectory; workspace permissions live in
the sibling `workspace-approvals` directory. Do not share this directory
between administrators. Listing an absent registry does not create it. Binary
inspection is limited to 128 MiB; manifests are limited to 16 KiB. Symlink storage
and unexpected registration files are rejected. Unix storage uses user ownership,
0700 directories and 0600 manifests; unsafe writable ancestors are rejected.
Windows creates protected DACLs granting the current user and SYSTEM access,
validates ownership and permissions, and rejects reparse points or filesystems
without persistent ACLs. Unrecognized ACL forms fail closed.

An unsafe storage error means the directory cannot meet these checks. Choose a
private user-local directory; do not broaden its permissions to bypass validation.

## Execution boundary

**Trust grants permission to execute code as your user. It is not a sandbox.**
A provider can read local files, use the network and change state independently
of the read-only protocol. Review external code accordingly. Permesh neither
attests its behavior nor verifies the truth of its identity assertions.

The host starts the managed binary without command arguments, in a temporary
working directory and with a cleared environment (Windows may retain system
runtime configuration). Draft-1 direct discovery receives no workspace settings
or credentials. Approved discovery/health operations receive only that instance's
`configuration` values and resolved named credentials through the dedicated stdin
pipe, after the provider ID, version and capability set pass handshake validation.
Credentials are not placed in child environment variables or command arguments.
Surrounding workspace identity mappings and trust metadata are not sent to the child.
Interpreted-provider registration remains unsupported. OS libraries and the local
operating system remain trusted dependencies.

The request buffer zeroizes on drop. The host rejects decoded response strings
and keys that reflect supplied credential values, including JSON-escaped
spellings, and discards raw stderr. This reduces accidental output leakage;
it does not prevent trusted code from transforming, persisting or exfiltrating
credentials. Only supply credentials you intend this binary to receive.

The host validates the handshake before requesting discovery, enforces the
[wire budgets](provider-protocol.md), drains and discards diagnostic stderr,
and returns only a validated, deterministically sorted snapshot. Raw diagnostics
are never echoed, including errors. A valid partial snapshot is clearly marked
incomplete. Malformed exchanges do not return earlier records as a successful
snapshot. Reports can contain sensitive access metadata; protect redirected JSON
using your operating system's file permissions.

The handshake deadline is 5 seconds and the whole operation deadline is 60
seconds; frames do not reset these deadlines. Diagnostic stderr is limited to
64 KiB cumulatively. Cancellation allows 1 second for cooperation, followed by
a process-cleanup deadline of 5 seconds. Temporary-directory removal is a
separate filesystem operation.

Timeouts and cancellation request termination of the supervised process group
or Windows job and reap the direct child within the cleanup deadline. This also
happens after successful discovery to stop remaining descendants. Permesh does
not promise to await every descendant's OS exit: the process-wrap Windows job
waiter can return on an earlier job notification. Unix programs can deliberately escape their process group;
this is not containment for malicious code. A same-user attacker can also modify
local state or race a pathname between verification and execution. Digest checks
are not a defense against an already-compromised user account.

## Output and next steps

Human discovery output shows record counts and provider completeness. `--json`
returns the normalized snapshot, preserving provenance and inheritance. See
[output schema and exit codes](output-schema.md). The snapshot is discarded after
output; no access database is created.

Native authors should use the [provider development guide](provider-development.md).
The Python example remains a draft-1 offline reference and cannot be registered as
a native executable. [Declarative setup](provider-setup.md) can add an instance
using provider-owned questions without sending answers or granting workspace
approval. [Catalog installation and explicit updates](provider-packages.md) store packages without trusting them. Automatic updates and
interpreter/dependency-bundle trust remain deferred.

## Retained trusted versions

Explicitly trusting another digest for the same provider ID retains earlier
trusted binaries and selects the new digest for standalone discovery and setup.
Workspace queries resolve their exact digest pin, so their existing approvals
remain usable. A repeated trust of the same digest is rejected, including attempts
to change its capabilities. To reuse an already trusted older version, explicitly
review the workspace pin and configuration; downloading it again is not required.
`external remove ID` removes all retained trusted versions for that ID and makes
affected workspace approvals unusable. It does not remove downloaded packages.
