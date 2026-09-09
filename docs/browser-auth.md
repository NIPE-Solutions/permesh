# Browser authentication

`permesh auth login INSTANCE --browser` runs an explicitly requested, local OAuth
login for an external provider that declares browser authentication. Ordinary
queries, setup, and credential status never open a browser or save OAuth tokens.
Use `--browser --no-open` to print the authorization URL on stderr and open it
manually. The URL contains a random state and PKCE challenge, not a credential or
the PKCE verifier. JSON command results remain on stdout.

First configure the provider's required OAuth client ID and authentication mode,
its client-secret reference when required, and its refresh-token reference. The
refresh destination must be exactly `keychain://INSTANCE/SLOT`, where `SLOT` is
the provider's declared refresh slot. Browser login cannot write environment
variables, other instances' credentials, or arbitrary credential names. Missing
OAuth client details must come from your own registered application; Permesh
does not supply a shared client or use a backend.

Review and approve the provider instance in this workspace before login:

```sh
permesh provider external review INSTANCE
permesh provider external approve INSTANCE --fingerprint REVIEWED_FINGERPRINT --accept-risk
permesh auth login INSTANCE --browser
```

Trust and workspace approval bind the exact provider executable and complete
configuration. Both are checked before executing the description exchange or
resolving any credential. The host resolves only the declared client secret;
the refresh credential need not exist yet. The provider subprocess receives no
configuration, credentials, authorization codes, state, or token response.
After success, the host reloads the workspace and checks approval and executable
integrity again before replacing the one configured refresh credential.

The host uses authorization code flow with a fresh 256-bit state, PKCE S256, and
an ephemeral listener bound only to `127.0.0.1` on an OS-selected port. It accepts
only `/oauth/callback` requests with the expected Host and unique matching state,
rejects duplicate or unknown query fields and callback overrides, and bounds
headers, bodies, attempts and deadlines. The browser flow has a five-minute
deadline; the token exchange has a fifteen-second deadline. HTTPS token requests
never follow redirects. Only declared scopes are requested; a token response
reporting additional scopes is rejected. Authorization and token endpoints must
be HTTPS domain URLs on port 443 without user information, queries or fragments.

Only a successful response containing a refresh token can reach native keychain
storage. Access tokens, authorization codes and PKCE verifiers are ephemeral;
owned token and request buffers are zeroized when dropped. HTTP/TLS libraries may
retain internal transport copies until their buffers are released. Errors and
Debug output never include token response bodies or credential values.
Cancellation or timeout before the final storage commit drops the flow and
stores nothing. The final native keychain call is synchronous and cannot be
undone once started; it is never queued as a background write after cancellation.

macOS uses the system `open` executable; Linux uses `/usr/bin/xdg-open` with
`BROWSER` removed and a neutral working directory. Windows uses the system
`explorer.exe` from `SystemRoot`. URLs are separate native arguments, never shell
commands. Use `--no-open` if the desktop opener is unavailable.

Provider authors declare this capability using optional [protocol draft 4](provider-protocol.md).
Discovery uses the explicitly selected legacy or negotiated contract; setup remains draft 3; package catalog protocol
lists continue to describe those existing operations so older CLIs can install
providers without using browser login.

The flow follows [OAuth for native apps (RFC 8252)](https://www.rfc-editor.org/rfc/rfc8252)
and [PKCE (RFC 7636)](https://www.rfc-editor.org/rfc/rfc7636).
Google desktop applications support the loopback flow described in its
[installed-app OAuth documentation](https://developers.google.com/identity/protocols/oauth2/native-app).
Live OAuth consent and native keychain writes require real application credentials
and an explicit login; automated tests use local synthetic endpoints only.
