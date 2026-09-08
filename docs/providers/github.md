# GitHub provider

The GitHub adapter is maintained and distributed separately in
[permesh-providers](https://github.com/NIPE-Solutions/permesh-providers).
Its [provider documentation](https://github.com/NIPE-Solutions/permesh-providers/blob/main/docs/github.md)
covers supported requests, token permissions, identity semantics, visibility,
request budgets and adapter tests.

The CLI no longer includes a GitHub HTTP adapter. Follow the explicit
[install, trust and setup workflow](../getting-started.md#connect-github), then
review and approve the workspace before health checks or queries. Installing a
package never implicitly trusts it. The external host enforces its own bounded
protocol and execution deadlines.

Existing `type: github` configurations remain readable for
[explicit migration](../github-migration.md). Legacy instances produce migration
guidance before credential access or network operations. In mixed workspaces,
available providers can still return useful partial results.
