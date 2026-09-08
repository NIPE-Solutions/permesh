# 4. Separate secrets from configuration

Status: accepted

## Decision and consequences

Only env://NAME and keychain://INSTANCE/token are supported. Native keyring stores credentials. Debug is redacted, values zeroize on drop, errors never include raw backend text. exec:// is not supported. Cost: headless Linux users may need environment references.

## Subsequent extension

[ADR 0010](0010-approved-external-workspace-invocations.md) adds exact local
workspace approval, draft-2 health/discovery and named credential slots while
preserving the original standalone draft-1 boundary and built-in token handling.
