# Providers

The provider boundary is read-only. Adapters supply capabilities, health and
normalized observations; core owns identity matching and graph traversal.
Provider output goes through the host's validated protocol.

| Provider | Scope | Availability |
| --- | --- | --- |
| Demo | Synthetic accounts, authoritative identities, resources, groups, memberships and grants; no secrets or network | Bundled; `init --demo` |
| GitHub | Organization/repository observations, teams, memberships and observed grants; no claim of complete effective authorization | External 0.1.0 published for all five native targets |
| Google | Customer-wide directory accounts and optionally authoritative identities; no resource grants or groups | Bundled access-token adapter; external refresh/browser source implemented, live tenant qualification pending |
| Cloudflare | Account-access observations with conservative policy and visibility handling | External source implemented; draft packages held for live qualification, absent from catalog |
| AWS | IAM users, roles, groups and policy attachments in one commercial account; no effective-permission or administrator analysis | External source implemented; live and native release qualification pending |

Official external source is maintained in
[permesh-providers](https://github.com/NIPE-Solutions/permesh-providers).
Google and Cloudflare drafts are not published releases or installable catalog
entries. Follow the [GitHub guide](providers/github.md) and [Google identity
source guide](providers/google.md) for the supported configuration paths.

The external Google provider supports access tokens, refresh tokens and a
provider-declared browser flow with host-owned PKCE and callback handling.
Browser login requires a configured OAuth client, trusted binary, approved
workspace and a same-instance keychain destination. It does not provide OAuth
client registration or qualify a live tenant automatically. The bundled Google
adapter continues to use externally supplied access tokens.

AWS uses named `access_key_id`, `secret_access_key` and optional `session_token`
references. It does not load ambient profiles, shared credential files, SSO
caches, process credentials or instance metadata. Refresh temporary credentials
outside Permesh. Its graph records policy attachments, not policy evaluation,
role-assumption paths, Identity Center or access to individual AWS resources.

Configure multiple instances by assigning different IDs. A GitHub instance may
list multiple organizations. `provider list` shows configured instances without
contacting services; `provider capabilities INSTANCE` needs no credentials.
`provider status [INSTANCE]` and `doctor` resolve local secrets and perform
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
