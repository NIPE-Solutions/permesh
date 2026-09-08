"""Create a deliberately small unsigned native candidate; Python standard library only."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import re
import stat
import tarfile
import zipfile

TARGETS = (
    'aarch64-apple-darwin', 'x86_64-apple-darwin',
    'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu',
    'x86_64-pc-windows-msvc',
)
INSTALL = b'''Permesh unsigned development candidate

This is not a qualified public release. Verify the adjacent SHA-256 checksum
before extracting. A checksum detects corruption, not publisher authenticity.
The archive contains one executable and the project's two license texts.
Dependency notice review remains a public-release gate.

Run ./permesh --help (Windows: .\\permesh.exe --help) from this directory.
For an offline trial, create an empty directory, change into it and run the
executable by its absolute path with: init --demo, doctor, admins --json.
No administrator privileges or system installation are required. Optionally
copy the executable into a user-owned directory already on your PATH.
Remove that executable to uninstall. Workspaces and native stored credentials
are separate; removing the executable does not delete them.

These candidates are unsigned, unnotarized, and not compatibility guarantees
for older operating systems. Native credential stores and live providers
require separate qualification. Do not redistribute as a public release.
'''


def regular_bytes(root, relative):
    path = root
    for part in relative.parts:
        path = path / part
        if path.is_symlink():
            raise ValueError('candidate input must not contain symlinks')
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError('candidate input must be a regular file')
    return path.read_bytes()


def package(root, target, version, output):
    if target not in TARGETS or re.fullmatch(r'[0-9][A-Za-z0-9.+-]{0,79}', version) is None:
        raise ValueError('unsupported target or unsafe version')
    root = Path(root).resolve(strict=True)
    executable = 'permesh.exe' if 'windows' in target else 'permesh'
    entries = [(executable, regular_bytes(root, Path('target') / target / 'release' / executable), 0o755)]
    entries += [(name, regular_bytes(root, Path(name)), 0o644) for name in ('LICENSE',)]
    entries.append(('INSTALL.txt', INSTALL, 0o644))
    output = Path(output).absolute()
    if any(parent.is_symlink() for parent in (output, *output.parents)):
        raise ValueError('candidate output must not contain symlinks')
    # Require a new directory: never merge with a workspace or previous output.
    output.mkdir()
    suffix = '.zip' if 'windows' in target else '.tar.gz'
    archive = output / f'permesh-{version}-{target}{suffix}'
    with archive.open('xb') as raw:
        if suffix == '.zip':
            with zipfile.ZipFile(raw, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as stream:
                for name, data, mode in entries:
                    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                    info.create_system = 3
                    info.external_attr = (stat.S_IFREG | mode) << 16
                    stream.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
        else:
            with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=9) as compressed:
                with tarfile.open(fileobj=compressed, mode='w', format=tarfile.USTAR_FORMAT) as stream:
                    for name, data, mode in entries:
                        info = tarfile.TarInfo(name)
                        info.size, info.mode, info.mtime = len(data), mode, 0
                        stream.addfile(info, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    with archive.with_name(archive.name + '.sha256').open('x', encoding='ascii', newline='\n') as checksum:
        checksum.write(f'{digest}  {archive.name}\n')
    return archive


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(package(args.root, args.target, args.version, args.output))
