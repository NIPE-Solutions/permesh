#!/bin/sh
# SPDX-License-Identifier: MIT
# Prepare a fresh CI runner to query the providers committed in permesh.yaml.
#
# 1. Install each official package listed in ci/providers.txt (catalog checksums
#    verified) and trust that exact executable on this runner.
# 2. Approve each configured instance. Review and approval only succeed when the
#    digest pinned in the committed permesh.yaml matches the trusted executable,
#    so CI never runs code the repository did not pin.
#
# Never edits permesh.yaml, never stores tokens, and stops at the first failure.
# Requires: permesh and jq on PATH.
set -eu

providers_file="${1:-ci/providers.txt}"

grep -v '^[[:space:]]*\(#\|$\)' "$providers_file" | while read -r provider version; do
  installed=$(permesh provider install "$provider" --version "$version" --json)
  executable=$(printf '%s' "$installed" | jq -er '.result.package.executable')
  digest=$(printf '%s' "$installed" | jq -er '.result.package.release.executable_sha256')
  capabilities=$(printf '%s' "$installed" | jq -er '.result.package.release.capabilities[]' | sed 's/^/--capability /' | tr '\n' ' ')
  # shellcheck disable=SC2086 # capability flags are intentionally split
  permesh provider external trust --id "$provider" --sha256 "$digest" $capabilities --accept-risk "$executable" --json >/dev/null
  echo "Trusted $provider $version ($digest)"
done

permesh provider list --json | jq -er '.result.providers[] | select(.provider != null) | .id' | while read -r instance; do
  if ! fingerprint=$(permesh provider external review "$instance" --json | jq -er '.result.fingerprint'); then
    echo "Cannot approve $instance: the digest pinned in permesh.yaml does not match a trusted package." >&2
    echo "Check the version in $providers_file against the pins written by provider add --portable." >&2
    exit 1
  fi
  permesh provider external approve "$instance" --fingerprint "$fingerprint" --accept-risk --json >/dev/null
  echo "Approved $instance"
done
