# Explicit provider networking

A provider instance can opt into a reviewed HTTPS proxy and additional CA
certificates. Permesh never copies shell proxy variables, certificate variables,
or arbitrary environment values into providers. Network context travels in the
bounded invocation only after the provider negotiates `network_v1` support.
No protocol version increment is needed for this optional feature.

```yaml
external:
  provider: github
  sha256: REVIEWED_EXECUTABLE_SHA256
  discovery_protocol: negotiated_v1
  network:
    https_proxy: http://proxy.example.com:8080
    no_proxy:
      - .internal.example.com
    ca_bundle:
      path: certificates/company-ca.pem
      sha256: REVIEWED_CA_FILE_SHA256
```

This is a field excerpt, not a complete workspace. Settings are per instance,
so unrelated providers do not acquire the same proxy or certificate authority.
Add only the settings you need. A CA-only configuration does not require a proxy.
An empty network object and a bypass list without a proxy are rejected.

## Review and trust

Keep public CA certificates in a workspace-relative PEM file. Compute the file's
SHA-256 with your existing checksum tool and review both the file and digest in
Git. Paths must remain inside the workspace; absolute paths, traversal, Windows
prefixes, and backslash separators are rejected. Symlinks escaping the workspace
are rejected. The host reads bounded bytes, verifies the exact digest, and checks
certificate-only PEM framing before resolving provider credentials. Private-key
blocks and arbitrary file content are not accepted. The provider's TLS client
must additionally parse the certificates.

Run `permesh provider external review INSTANCE` after editing the configuration,
then approve the new fingerprint. Proxy, bypass, CA path and digest are part of
the instance's approval context. Changing certificate bytes without updating the
pin fails; updating the pin requires fresh approval. Already prepared invocations
carry the checked bytes, so providers do not reopen a mutable local CA path.
Review output includes settings and the file pin, never the PEM contents.

Adding a CA broadens trust for this provider's TLS connections. Configure only
an authority your organization intends to trust. It does not disable hostname
verification or replace the default trusted roots. A proxy is another explicit
network recipient; your organization's TLS interception policy still applies.

## Supported syntax and limits

- `https_proxy`: an HTTP or HTTPS proxy URL, at most 2,048 ASCII bytes. Credentials,
  URL query/fragment, non-root paths, zero ports, SOCKS and TLS bypasses are rejected.
  HTTP here describes the connection to the proxy; provider API traffic is HTTPS.
- `no_proxy`: at most 64 unique DNS names, leading-dot domain suffixes, IP addresses
  or `*`, each at most 253 bytes. CIDR ranges, ports, URL syntax, whitespace and
  comma-separated values are not supported. Use a YAML list.
- `ca_bundle`: at most 256 KiB and 64 certificate PEM blocks. Use the exact digest
  of the file bytes, including line endings. A shared CA file should retain stable
  line endings in Git. Certificate parsing failures stop the provider before API use.

There is no proxy credential backend, automatic certificate discovery, inherited
`NO_PROXY`, custom provider endpoint, or `insecure` option in this slice.

## Provider and operation compatibility

Networking requires negotiated protocol v1 and the optional `network_v1` feature.
The host requests it in the public handshake; the response must agree before
credentials or CA bytes are delivered. An older or unsupported provider fails
with an actionable negotiation error; it is never retried with direct networking.
Legacy discovery rejects network context before process launch.

Official GitHub, Google and Cloudflare networking adapters are being qualified in
the provider repository. Do not infer support from the provider name or from an
older published package. AWS transport support is a separate follow-up; it must
reject this feature until implemented. See [provider availability](providers.md).

The host-owned browser OAuth flow currently rejects instances with explicit
network settings before launching a provider or opening an authorization flow.
Use supported access-token or provider refresh-token configuration until host
OAuth networking is implemented. Do not remove a required proxy merely to bypass
this safeguard. Provider refresh exchanges must use the same approved context as
their discovery API requests.

Package downloads and catalog access have a separate fixed distribution network
policy; this configuration does not redirect or change their trust roots.

## Implementation references

The SDK validates a non-secret, filesystem-free transport context. Config uses
an independent file-reference DTO, and output uses an independent review DTO.
The host resolves pinned certificate bytes; adapters configure their own HTTP
clients without inheriting process state. Reqwest supports [explicit proxies](https://docs.rs/reqwest/0.13.4/reqwest/struct.Proxy.html)
and [custom root certificates](https://docs.rs/reqwest/0.13.4/reqwest/struct.ClientBuilder.html).
These are transport controls, not a sandbox for trusted native executables.
