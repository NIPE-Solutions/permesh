# Providers

The provider boundary is read-only. Adapters supply capabilities, health and
normalized observations; core owns identity matching and graph traversal.
Provider output goes through the host's validated protocol.

| Provider | Scope | Availability |
| --- | --- | --- |
| Demo | Synthetic accounts, authoritative identities, resources, groups, memberships and grants; no secrets or network | Bundled; `init --demo` |
| Inventory | Explicit pinned JSON identity inventory with declared scope, provenance and freshness; no network | Built into current source; [inventory contract](identity-inventory.md), absent from published alpha.2 |
| GitHub | Organization/repository observations, teams, memberships and observed grants; no claim of complete effective authorization | External 0.1.0 published for all five native targets |
| Google | Customer-wide directory accounts and optionally authoritative identities; no resource grants or groups | External 0.2.0 source candidate; refresh/browser support implemented, no package published |
| Cloudflare | Account-access observations with conservative policy and visibility handling | External 0.2.0 is an unpublished candidate held for qualification, absent from catalog |
| AWS IAM | IAM users, roles, groups and policy attachments in one commercial account; no effective-permission or administrator analysis | External 0.2.0 is an unpublished candidate; live and release qualification remain separate gates |
| GitLab | Reviewed groups/projects, direct membership and collapsed inherited/shared access observations with native levels | External 0.2.0 source candidate; no package published |
| Microsoft Entra ID | Verified tenant-bound directory accounts/identities, groups, direct memberships and optional service principals; no Azure RBAC or PIM | External 0.2.0 source candidate; no package published |
| AWS Identity Center | Explicit instance/store and account scope; groups, provisioned permission sets and assignments, optionally scoped Organizations names | External 0.2.0 source candidate; no effective-permission claim or package published |

Official external source is maintained in
[permesh-providers](https://github.com/NIPE-Solutions/permesh-providers).
The seven service-provider binaries have negotiated-v1 0.2.0 source candidates.
The original GitHub, Google, Cloudflare and AWS IAM candidates completed earlier
offline qualification in [provider PR 13](https://github.com/NIPE-Solutions/permesh-providers/pull/13).
New adapters have separate candidate checks; see the
[qualification matrix](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/qualification.md).
They are not published releases or installable catalog entries. GitHub 0.1.0
remains the only published package. Follow the [GitHub guide](providers/github.md)
and [Google identity source guide](providers/google.md) for setup and limitations.

The external Google provider supports access tokens, refresh tokens and a
provider-declared browser flow with host-owned PKCE and callback handling.
Browser login requires a configured OAuth client, trusted binary, approved
workspace and a same-instance keychain destination. It does not provide OAuth
client registration or qualify a live tenant automatically. Legacy `type: google`
configurations require [explicit migration](google-migration.md). Service APIs
are accessed through external providers; demo and pinned inventory reads are local.

AWS providers accept explicit `access_key_id`, `secret_access_key` and
`session_token` slots as required by their declared configuration. Current host
source also supports an [explicit temporary AWS profile](aws-profiles.md), read
only after approval and bound to the configured account/role. There is no ambient
credential chain, implicit SSO cache, process helper or metadata fallback. Refresh
temporary sessions outside Permesh. The IAM adapter records policy attachments;
the separate Identity Center adapter records scoped permission-set assignments.
Neither evaluates effective AWS permissions or proves resource-level authorization.

Configure multiple instances by assigning different IDs. A GitHub instance may
list multiple organizations. `provider list` shows configured instances without
contacting services; `provider capabilities INSTANCE` needs no credentials.
`provider status [INSTANCE]` and `doctor` resolve the explicitly configured credentials and perform
lightweight health checks. Successful health does not guarantee discovery
visibility.

`user`, `admins` and `orphaned` discover configured providers, validate their
graphs and correlate identities. Requests are concurrent across at most four
instances and bounded by time and record budgets. Failed or partial providers
remain explicit in both output formats.

[Package installation and updates](provider-packages.md) download explicitly
selected releases. Downloading does not trust or execute code. [Native trust and
workspace approval](external-providers.md) are separate decisions;
[declarative setup](provider-setup.md) does not approve execution. Dynamic
API-driven setup remains future work. See [provider development](provider-development.md)
and the [roadmap](ROADMAP.md) for extension scope and release gates.
