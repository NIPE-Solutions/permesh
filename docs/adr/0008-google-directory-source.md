# 0008: Google Workspace as an explicit directory source

Status: accepted

Use the Admin SDK Directory users endpoint as the first real identity source.
Keep it separate from Google resource-access discovery: accounts and identities
are the only advertised capabilities. Authority requires a source declaration.
No orphan verdicts are introduced with this adapter.

Authenticate using externally supplied OAuth access tokens through existing secret
references. This adds no credential library, refresh-token storage or project
OAuth backend. It does require users to supply renewed tokens. A native OAuth
flow can follow as a separate authentication feature with its own threat review.

Require a customer ID, read all visible domains within that customer, and reject
records for another customer. Canonical IDs combine the Google customer and
immutable user ID; primary email is a label and exact directory-attested matching
address. Do not infer employee status, human kind or identity from aliases and
recovery fields. Missing status evidence remains unknown; suspension and archival
indicate inactive account state, not terminated employment.

Keep HTTP pagination and quota handling in the adapter. Reuse the workspace's
existing reqwest/Tokio/serialization dependencies and provider snapshot contract.
No Google SDK dependency is warranted for a single GET endpoint. An explicit
field mask minimizes collected data. Provider tests use a local mock server,
including partial failures and unsafe responses; real tenant qualification must
be documented before claiming supported production deployment.

References and operational limitations are in [the provider guide](../providers/google.md).
