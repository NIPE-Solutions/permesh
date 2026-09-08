# Credentials

Workspace configuration accepts `env://NAME` and
`keychain://instance-id/credential-name`. Built-in providers continue to use the
`token` account only. Approved external instances can declare named credential
slots; the keychain service must match the containing instance and the account
must equal the slot name. Literal credentials, executable references, URL
parameters and encoded paths are rejected.

Environment names use ASCII letters, digits and underscore, beginning with a
letter or underscore. Provider IDs use ASCII letters, digits, hyphen and
underscore. A reference is a locator; it is safe to commit a locator, but never
commit its resolved value. Environment values are read literally, without shell
execution or interpolation.

Native entries use service `permesh:<instance-id>` and the selected credential
slot as account (`token` for built-in providers). Keyring
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
account assignments are rejected. Workspace references alone do not authorize
external execution: matching local binary trust and workspace approval are
required before resolving an external instance's credentials.

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

Keychain names are shared at user level: the same instance ID and slot in two
workspaces resolve the same `permesh:<id>` service/account pair. Choose distinct
instance IDs for unrelated credentials. Login explicitly replaces that entry;
it does not establish tenant-specific isolation.

## Named external credentials

Declare references under `external.credentials`, for example
`token: keychain://internal-main/token` or
`tenant_key: env://PERMESH_TENANT_KEY`. Slot names begin with an ASCII letter,
use letters, digits, hyphens or underscores, and are at most 64 bytes.

```sh
permesh auth login internal-main --credential token
permesh auth status
permesh auth logout internal-main --credential token
```

For noninteractive login, use `--token-stdin` to read the selected slot's value
from stdin; never pass it as a command argument. Environment-backed slots are
managed outside Permesh. `auth status` checks named credential availability;
it does not authenticate against the provider. Explicit login/logout manage local
credentials, not workspace execution approval or remote token revocation.

Workspace discovery and health require a matching approval before external
credential resolution. The host verifies the draft-2 handshake's exact version,
provider ID and capability set before sending configuration and credentials on
stdin. It clears the child environment and supplies no credential arguments.
Each credential is bounded to 16 KiB, all credentials to 64 KiB and 16 slots;
the serialized operation frame zeroizes on drop.

Raw stderr is discarded and response strings/keys reflecting supplied credential
values are rejected after JSON decoding. These controls reduce accidental echo;
a trusted executable can still encode, save or send credentials elsewhere. Review
its source, endpoint settings and authority before approving it. Changing a
reference invalidates approval; rotating the value behind an unchanged reference
does not.

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

[Provider setup](provider-setup.md) accepts references for credential fields,
validates their instance and slot binding, and writes those references into the
workspace. It does not resolve environment values, retrieve keychain values or
store new credentials. Plaintext values in credential fields are rejected after
the provider's field schema is known. Other settings must also be nonsecret;
provider labels cannot safely classify an arbitrary pasted value. Use the explicit
`auth login` workflow separately when a keychain credential needs storing.
