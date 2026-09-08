# Explicit native external providers

Permesh can supervise one explicitly trusted native provider for discovery.
This is an expert-facing draft interface. Ordinary `user`, `admins`, `orphaned`
and `doctor` commands do not execute external programs, and workspace YAML cannot
select an executable. Nothing is downloaded or installed automatically.

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

## Local storage

Registration is user-local, outside workspace configuration. Permesh uses the
platform data directory selected by `directories::ProjectDirs`: Application
Support on macOS, the XDG data directory on Linux and Local AppData on Windows.
Each registration contains a schema-1 manifest with ID, SHA-256 and capabilities,
and a managed native executable. Commands show the storage path.

`PERMESH_DATA_DIR` explicitly overrides the base directory; it must be absolute.
The registry lives in its `providers` subdirectory. Do not share this directory
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
runtime configuration). No workspace contents, provider configuration or resolved
credentials are transmitted. Credential transport and interpreted-provider
registration remain unsupported. OS libraries and the local operating system
remain trusted dependencies.

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

External discovery does not yet feed ordinary identity correlation or source
authority. Native authors should use the [provider development guide](provider-development.md).
The Python example is an offline protocol reference and cannot be registered as
a native executable.
