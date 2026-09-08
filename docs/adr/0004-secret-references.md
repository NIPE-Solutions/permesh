# 4. Separate secrets from configuration

Status: accepted

## Decision and consequences

Only env://NAME and keychain://INSTANCE/token are supported. Native keyring stores credentials. Debug is redacted, values zeroize on drop, errors never include raw backend text. exec:// is not supported. Cost: headless Linux users may need environment references.
