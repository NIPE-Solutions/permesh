# Install Permesh

Download an archive and its adjacent `.sha256` file from the [0.1.0-alpha.1 release](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.1). Choose your OS and CPU:

| System | Target | Archive |
| --- | --- | --- |
| macOS Apple Silicon | aarch64-apple-darwin | tar.gz |
| macOS Intel | x86_64-apple-darwin | tar.gz |
| Linux x86_64 | x86_64-unknown-linux-gnu | tar.gz |
| Linux ARM64 | aarch64-unknown-linux-gnu | tar.gz |
| Windows x86_64 | x86_64-pc-windows-msvc | zip |

These unsigned alpha binaries are for evaluation. Read the [limitations](releases/0.1.0-alpha.1.md), particularly OS compatibility and credential storage.

The current checkout prepares [0.1.0-alpha.2](releases/0.1.0-alpha.2.md).
Use the published release linked above until alpha.2 qualification and publication
are complete. Candidate builds are not installation releases. After publication,
select assets from the [release list](https://github.com/NIPE-Solutions/permesh/releases)
and verify the exact version stated on that release page.

## Verify before extracting

On macOS, replace `ARCHIVE` with the downloaded filename:

```sh
shasum -a 256 -c ARCHIVE.sha256
```

On Linux:

```sh
sha256sum -c ARCHIVE.sha256
```

On Windows PowerShell:

```powershell
(Get-FileHash .\ARCHIVE.zip -Algorithm SHA256).Hash
Get-Content .\ARCHIVE.zip.sha256
```

Compare the hash with the first field in the checksum file (case does not matter). Stop if it differs. Checksums downloaded alongside an archive detect corruption, not publisher impersonation.

Extract into a new directory using `tar -xzf ARCHIVE.tar.gz` or PowerShell `Expand-Archive .\ARCHIVE.zip -DestinationPath .\permesh-alpha`. Each archive contains `permesh` (Windows: `permesh.exe`), `LICENSE`, `THIRD-PARTY-NOTICES.txt`, and `INSTALL.txt`.

Run `./permesh --version` (Windows: `.\permesh.exe --version`) from that directory. The version must match the selected release (`0.1.0-alpha.1` for the published release linked above). You may copy the executable into a user-owned directory already on your PATH; no administrator privileges are needed. Keep license and dependency notices when redistributing. OS download protections may prompt or block unsigned programs; verify the source and follow your organization's policy rather than disabling protections globally.

Try the [offline demo](getting-started.md#evaluate-offline), then configure providers explicitly. Installation itself does not fetch provider data or store credentials.

## Alpha.2 candidate and release provenance

Alpha.2 is currently a release candidate. Do not infer publication or successful
attestation from this guide. Its eventual release page must identify the exact
reviewed source commit, successful same-run workflow and downloadable bundle.
Alpha.1 assets are historical and are not retroactively attested.

For an alpha.2 archive, inventory or checksum obtained from a qualified candidate
run or future published release, first verify the file with a current GitHub CLI:

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

Use the full reviewed source SHA recorded for that exact artifact set. Repeat
with `--bundle attestation-bundle.json` to verify the distributed bundle. The
bundle is mandatory for alpha.2 publication but is not itself a trust root;
Sigstore trust-data retrieval can still require network access. Stop on a nonzero
exit, unexpected identity or mismatched digest. A matching checksum alone is
insufficient. Continue with checksum verification and extraction only after
provenance verification succeeds.

To independently inspect a complete set, put exactly the five archives, five
inventories and ten checksum files in `candidate-files/`; keep the bundle outside
that directory. From a checkout of the recorded revision, run:

```sh
python3 scripts/verify_candidates.py candidate-files \
  --version 0.1.0-alpha.2 --lockfile Cargo.lock \
  --source-sha REVIEWED_40_CHARACTER_COMMIT_SHA \
  --bundle attestation-bundle.json
```

The helper checks all 20 subjects, archive contents and inventory bindings before
accepting signatures. For native Actions download directories, use the layout
and `--artifact-run-sha` option in [artifact attestations](artifact-attestations.md).
These checks establish signed build provenance, not Apple notarization, Windows
Authenticode, reproducible builds or safe source behavior.

## Upgrade or uninstall

To upgrade, download and verify the desired release and replace the executable while it is not running. There is no CLI self-update or background update check. `provider update` manages separate provider packages and does not update the CLI.

To uninstall, remove the executable. This preserves workspace files, exported reports, installed provider packages, local approvals and keychain credentials. Use `auth logout` for credentials you want to remove before uninstalling; see [secret storage](secrets.md) and [provider storage](provider-packages.md) for retained local data. Never delete shared credentials indiscriminately.

## Build from source

A checkout needs Rust 1.91 or newer:

```sh
cargo install --path crates/permesh-cli --locked
```

The project is not published to crates.io; `cargo install permesh` is not currently supported.
