# Permesh documentation

Permesh answers who has access, what role they have, and which observed path
connects them to a resource. Choose a starting point below.

## First time here

1. [Install Permesh](installation.md): choose your platform and verify the download.
2. [Try the demo and connect GitHub](getting-started.md): see useful results before granting API access.
3. [Understand identity matching](identity-resolution.md): connect a provider login to a canonical identity safely.
4. [Check provider coverage](providers.md): know what is available and what each adapter can prove.

Documentation on `main` follows current source. Published alpha.2 has earlier
contracts; use its [release notes](releases/0.1.0-alpha.2.md) and the
[upgrade notes](migrations/domain-schema-2.md) when comparing versions.

## Everyday access questions

| Task | Guide |
| --- | --- |
| Understand accounts, identities, resources and paths | [Concepts](concepts.md) |
| Find privileged accounts and unknown privilege | [Admins](admins.md) |
| Review inactive, unmatched and non-human accounts | [Orphaned](orphaned.md) |
| Review and save stable identity mappings | [Identity mapping](identity-mapping.md) |
| Use a maintained identity roster | [Pinned identity inventory](identity-inventory.md) |
| Process reports in scripts | [JSON schemas and exit codes](output-schema.md) |
| Diagnose authentication or provider failures | [Troubleshooting](troubleshooting.md) |
| Set up command completion | [Shell completion](completion.md) |

## Connect and operate providers

- [Provider setup](provider-setup.md): guided official setup and automation answers.
- [Packages and updates](provider-packages.md): installation, versions and explicit adoption.
- [External provider trust](external-providers.md): third-party binaries and workspace approval.
- [GitHub](providers/github.md) and [Google Workspace](providers/google.md).
- [Official provider catalog and guides](https://github.com/NIPE-Solutions/permesh-providers): Cloudflare, AWS, release status and contributions.
- [Networking](networking.md): explicit proxies and additional certificate authorities.

## Use Permesh as a team

- [Team workflows](team-workflows.md): shared Git configuration, local trust and portable pins.
- [Configuration](CONFIGURATION.md): strict schema, workspace discovery and identity sources.
- [Secret storage](secrets.md): environment references, keychain storage, rotation and removal.
- [Privacy](privacy.md), [security](security.md) and [threat model](threat-model.md).
- [GitHub migration](github-migration.md), [Google migration](google-migration.md) and [domain/schema migration](migrations/domain-schema-2.md).

## Build and extend

- [Native scaffold and offline conformance](provider-development/tooling.md).
- [Contribution guide](../CONTRIBUTING.md) and [provider development](provider-development.md).
- [Normalization rules](provider-development/normalization.md) and [protocol compatibility](provider-protocol/versioning.md).
- [Architecture](ARCHITECTURE.md), [domain model](DOMAIN_MODEL.md) and [provider model](PROVIDER_MODEL.md).
- [Roadmap](ROADMAP.md), [release qualification](releasing.md) and [architecture audit](audits/architecture-hardening.md).
