# Release status and upgrade notes

This page holds release, catalog-contract and upgrade detail that used to live in
the README. For what is stable, see the release notes; for planned work, see the
[roadmap](ROADMAP.md).

## Current release

**Release status:** Permesh is alpha software. The published CLI is
[0.1.0-alpha.3](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.3).
It is an evaluation prerelease and includes the current identity semantics,
JSON schema 2 and local review workflows. See the [release notes](releases/0.1.0-alpha.3.md)
and [migration guide](migrations/domain-schema-2.md).

Installation instructions cover checksums and build provenance; platform signing
remains a release limitation.

The snapshot, diff, offboarding, resource and policy commands are evaluation
features in the published `0.1.0-alpha.3` prerelease.

## Providers

An implemented provider is not automatically a qualified release. AWS attachments
and Cloudflare assignments do not establish effective privilege. Review
[coverage, limitations, and availability](providers.md) before relying on a report.

All seven service providers have catalog-installable 0.2.0 evaluation releases;
GitHub 0.1.0 also remains available.

Current source supports guided `provider add` for all seven providers. The
published alpha.3 supports it for GitHub, Google, Cloudflare and AWS IAM; with
alpha.3, GitLab, Entra and AWS Identity Center use `provider install` followed by
the explicit external trust, setup and approval flow.

## Upgrading from alpha.2

Upgrade the CLI to alpha.3 before using the new provider catalog contract. Alpha.2
rejects entries containing `discovery_protocol`, even when an older provider
version is requested. Upgrading the CLI does not change installed provider pins,
and existing binary trust remains stored. Alpha.2 legacy approval records also
remain stored, but alpha.3's scoped fingerprint v2 does not accept them: review
and explicitly approve each configured external provider once before querying.
