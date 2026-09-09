# Privacy

Permesh is designed to run locally without a backend, account registration, telemetry, background update checks, remote configuration, or persistent access database. Demo data is synthetic. Configured real providers require network requests to their APIs; those providers can log the token identity, request metadata, client IP address, and timing according to their own policies.

Configuration stores provider settings and secret references. Environment and native credential stores hold secret values outside shared YAML. During collection, resolved credentials and access metadata exist in process memory. Redacted formatting and zeroization reduce accidental disclosure; they do not protect against a compromised operating system, debugger, swap, crash dump, or malicious dependency.

Human and JSON results can include identities, memberships, repository names, permissions, and access paths. Treat them as sensitive organizational data. Permesh does not provide a report vault or retention service. Shell redirection, terminal scrollback, CI logging, screen recording, backups, and downstream tools determine who can read saved output and how long it remains available. File export is future functionality; stdout is under the caller's control.

Do not attach real organization reports or raw error bodies to issues. Reproduce with demo data, omit secrets, and review every field before sharing. Revoking a token is done through its issuer; removing a keychain entry only removes that local copy. There is no project-operated service from which to request deletion of a discovery database.

Explicit `provider install` and `provider update` commands contact GitHub for the
official static release catalog and, when requested, native package archives.
`update --check` downloads catalog metadata only. Requests contain no workspace configuration, identities, access reports or
credentials. GitHub receives ordinary
connection metadata and the selected release URL; its retention policies apply.
The downloader disables environment proxies and sends no authentication headers.
Queries, doctor, setup and authentication never check the catalog or download
updates. There is no scheduled updater. See [provider packages](provider-packages.md).

Build and development tools have their own network behavior: Cargo downloads dependencies, and security checks fetch advisory information. That development activity is distinct from CLI runtime behavior.

Explicitly trusted external providers are separate native programs. Standalone
draft-1 discovery receives no workspace settings or credentials. Locally approved
draft-2 workspace operations receive the selected instance's configuration and
resolved named credentials on stdin after handshake validation. The complete
workspace configuration is used to bind the local approval but is not transmitted
to the provider. Approval storage contains binding metadata and fingerprints,
not resolved credentials or access reports; raw child stderr is not retained.

These programs execute with your operating-system authority and can independently
read files, retain received credentials or contact services. Response-reflection
checks are not an exfiltration barrier. The project's no-backend guarantee does
not attest third-party code; review its behavior and requested credential slots
before granting binary trust and workspace approval.
See [external-provider security boundaries](external-providers.md).

During [provider setup](provider-setup.md), draft-3 description receives an instance
ID but no answers, configuration, credential references or values, or workspace
and answer-file paths. The CLI evaluates answers locally and stores only settings
and references in the workspace. Setup does not read or write the credential
store. Answer files must contain nonsecret settings and references; arbitrary
text fields are not a secret vault. The trusted binary still runs as your user
and can access resources independently of the host-supplied protocol context.


Explicit snapshots contain sensitive access metadata and identity mappings. They
are saved only by a requested export, using private files where the platform
supports them. No copy is sent to the project or added to Git. Offline inspection
and comparison make no network requests. Delete local exports and backups according
to your organization's retention needs; Permesh has no central copy to delete.
See [snapshot storage and comparison](snapshots.md).
