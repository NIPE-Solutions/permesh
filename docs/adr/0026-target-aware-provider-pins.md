# Bind shared provider instances to explicit target digests

Status: accepted for prerelease configuration.

A single executable digest cannot identify independently built macOS, Linux and
Windows binaries. Add an optional `sha256_by_target` alternative to `sha256`,
with exactly one form required. Explicit keys make review and diagnostics clearer
than overloading a scalar field or implicitly consulting a mutable local registry.
No separate lockfile, target wildcard or execution override is introduced.

One SDK-owned vocabulary defines supported targets. Configuration parsing remains
platform-independent; execution selects only the running CLI target and fails
before secret resolution when absent. Scalar serialization remains unchanged.
Portable approval binds the complete map and selected target/digest in addition
to existing context. Foreign pin changes deliberately require re-approval too.

Guided portable onboarding selects one version from one validated catalog and
requires consistent contracts across its included targets. Only native artifacts
are downloaded, verified and explicitly trusted locally. Shared pins do not grant
local execution trust, and catalog digests do not become signature claims.
Installation/update and workspace adoption remain separate actions.

See [team workflows](../team-workflows.md) for migration, limitations and review.
