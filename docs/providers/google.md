# Google Workspace directory

The Google provider reads directory identities. It does not enumerate groups,
application permissions, Drive sharing, Google Cloud IAM, or effective access.
This adapter is covered by mock API tests; live tenant qualification is pending.

The implementation lives in the official
[provider repository](https://github.com/NIPE-Solutions/permesh-providers/tree/main/providers/google).
The 0.2.0 source candidate has passed offline native qualification, but no Google
package is published in the catalog yet. Legacy `type: google` configurations
are parsed for migration only; queries and authentication reject them before
reading credentials. See [migration](../google-migration.md).

## Permissions and authentication

The required scope is `https://www.googleapis.com/auth/admin.directory.user.readonly`
with customer-directory visibility for the authenticating administrator. The
external provider supports access tokens, refresh credentials and a declarative
browser flow using host-owned PKCE. There is no Permesh OAuth application or
credential exchange backend. See the provider-owned
[least-privilege setup, OAuth client requirements and revocation guide](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/google.md).

## Configuration

Once a qualified package is published, `permesh provider add google` uses the
same guided installation and explicit consent flow as GitHub. Until then, this
command reports that a compatible published release is unavailable.

For a reviewed, separately built native candidate, trust its executable digest
and exact `accounts`/`identities` capabilities, then configure it explicitly:

```sh
permesh provider setup google --id google-main \
  --discovery-protocol negotiated-v1 --authoritative
permesh provider external review google-main
```

Setup prompts for the stable customer ID, authentication mode and credential
references. It does not fetch directory data or grant execution approval. Review
and approve the workspace context before authentication or queries. For scripts,
use `--answers FILE`; see [declarative setup](../provider-setup.md).

Authority is explicit. Without the source declaration, Google accounts are still
available for lookup, but their directory identity statuses are not used as an
authoritative source. Multiple provider instances are supported. Keep credential
values in environment variables or the OS keychain, never in shared configuration.

## Identity semantics

Account keys use the provider instance and immutable Google user ID. Canonical
identity IDs are `google:CUSTOMER_ID:USER_ID`; email renames do not create a new
canonical identity. Two instances observing the same customer/user share that ID;
conflicting status observations remain ambiguous.

Only `primaryEmail` is used as a directory-attested address. This is the address
assigned to that managed account, not proof of a person's identity or employment.
Other email fields, aliases, recovery addresses, names, and custom fields are not
fetched or used for correlation. Exact matching remains case-sensitive. Address
reassignment remains a limitation of email-based correlation; prefer explicit
immutable account mappings for security-sensitive joins.

To join a GitHub account, which does not expose verified email through this
adapter, map its numeric ID to the Google canonical ID:

```yaml
identity:
  sources:
    - provider: google-main
      authoritative: true
  aliases:
    "google:C01234567:123456789":
      github-main: ["987654321"]
```

Both provider instances must be configured. An alias alone carries unknown status
if the directory identity could not be discovered. A failed authority cannot
establish that somebody is inactive.

- `archived: true` maps to `inactive`, including when also suspended.
- Otherwise `suspended: true` maps to `suspended`.
- Both fields explicitly false map to `active`.
- Missing status fields map to `unknown`; malformed values mark discovery incomplete.
- Identity and account kind/affiliation remain `unknown`: the directory does not establish human,
  contractor, bot, or service-account intent.

Active means the observed account is neither suspended nor archived. It is not an
employment assertion or a guarantee that sign-in is possible. Deleted users are
not enumerated. A reused email does not reuse the immutable Google identity ID.

## Discovery and limits

Permesh sends GET requests only to
`https://admin.googleapis.com/admin/directory/v1/users`. Redirects are disabled.
Requests use the configured customer, administrator view, basic projection, and a
field mask for immutable IDs, customer ID, primary email and status fields.
No mailbox, document, password, or recovery content is requested.

Discovery follows opaque page tokens on that same endpoint. Repeated tokens,
duplicate IDs, wrong-customer records, malformed responses, and interrupted
collection must not appear as a complete directory. Successful earlier pages are
retained when later pages fail. Provider limitations are present in JSON and human
reports; completeness is bounded by the credential's visibility.

`doctor` performs one bounded directory request rather than fetching the full
identity list. A successful health check does not prove exhaustive directory
visibility. Expired tokens produce a sanitized authentication error; denied
permissions identify the read-only scope and admin access requirement.

The [users.list contract](https://developers.google.com/workspace/admin/directory/reference/rest/v1/users/list),
[User fields](https://developers.google.com/workspace/admin/directory/reference/rest/v1/users),
and [rate-limit guidance](https://developers.google.com/workspace/admin/directory/v1/limits)
are the provider references. Collection remains subject to Permesh's overall
60-second external-host deadline (the native runtime limits each operation to 55 seconds). Individual requests have a 5-second connect timeout and a 15-second total
response timeout. Discovery allows 500 rows per page, 200 pages, 100,000 rows,
2 MiB per response, and page tokens up to 4,096 bytes. Requests are sequential.
Quota responses and server errors allow at most two retries, with exponential
backoff and jitter. `Retry-After` is honored up to five seconds; a longer delay
ends collection with a quota error instead of retrying early. These are defensive
limits, not scale guarantees.

## External provider migration

An existing Google instance must be converted explicitly using a reviewed and trusted external executable. See [Google migration](../google-migration.md). Conversion preserves authority and token references; it does not authenticate or approve execution.
