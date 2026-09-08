# Google Workspace directory

The Google provider reads directory identities. It does not enumerate groups,
application permissions, Drive sharing, Google Cloud IAM, or effective access.
This adapter is covered by mock API tests; live tenant qualification is pending.

This page describes the bundled `type: google` adapter. Native Google packages
use their own pinned SDK and wire contract; see [migration](../google-migration.md).

## Permissions and authentication

Enable the Admin SDK API in an organization-controlled Google Cloud project.
Use an OAuth access token authorized for:

```text
https://www.googleapis.com/auth/admin.directory.user.readonly
```

The authenticating administrator also needs permission to read the customer-wide
user directory. The scope grants API access; it does not bypass administrator
role restrictions. A restricted admin view can limit visibility.

Obtain a token through your organization's existing OAuth tooling using a
[Google-supported authorization flow](https://developers.google.com/identity/protocols/oauth2).
An API key, ID token, refresh token, service-account JSON key, and GitHub token
are not substitutes for an OAuth access token. Permesh does not implement Google
browser login, token refresh, Application Default Credentials, or service-account
impersonation in this version. Use your existing tooling to renew expired tokens.
There is no Permesh OAuth application or credential exchange service.

For local keychain storage, `permesh auth login google-main` prompts for the access
token without echo. Prefer an environment reference for short-lived tokens; inject
the value through your secret tooling, not shell command arguments. Permesh never
writes resolved credentials into configuration.

Revoke authorization through the account's Google app-access settings or your
organization's existing OAuth administration process. `auth logout` only removes
the local keychain entry; it does not revoke Google authorization.

## Configuration

Find the immutable customer ID in your Google Admin console. Use the explicit
`C...` ID, not a domain name or the credential-dependent `my_customer` alias.

```sh
permesh provider add google --id google-main --customer-id C01234567 \
  --token-ref env://PERMESH_GOOGLE_TOKEN --authoritative
permesh doctor
permesh user alice@example.com --json
```

Equivalent configuration:

```yaml
version: 1
organization:
  name: Example
providers:
  - id: google-main
    type: google
    customer_id: C01234567
    auth:
      token: env://PERMESH_GOOGLE_TOKEN
identity:
  sources:
    - provider: google-main
      authoritative: true
```

Authority is explicit. Without the source declaration, Google accounts are still
available for lookup, but their directory identity statuses are not used as an
authoritative source. Multiple provider instances are supported.

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
120-second provider deadline. Individual requests have a 5-second connect timeout and a 15-second total
response timeout. Discovery allows 500 rows per page, 200 pages, 100,000 rows,
2 MiB per response, and page tokens up to 4,096 bytes. Requests are sequential.
Quota responses and server errors allow at most two retries, with exponential
backoff and jitter. `Retry-After` is honored up to five seconds; a longer delay
ends collection with a quota error instead of retrying early. These are defensive
limits, not scale guarantees.

## External provider migration

An existing Google instance can be converted explicitly once the separate Google executable is qualified and trusted. See [Google migration](../google-migration.md). Conversion preserves authority and token references; it does not authenticate or approve execution.
