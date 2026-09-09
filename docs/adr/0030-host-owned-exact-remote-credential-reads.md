# ADR 0030: Host-owned exact remote credential reads

Status: accepted for the current-main source candidate; live deployment qualification pending separately for each backend.

## Context

Provider tokens may already live in an organization-managed secret store. Delegating generic secret-manager access to every native provider would expose bootstrap tokens and duplicate trust, network and error-handling logic. Running an arbitrary credential command would add another executable boundary. Configuration inspection must remain offline and existing workspace approvals must retain their historical representation.

## Decision

Extend pure secret references with three explicit named schemes. Resolve them only through versioned per-instance declarations for 1Password Connect, Vault KV v2 or OpenBao KV v2. The local secret resolver refuses remote references. Declarations contain only an HTTPS origin, exact locator/field, optional fixed KV version, explicit local bootstrap reference and optional reviewed proxy/CA settings.

Validate and approve every declaration before bootstrap resolution. Reuse the provider-scoped configuration fingerprint, native executable trust and final executable-pin check. Keep omitted resolver maps absent from legacy serialization. Split provider preparation into blocking trust/config/CA validation and asynchronous bounded GET operations. Never execute an arbitrary helper, follow redirects, inherit ambient proxies, enumerate secrets or fall back to another instance/backend.

The remote server returns a whole Connect item or KV object. Decode through bounded backend response models and deliver only the exact selected string. Bootstrap tokens stay host-side. Own zeroizing body/secret buffers, static errors and sensitive authentication headers; do not claim elimination of all copies made by dependencies. No resolved secret or raw response enters CLI reports or approval storage.

## Consequences

Remote stores become explicit additional network destinations visible before approval. Auth status reports configured/unverified without contacting them. Login/logout remain local-keychain operations and reject remote delivery slots. TLS, proxy, bootstrap provisioning and minimum read permissions remain operator responsibilities. The configured GET constrains this implementation but cannot reduce the server-side scope of the supplied token.

Fixtures test Connect identity/field matching and the separately dispatched Vault/OpenBao KV v2 subsets. They do not qualify a live product version. Namespaces, leases, renewals, generic password-manager access and recursive resolution remain unsupported. See [remote credential configuration and qualification limits](../remote-credentials.md).
