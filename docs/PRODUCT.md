# Permesh product scope

**Know who has access to what.**

Permesh is a local-first, read-only CLI for inspecting, correlating, explaining, and auditing access metadata. It has no backend, accounts, telemetry, background update checks, remote configuration, or persistent access database. Access queries contact only explicitly configured provider APIs. Explicit package installation and update commands also contact GitHub for the official catalog and release artifacts.

The first implementation milestone delivers `init --demo`, `doctor`, `provider list/status/capabilities`, `user <identity>`, `admins`, `orphaned` and versioned JSON, plus GitHub access discovery and a Google Workspace directory source. Environment and native keychain references keep credentials out of shared YAML. Demo data is synthetic and requires no network. GitHub is an observed-access adapter, not a claim of complete effective authorization. Stable qualification remains gated on broader platform and credentialed acceptance.

Success means a new contributor can run the four demo commands without credentials, explain a team-derived grant, observe partial failures in both output formats, and reproduce tests without Internet access after dependencies are installed.

Native external discovery and health are available through local binary trust
and separate exact workspace approval. Approved providers participate in ordinary
queries and can receive reviewed named credential slots. Explicit catalog
installation and updates are available; downloading, binary trust and workspace
approval remain separate decisions. Interpreted-provider trust is unsupported.
Declarative setup uses typed provider-owned questions with CLI-owned prompts and
does not grant workspace execution approval.

CLI [0.1.0-alpha.3](releases/0.1.0-alpha.3.md) publishes five native targets with
verified same-run GitHub provenance, dependency inventories and checksums.
Executables have no platform code signatures or notarization; the inventory is
not a standards SBOM. Live browser/tenant and interactive desktop qualification
remain open.

Google browser authentication and the seven catalog-installable external provider
0.2.0 evaluation releases extend this foundation. GitHub 0.1.0 remains available
as a retained legacy catalog release.
Alpha.2 cannot parse the new negotiated-discovery catalog metadata and must be
upgraded before using the current catalog; CLI replacement leaves installed pins unchanged.
Existing trust and legacy approval records remain stored, but alpha.3 requires a
fresh provider review and explicit scoped approval before execution.
Alpha.3 package installation supports all seven providers, while guided
`provider add` supports GitHub, Google, Cloudflare and AWS IAM. GitLab, Entra and
Identity Center require the explicit external trust/setup path after installation.
AWS currently inventories IAM policy attachments using
named credentials, without native profile/SSO loading or effective-policy evaluation.
No mutation APIs, policy engine, web UI, provisioning, HR system, SSO, password
manager, SIEM, billing, cloud synchronization, AI matching, or feature gates
belong in this milestone.
