# Negotiate explicit, approved provider network context

Status: accepted for prerelease discovery and health.

Clearing external environments prevents accidental credential leakage, but also
removes enterprise proxy and CA settings. Restoring inherited environment values
would make Git-reviewed configuration incomplete and could disclose unrelated
credentials. Instead, instances declare network configuration explicitly.

Workspace config stores a proxy, bounded bypass list and optional relative CA
file with an exact SHA-256 pin. Approval binds these fields. The host checks the
CA bytes before resolving credentials and transmits those bytes in a validated
SDK context rather than exposing local filesystem paths. Config, transport and
presentation DTOs remain separate. Existing instances omit network entirely,
preserving their serialized approval context.

Negotiated v1 uses an optional requested/acknowledged `features` list with
`network_v1`. Features are distinct from operations and record capabilities.
Without an explicit request, existing handshake bytes remain unchanged. A host
with configured networking requires acknowledgement before sending an invocation.
There is no fallback to direct networking. This keeps compatible extension
negotiation separate from wire major versions; older providers can reject the
opt-in request without changing existing behavior.

Initial controls exclude proxy authentication, ambient configuration, CIDR bypass
rules, custom endpoints and TLS-verification bypasses. Adapters opt into the
feature only when every relevant HTTP client, including token refresh, uses it.
AWS and host browser OAuth must reject networking until their own transports
support it. This is explicit incomplete coverage, not a silent direct connection.

The SDK performs bounded syntax/PEM-framing validation; TLS implementations parse
actual certificates and retain hostname verification and default roots. The
native process remains trusted code with the user's OS permissions. Proxy and CA
controls cannot constrain a malicious approved executable. See [networking](../networking.md).
