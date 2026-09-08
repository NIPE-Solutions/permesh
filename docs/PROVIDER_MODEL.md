# Provider contract

Adapters normalize provider records and preserve provenance; core owns correlation. Each adapter supplies metadata with explicit read capabilities, async health check, and async discovery returning a bounded per-provider snapshot. A boxed Send future supports dynamic dispatch without an async-trait dependency. Health checks do not enumerate the access graph. Cancellation drops the future; providers must not detach background tasks.

The application caps concurrent provider executions at four and supplies an overall deadline. GitHub additionally bounds individual requests, retries, body size and page count. Snapshot collection is buffered initially, with hard limits to prevent unbounded responses; streaming events remain the external protocol direction. Failed endpoints retain successfully observed data and add completeness warnings where safe.

GitHub scope: organization members/owners, organization repositories, teams and memberships, team-to-repository roles, collaborator access observations. Collaborator observations must never be falsely labeled direct; GitHub can include inherited access. Organization owners retain their organization role, while repository access remains observed from repository APIs. Custom roles retain provider names and unknown normalized privilege unless standard semantics are proven. Public email is never treated as verified.

External provider protocol is specified in [provider-protocol.md](provider-protocol.md). Offline discovery validation is implemented in `permesh-provider-protocol`; executable loading is deferred. No empty official AWS/Cloudflare crates are created. Google supplies only directory accounts and identities; it does not invent access grants. See [Google scope](providers/google.md).
