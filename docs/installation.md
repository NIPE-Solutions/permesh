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

## Upgrade or uninstall

To upgrade, download and verify the desired release and replace the executable while it is not running. There is no CLI self-update or background update check. `provider update` manages separate provider packages and does not update the CLI.

To uninstall, remove the executable. This preserves workspace files, exported reports, installed provider packages, local approvals and keychain credentials. Use `auth logout` for credentials you want to remove before uninstalling; see [secret storage](secrets.md) and [provider storage](provider-packages.md) for retained local data. Never delete shared credentials indiscriminately.

## Build from source

A checkout needs Rust 1.91 or newer:

```sh
cargo install --path crates/permesh-cli --locked
```

The project is not published to crates.io; `cargo install permesh` is not currently supported.
