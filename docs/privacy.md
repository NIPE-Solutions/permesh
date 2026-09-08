# Privacy

Permesh is designed to run locally without a backend, account registration, telemetry, update checks, remote configuration, or persistent access database. Demo data is synthetic. Configured real providers require network requests to their APIs; those providers can log the token identity, request metadata, client IP address, and timing according to their own policies.

Configuration stores provider settings and secret references. Environment and native credential stores hold secret values outside shared YAML. During collection, resolved credentials and access metadata exist in process memory. Redacted formatting and zeroization reduce accidental disclosure; they do not protect against a compromised operating system, debugger, swap, crash dump, or malicious dependency.

Human and JSON results can include identities, memberships, repository names, permissions, and access paths. Treat them as sensitive organizational data. Permesh does not provide a report vault or retention service. Shell redirection, terminal scrollback, CI logging, screen recording, backups, and downstream tools determine who can read saved output and how long it remains available. File export is future functionality; stdout is under the caller's control.

Do not attach real organization reports or raw error bodies to issues. Reproduce with demo data, omit secrets, and review every field before sharing. Revoking a token is done through its issuer; removing a keychain entry only removes that local copy. There is no project-operated service from which to request deletion of a discovery database.

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
