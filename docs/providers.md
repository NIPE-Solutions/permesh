# Providers

The provider boundary is read-only. Adapters supply capabilities, health and normalized observations; core owns identity matching and graph traversal. Adapters never print directly to the terminal.

- **demo**: accounts, authoritative synthetic identities, resources, groups, memberships, grants. No secrets and no network. `init --demo` configures it as the identity authority.
- **github**: accounts, organization/repository resources, teams, memberships, observed access grants. GitHub.com only; no public-email verification, HR status, session inspection, credentials inventory, or mutations. [Dedicated guide](providers/github.md).

- **google**: customer-wide directory accounts and optionally authoritative identities, using an OAuth access-token reference. No Google resource grants or groups. [Dedicated guide](providers/google.md).

Configure multiple instances by assigning different IDs. A single GitHub instance may list multiple organizations. `provider list` shows configured instances without contacting services. `provider capabilities INSTANCE` also needs no credentials. `provider status [INSTANCE]` and `doctor` resolve local secrets and make lightweight provider health checks; successful health does not guarantee permission to every discovery endpoint.

`user` and `admins` discover each configured provider, validates its graph, correlates identities, and returns access paths. Requests are concurrent across at most four provider instances, sequential within each adapter, and bounded by time and record budgets. A failing provider appears explicitly in both output formats.

AWS and Cloudflare built-in adapters are not implemented. Native external
providers can be registered locally and, after exact workspace approval, used for
health and ordinary access queries. See [external providers](external-providers.md).
[Declarative setup](provider-setup.md) supports registered native providers. Provider catalogs, downloads and dynamic provider-driven steps remain deferred. See [provider development](provider-development.md) for extension design and [roadmap](ROADMAP.md) for release gates.
