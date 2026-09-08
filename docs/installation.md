# Install Permesh

Download an archive and its adjacent `.sha256` file from the [0.1.0-alpha.2 release](https://github.com/NIPE-Solutions/permesh/releases/tag/v0.1.0-alpha.2). Choose your OS and CPU:

| System | Target | Archive |
| --- | --- | --- |
| macOS Apple Silicon | aarch64-apple-darwin | tar.gz |
| macOS Intel | x86_64-apple-darwin | tar.gz |
| Linux x86_64 | x86_64-unknown-linux-gnu | tar.gz |
| Linux ARM64 | aarch64-unknown-linux-gnu | tar.gz |
| Windows x86_64 | x86_64-pc-windows-msvc | zip |

These alpha binaries are for evaluation and have signed build provenance, but no
platform code signatures or notarization. Read the [limitations](releases/0.1.0-alpha.2.md), particularly OS compatibility and credential storage.

The release is built from source commit `aad58397e139ed755fcf983da8e9f045e7e5a556` in
[attested run 34273757910](https://github.com/NIPE-Solutions/permesh/actions/runs/34273757910). It includes five archives,
five dependency inventories, ten checksum files and `attestation-bundle.json`.
The historical alpha.1 assets remain unchanged and are not retroactively attested.

Verify [alpha.2 provenance](#alpha2-release-provenance) before extracting or
running an executable, then check the adjacent checksum below.

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

Run `./permesh --version` (Windows: `.\permesh.exe --version`) from that directory. The version must be `0.1.0-alpha.2` for this release. You may copy the executable into a user-owned directory already on your PATH; no administrator privileges are needed. Keep license and dependency notices when redistributing. OS download protections may prompt or block unsigned programs; verify the source and follow your organization's policy rather than disabling protections globally.

Try the [offline demo](getting-started.md#evaluate-offline), then configure providers explicitly. Installation itself does not fetch provider data or store credentials.

## Alpha.2 release provenance

For each downloaded alpha.2 archive, inventory or checksum, verify the file with
a current GitHub CLI. These commands pin the exact released source and signer:

```sh
gh attestation verify DOWNLOADED_FILE \
  --repo NIPE-Solutions/permesh \
  --cert-identity https://github.com/NIPE-Solutions/permesh/.github/workflows/attested-candidates.yml@refs/heads/main \
  --source-ref refs/heads/main \
  --source-digest aad58397e139ed755fcf983da8e9f045e7e5a556 \
  --signer-digest aad58397e139ed755fcf983da8e9f045e7e5a556 \
  --predicate-type https://slsa.dev/provenance/v1 \
  --deny-self-hosted-runners
```

Do not substitute a newer source SHA to make a verification failure pass. Repeat
with `--bundle attestation-bundle.json` to verify the distributed bundle. The
published bundle is not itself a trust root;
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
  --source-sha aad58397e139ed755fcf983da8e9f045e7e5a556 \
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
