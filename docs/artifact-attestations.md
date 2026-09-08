# Verify native candidate provenance

The manual [Attested native candidates](../.github/workflows/attested-candidates.yml)
workflow builds the reviewed `main` revision on all five native targets, then
signs provenance for the resulting archives, dependency inventories and checksum
files. This is separate from Apple Developer ID signing, Apple notarization and
Windows Authenticode. The executables remain unsigned by those platform systems.
No release, registry or package-manager publication is performed.

This workflow must complete successfully before describing any resulting artifact
as having verified signed provenance. Adding the workflow alone is not evidence
that a signature exists. Previous unsigned candidate runs and published releases
are not retroactively attested.

## Maintainer qualification

After reviewing the exact source revision and its required checks, dispatch from
`main` with its full commit SHA:

```sh
gh workflow run attested-candidates.yml --repo NIPE-Solutions/permesh --ref main \
  -f reviewed_sha=REVIEWED_40_CHARACTER_COMMIT_SHA
```

The workflow fails unless its repository, branch and actual source SHA match the
expected values. The reused native build/test workflow retains `contents: read`
and has no signing permission. Only a separate downstream hosted job can request
an OIDC certificate and write attestations. It cannot write repository contents,
releases or packages. No pull-request job receives additional permissions.

That job downloads only candidate artifacts from the same run, keeps the five
artifact directories separate to detect unexpected or colliding inputs, and
requires all 20 expected files. Before signing, it validates checksums, exact
archive member allowlists, regular files, target/version identity, the archived
binary digest and the reviewed lockfile digest. It never extracts or executes a
downloaded binary. Unix lockfile digests must match LF checkout bytes. Windows
may match either the same LF bytes or their exact CRLF transformation, reflecting
Git's platform checkout conversion; no other lockfile content is accepted.

The signed SLSA provenance subjects include all five archives, five inventories,
and ten checksum files. The inventory is a Permesh dependency inventory, not an
SPDX/CycloneDX SBOM; it is signed as a build artifact. The job uses the actual
Sigstore bundle to verify **every file** against the exact repository, signing
workflow identity, source and signer commit digests, `refs/heads/main`, the SLSA
v1 predicate type, and GitHub-hosted runners. Only successful verification enables
upload of `permesh-attestations-SHA/attestation-bundle.json`. The candidate files
remain in the five original `permesh-candidate-TARGET-SHA` artifacts.

Record the run URL, source SHA and artifact hashes when manually promoting those
exact bytes. For alpha.2, publishing the exact verified bundle alongside all 20 subjects and
checking public lookup after publication are mandatory; see the
[alpha.2 publication gates](releasing.md#additional-alpha2-publication-gates). A successful signature check does not complete the
remaining live-provider or platform acceptance gates.

## Public verification

Use a current [GitHub CLI](https://cli.github.com/manual/gh_attestation_verify)
with artifact-attestation support. Obtain the expected full source commit SHA
from the reviewed release notes. For each downloaded archive, inventory or
checksum file:

```sh
gh attestation verify DOWNLOADED_FILE \
  --repo NIPE-Solutions/permesh \
  --cert-identity https://github.com/NIPE-Solutions/permesh/.github/workflows/attested-candidates.yml@refs/heads/main \
  --source-ref refs/heads/main \
  --source-digest REVIEWED_40_CHARACTER_COMMIT_SHA \
  --signer-digest REVIEWED_40_CHARACTER_COMMIT_SHA \
  --predicate-type https://slsa.dev/provenance/v1 \
  --deny-self-hosted-runners
```

The CLI retrieves the attestation from the public repository. Follow its ordinary
GitHub authentication prompt if necessary; no maintainer or signing credentials
are needed for verification. To use a downloaded bundle, append
`--bundle attestation-bundle.json`. The bundle supplies the attestation but is
not itself a trust root. Verification may still need network access to obtain
Sigstore trust data. Do not substitute a custom trust root supplied beside an
untrusted binary or accept a different source SHA just to make verification pass.

The exact certificate identity above already pins the signing workflow and ref.
Do not also pass `--signer-workflow`: the current CLI makes those identity flags
mutually exclusive. A nonzero exit means verification failed; presence of a
bundle or matching checksum alone is insufficient.

For a full candidate set downloaded into a single flat directory, the repository
helper additionally inspects all archives and inventory bindings:

```sh
python3 scripts/verify_candidates.py DOWNLOADED_DIRECTORY \
  --version REVIEWED_VERSION --lockfile Cargo.lock \
  --source-sha REVIEWED_40_CHARACTER_COMMIT_SHA \
  --bundle attestation-bundle.json
```

Run the helper from a checkout of the reviewed revision so `Cargo.lock` is the
reviewed file. For native Actions download directories, add
`--artifact-run-sha REVIEWED_40_CHARACTER_COMMIT_SHA`. Without `--source-sha`, the
helper checks only candidate structure and hashes and explicitly reports that
signatures were not checked.

## Tool and permission review

GitHub's [artifact attestation documentation](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
permits attestations for public repositories on all current plans. Read-only API
inspection on 2026-09-08 confirmed that `NIPE-Solutions/permesh` is public, Actions
is enabled, and the authenticated maintainer has push/admin access. Actual OIDC
issuance and attestation creation still require a successful workflow run.

The workflow pins [actions/attest v4.2.2](https://github.com/actions/attest/releases/tag/v4.2.2)
to `1e69f48acb82d1966a394da916b4c1698aa569d6` and
[actions/download-artifact v8.0.1](https://github.com/actions/download-artifact/releases/tag/v8.0.1)
to `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c`, checked against the upstream tag
APIs on 2026-09-08. Both are MIT-licensed GitHub-maintained actions.
`actions/attest` is the upstream replacement for new uses of
`actions/attest-build-provenance`. Its storage-record and registry-push options are
explicitly disabled, so this file-based workflow does not request package or
artifact-metadata write permissions. These pins identify the reviewed versions;
they are not an independent audit of the actions or hosted runner images.

This workflow is not a claim of reproducible builds or SLSA Build Level 3. It
trusts the reviewed workflow, the native jobs in the same run, GitHub's OIDC
identity, and Sigstore verification. It does not prove that the source or its
build dependencies are free of malicious behavior.

## Remaining platform-signing prerequisites

Platform signing has not been configured or performed. Before implementing it,
maintainers need the following in protected signing facilities, not in repository
files or build artifacts:

- **macOS:** Apple Developer Program membership, Team ID, a Developer ID
  Application signing identity with its private key and secure access method,
  and authorized notarization credentials (an App Store Connect API key with its
  key/issuer identifiers, or another supported notarytool credential profile).
  Select the notarized distribution container and supported stapling strategy;
  an installer package would additionally require a Developer ID Installer
  identity. See Apple's [Developer ID guidance](https://developer.apple.com/developer-id/)
  and [custom notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
- **Windows:** select an existing publicly trusted Authenticode signing
  certificate/service and secure key access, or provision an Azure Artifact
  Signing account with completed identity validation, a Public Trust certificate
  profile, account endpoint, and narrowly scoped signing identity. See Microsoft's
  [setup requirements](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)
  and [trust models](https://learn.microsoft.com/en-us/azure/artifact-signing/concept-trust-models).

Platform signing changes executable bytes. Any future signing path must finalize
platform signatures and notarization packaging before recording binary hashes,
archive hashes and provenance subjects, then verify those final distributed
bytes on the native platforms. Package-manager manifests must reference the
exact final promoted hashes. GitHub provenance does not remove Gatekeeper or
SmartScreen checks or establish Windows reputation.
