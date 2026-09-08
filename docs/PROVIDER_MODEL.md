# Provider contract

Adapters normalize provider records and preserve provenance; core owns correlation. Each adapter supplies metadata with explicit read capabilities, async health check, and async discovery returning a bounded per-provider snapshot. A boxed Send future supports dynamic dispatch without an async-trait dependency. Health checks do not enumerate the access graph. Built-in providers must not detach background tasks. Supervised external operations
receive cancellation explicitly and are awaited through process cleanup; do not
cancel them by dropping their future or wrapping them in a dropping timeout.

The application caps concurrent provider executions at four and supplies an overall deadline. GitHub additionally bounds individual requests, retries, body size and page count. Snapshot collection is buffered initially, with hard limits to prevent unbounded responses; the separate native external host validates bounded streaming events. Failed endpoints retain successfully observed data and add completeness warnings where safe.

GitHub scope: organization members/owners, organization repositories, teams and memberships, team-to-repository roles, collaborator access observations. Collaborator observations must never be falsely labeled direct; GitHub can include inherited access. Organization owners retain their organization role, while repository access remains observed from repository APIs. Custom roles retain provider names and unknown normalized privilege unless standard semantics are proven. Public email is never treated as verified.

External provider protocol is specified in [provider-protocol.md](provider-protocol.md). Pure versioned discovery/health validation is implemented in `permesh-provider-protocol`.
`permesh-provider-external` runs only managed native copies: draft 1 for standalone
discovery and draft 2 for approved workspace operations. Approval binds full
configuration and registration before secret resolution; credentials are sent
only after an exact handshake. External identities require explicitly approved
source selection and registered identity capability. No empty official AWS/Cloudflare crates are created. Google supplies only directory accounts and identities; it does not invent access grants. See [Google scope](providers/google.md).
