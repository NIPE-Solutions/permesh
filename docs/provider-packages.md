# Install and update provider packages

The official [provider repository](https://github.com/NIPE-Solutions/permesh-providers)
hosts a static catalog and independently versioned native releases. The catalog is
currently empty: the CLI distribution flow is implemented, but no provider package
is advertised until it passes release qualification. Existing bundled providers
remain available. These commands require no workspace.

```bash
permesh provider install github --version VERSION
permesh provider update github --check
permesh provider update github
permesh provider update github --version VERSION
```

Replace `VERSION` with an exact published stable semantic version. Initial
installation requires `--version`. Ordinary update selects the newest compatible
stable version and never silently downgrades. `--version` permits an intentional
older version. Prerelease versions, build metadata and version ranges are not
accepted. Update requires an existing downloaded package for the current platform.

`--check` fetches metadata only and does not create local state. Finding an update
returns success (0), with `update_available` in JSON. Combining `--check` with
`--version` inspects that exact release without downloading it. Network/catalog/
integrity failures use exit 3, invalid requests or unavailable compatible releases
use 2, and local storage failures use 5. Ctrl+C during a download returns 130.

## Download is separate from trust

Installation verifies the archive and executable digests, validates the platform
header and stores the executable and license in private local package storage.
It never runs code, prompts through a provider, reads credentials, rewrites a
workspace or grants execution trust. The command reports the executable path,
release metadata and digest. Review the release source, capabilities and digest,
then explicitly register it using the existing trust command:

```bash
permesh provider external trust /ABSOLUTE/PACKAGE/EXECUTABLE \
  --id github --sha256 REVIEWED_EXECUTABLE_SHA256 \
  --capability accounts --capability resources --capability grants --accept-risk
```

The capabilities above are illustrative; specify exactly the reviewed release's
capabilities. Missing or extra capabilities fail the subsequent handshake.
Then run `permesh provider setup github --id github-main` for a package that
supports setup protocol 3. Review and approve workspace execution separately.

Downloaded versions remain side by side. The update baseline is the greatest
installed version for the current platform, not an implicit workspace selection.
Downloading an older version makes it available for an explicit rollback; it
does not change that baseline or any workspace pin. Already installed matching
packages are verified and returned without re-downloading. Changed metadata or
digests for an installed version are rejected.

Trusting a new digest for the same provider ID retains older trusted digests.
Existing workspaces continue using their exact pin and approval. Adopting a new
version requires an explicit configuration change and renewed workspace review.
No configuration migrations or garbage collection run automatically. Package
installation and trust storage are distinct; removing a trust registration does
not delete downloaded archives' extracted files.

## Catalog and package format

Catalog schema 1 contains `schema_version` and `releases`. Each release contains:

| Field | Meaning |
| --- | --- |
| `provider`, `version`, `target` | Unique provider/version/platform coordinate |
| `capabilities` | Exact discovery capabilities advertised by the binary |
| `protocols` | Supported protocol drafts; 2 required, 3 optional |
| `archive_sha256`, `executable_sha256` | Lowercase SHA-256 digests |
| `archive_size` | Exact compressed byte count |

Provider IDs start with a lowercase ASCII letter and contain lowercase letters,
digits, hyphens or underscores, up to 32 bytes. Catalogs are bounded to 1 MiB and
4,096 releases. Unknown/duplicate fields and duplicate coordinates are errors.
Supported targets are macOS x86_64/ARM64, Linux GNU x86_64/ARM64 and Windows MSVC
x86_64. Other ABIs are explicitly unsupported.

The catalog is fetched from the fixed official repository's `main/catalog/v1.json`.
Each asset is derived as `github.com/NIPE-Solutions/permesh-providers/releases/download/PROVIDER-vVERSION/permesh-provider-PROVIDER-VERSION-TARGET.zip`.
Catalog content cannot introduce an arbitrary download host or executable command.
There is no workspace-provided catalog URL.

ZIP packages contain exactly `provider` (or `provider.exe` on Windows) and `LICENSE`.
The license file must include all notices required by the distributed executable.
Archive downloads and executable contents are bounded to 128 MiB; the license is
bounded to 1 MiB. The ZIP profile rejects ZIP64, extra fields, archive comments, prefixes and trailing records.
The installer also rejects extra entries, duplicates, links,
nonregular files, unsafe paths and mismatched digests. Header checks validate the
declared architecture and executable format; they do not prove loadability or
publisher identity. Staging and publication do not replace existing versions.

## Network and trust limits

Catalog requests have a 30-second total deadline, archives 120 seconds including
redirects and body reads. Archive redirects are restricted to HTTPS on port 443
at `github.com`, `release-assets.githubusercontent.com` and
`objects.githubusercontent.com`, with at most five hops. Catalog redirects are
rejected. Authentication headers, environment proxies, automatic retries and
content decompression are disabled for distribution requests.

Checksums establish correspondence with the fetched catalog. They do not establish
independent publisher authenticity if the repository and artifacts are both
compromised. Artifact attestations/signatures are not verified by this initial
client. Review repository ownership and release provenance before trusting code.
Provider binaries run as the user and are not sandboxed.

No background update checks exist. Access queries, doctor, authentication and
setup never contact the catalog. Installation sends no access data or credentials
to GitHub; see [privacy](privacy.md). Local same-user malicious processes remain
outside the filesystem race protections, as with existing trust storage.
