# Carry verified discovery compatibility into provider setup

Status: accepted, pre-stability.

Official 0.2.0 provider candidates emit richer negotiated-v1 observations and
reject experimental configured discovery. A package declaring legacy discovery
would be misleading, while probing and silently falling back would obscure the
execution contract the user approved.

Release metadata therefore has an optional `discovery_protocol: negotiated_v1`.
Missing means legacy and remains omitted on serialization. Negotiated releases
list only setup draft 3 in `protocols`; mixed discovery-family claims fail closed.
This is an intentional additive catalog contract that older strict clients reject.
No existing published entry or executable is rewritten.

Guided add carries the verified release contract into the configuration it asks
the user to approve. Standalone setup and legacy migration expose an explicit
selector for separately trusted binaries, defaulting to legacy. The selector,
digest and provider configuration remain bound to workspace approval. Updating
an installed version never silently updates an existing workspace.

Google now follows the same external-only boundary as GitHub. Its native adapter
already lives in the providers repository and has offline qualification. The
remaining bundled implementation is removed; legacy parsing is retained solely
for an explicit migration preserving IDs, authority, aliases and secret references.
Requests fail before credentials for unmigrated instances. Demo remains bundled.

This intentionally advances source ahead of Google package publication. The
release catalog still lacks Google; docs identify the reviewed-build requirement
until compatible artifacts pass live and distribution qualification. Keeping a
second production adapter would prolong semantic drift and duplicate security
maintenance. This change does not authorize publication or weaken trust gates.
