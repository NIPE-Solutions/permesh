# Temporary AWS profiles

This source feature is unreleased. It supplies one approved AWS provider operation
from one explicitly selected temporary shared-credentials profile. It does not
load the AWS default credential chain or execute the AWS CLI.

Use it when your existing authentication process already writes temporary STS
credentials to a private shared-credentials file. Permesh reads that file locally,
selects one named section, and delivers only its access key ID, secret access key
and session token to the approved native provider. It neither copies the session
into another credential store nor changes your AWS files.

## Configuration

Add the source to the external provider instance's configuration. Supply your
actual provider registration, reviewed binary pin and AWS settings; this fragment
only illustrates the authentication fields:

```yaml
external:
  configuration:
    account_id: '123456789012'
    region: eu-west-1
    caller_role: AccessReview
  aws_profile:
    version: 1
    credentials_file: /Users/alice/.aws/credentials
    profile: access-review
```

Use an explicit absolute path for your operating system. On Windows, for example,
use a single-quoted YAML path such as `'C:\Users\alice\.aws\credentials'`.
Do not commit credential files. A machine-specific configuration selected with
`--config` can keep local paths separate from shared workspace definitions.

The selected section must contain exactly the three AWS shared-credentials keys:
`aws_access_key_id`, `aws_secret_access_key`, and `aws_session_token`. Access keys
must begin with `ASIA`, the AWS STS temporary-key prefix. Obtain the values through
your existing approved AWS authentication process. Permesh never asks you to
create a long-lived access key for this source.

`caller_role` is the role name in the STS assumed-role caller ARN, not an IAM role
path or an inferred display name. The adapter verifies the configured account and
role before discovery. An account or role mismatch fails authentication. Use an
AWS adapter candidate that supports `caller_role`; older binaries may reject this
setting. Configuring the source does not install or update the provider package.

Review and approve the instance after changing this configuration. Review shows
the source path, profile, target account and role without reading the credentials.
Normal queries require valid executable trust and workspace approval before the
file is opened. File, profile, account, role or network configuration changes
invalidate the corresponding approval. Rotating the selected session's values
does not require a new approval.

## Supported behavior and limits

- One exact named profile; no default-profile fallback or environment overrides.
- Temporary credentials only; all three fields are required and read together.
- UTF-8 shared-credentials syntax with named sections, `key = value` fields, and
  whole-line `#` or `;` comments. No quoted values, interpolation or continuations.
- Duplicate selected sections or keys, unknown selected fields and malformed input
  are rejected. Other profiles are not resolved or executed.
- Absolute regular files only, with a 1 MiB input bound. Symbolic links and Windows
  reparse points are rejected. Unix files must have no group/other permission bits.
  On Windows, protect the existing file with your user ACL; Permesh does not change
  its ACL. Same-user filesystem replacement races cannot be eliminated by portable
  path checks.
- `auth status` reports this source as configured but unverified without reading
  it. `doctor` checks it through an approved provider operation.

AWS `config` files, `credential_process`, `source_profile`, automatic role
assumption, web identity, instance/container metadata, SSO cache discovery and
browser login are not supported by this source. Unsupported selected fields fail
explicitly. Permesh does not run helper commands or inherit the shell's AWS
settings as a fallback.

## Refresh and expiry

Refresh the same named profile using your existing authentication process before
running Permesh. Prefer an atomic replacement of the credential file so a reader
cannot observe a half-written session. Each new provider operation reads the
selected profile once; it never switches credentials during that operation.
Expired sessions fail through the AWS API. Shared-credentials files do not carry
a standardized expiry field, so local status cannot prove that a session is valid.

Revoking or replacing the AWS session remains an AWS-side action. Removing this
configuration prevents future profile delivery but does not revoke a session.

## References

AWS documents the [shared-credentials format](https://docs.aws.amazon.com/cli/latest/topic/config-vars.html)
and the [STS temporary access-key prefix](https://docs.aws.amazon.com/STS/latest/APIReference/API_GetAccessKeyInfo.html).
The [process credential provider](https://docs.aws.amazon.com/sdkref/latest/guide/feature-process-credentials.html)
executes commands, which is why it is outside this source's supported subset.
