# Permesh product scope

**Know who has access to what.**

Permesh is a local-first, read-only CLI for inspecting, correlating, explaining, and auditing access metadata. It has no backend, accounts, telemetry, background update checks, remote configuration, or persistent access database. Access queries contact only explicitly configured provider APIs. Explicit package installation and update commands also contact GitHub for the official catalog and release artifacts.

The first implementation milestone delivers `init --demo`, `doctor`, `provider list/status/capabilities`, `user <identity>`, `admins`, `orphaned` and versioned JSON, plus GitHub access discovery and a Google Workspace directory source. Environment and native keychain references keep credentials out of shared YAML. Demo data is synthetic and requires no network. GitHub is an observed-access adapter, not a claim of complete effective authorization. A release remains gated on platform CI and a credentialed acceptance exercise.

Success means a new contributor can run the four demo commands without credentials, explain a team-derived grant, observe partial failures in both output formats, and reproduce tests without Internet access after dependencies are installed.

Native external discovery and health are available through local binary trust
and separate exact workspace approval. Approved providers participate in ordinary
queries and can receive reviewed named credential slots. Explicit catalog
installation and updates are available; downloading, binary trust and workspace
approval remain separate decisions. Interpreted-provider trust is unsupported.
Declarative setup uses typed provider-owned questions with CLI-owned prompts and
does not grant workspace execution approval.

Google browser authentication and the external Google, Cloudflare and AWS source
implementations extend this foundation; live qualification and package publication
remain separate gates. AWS currently inventories IAM policy attachments using
named credentials, without native profile/SSO loading or effective-policy evaluation.
No mutation APIs, policy engine, web UI, provisioning, HR system, SSO, password
manager, SIEM, billing, cloud synchronization, AI matching, or feature gates
belong in this milestone.
