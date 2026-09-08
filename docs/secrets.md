# Credentials

Workspace configuration accepts only `env://NAME` and
`keychain://provider-id/token`. Literal credentials, executable references,
URL parameters, encoded paths and alternate accounts are rejected. Keychain
references must match the containing provider ID.

Environment names use ASCII letters, digits and underscore, beginning with a
letter or underscore. Provider IDs use ASCII letters, digits, hyphen and
underscore. A reference is a locator; it is safe to commit a locator, but never
commit its resolved value. Environment values are read literally, without shell
execution or interpolation.

Native entries use service `permesh:<provider-id>` and account `token`. Keyring
4's native adapter selects macOS Keychain, Windows Credential Manager, or Secret
Service on Linux. A working user session and unlocked credential store may be
required. Reading configuration does not contact the credential store. Resolving
a reference reads credentials; only explicit authentication commands should call
the library's `store` and `delete` operations. No fallback to plaintext files is
implemented. Deleting an absent entry reports a curated credential-store error.

Resolved secrets use `secrecy::SecretString`, which zeroizes its backing storage
on drop. They have a redacted `Debug` representation and no serialization
implementation. The `expose()` method is the explicit boundary for passing a
token to the HTTP client or native store. OS libraries, environment storage and
HTTP implementations can retain their own copies; zeroization does not promise
to erase those copies or protect a compromised process.

Errors deliberately discard raw parser, environment and credential-store
messages. Workspace YAML is limited to 1 MiB, one document, 16 levels, 30,000
nodes and 100,000 parser events. Anchors, aliases, merge keys, unsupported tags
and includes are rejected. Properties and filesystem includes are not enabled.
Duplicate map keys, provider IDs, organizations, identity sources and explicit
account assignments are rejected. No workspace setting starts a process.

Package tests use temporary directories and a child process with an isolated
environment. They do not create, read or delete real native credentials. Native
credential-store integration remains a platform release check.

A separate manual macOS Apple Silicon exercise verified synthetic credential
storage, resolution, independent OS retrieval and deletion for revision
`9ad1504`. The temporary entry was removed afterward. Windows Credential Manager
and Linux Secret Service still require native integration qualification; see
[release evidence](releasing.md#qualification-evidence-2026-09-08).

Configuration loading rejects nonregular files before opening and checks the
opened file again. A process concurrently replacing filesystem entries can still
race these checks; workspace contents must not be concurrently manipulated by an
untrusted local process. Parser diagnostics retain only numeric line/column
locations, never source excerpts.

Ctrl+C stops waiting and exits; a native keychain write already in progress may have completed. Check `auth status` before retrying interrupted login/logout. No rollback is promised for native operations.

Keychain names are shared at user level: the same provider instance ID in two workspaces resolves the same `permesh:<id>` entry. Choose distinct instance IDs for unrelated credentials. Login explicitly replaces that entry; it does not establish tenant-specific isolation.

## Native integration checks

The `native` integration tests are ignored by default because they access the
OS credential store. To explicitly run a synthetic round trip on an unlocked
store:

```sh
cargo test -p permesh-secrets --test native --locked -- --ignored --exact native_round_trip
```

The test uses a unique entry, checks retrieval and replacement without printing
values, and verifies deletion. A cleanup guard also attempts deletion on failure.
It never uses provider credentials or makes provider API requests.

CI runs this test on Windows Credential Manager and on Linux with a temporary
D-Bus session and unlocked GNOME Keyring. The Linux setup follows the
[GNOME Keyring daemon options](https://manpages.debian.org/unstable/gnome-keyring/gnome-keyring-daemon.1.en.html)
and [D-Bus session lifecycle](https://dbus.freedesktop.org/doc/dbus-run-session.1.html).
A separate process points at a nonexistent session bus to test redacted service
unavailability. Desktop prompts and locked-store interactions still require
manual checks.
