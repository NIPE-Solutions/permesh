"""Validate the complete native candidate set, optionally verifying signed provenance."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess
import tarfile
import zipfile

from package_candidate import TARGETS

REPOSITORY = 'NIPE-Solutions/permesh'
WORKFLOW = REPOSITORY + '/.github/workflows/attested-candidates.yml'
MAX_ARTIFACT = 256 * 1024 * 1024
MAX_MEMBER = 64 * 1024 * 1024


def safe_path(path):
    path = Path(path).absolute()
    if any(p.is_symlink() or p.is_junction() for p in (path, *path.parents)):
        raise ValueError('candidate paths must not be linked')
    return path


def regular(path, limit=MAX_ARTIFACT):
    path = safe_path(path)
    if not path.is_file() or path.stat().st_size > limit:
        raise ValueError('candidate must be a bounded regular file')
    return path


def digest(path):
    with regular(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def archive_digest(path, binary):
    expected = {binary, 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'INSTALL.txt'}
    seen, result = set(), None

    def consume(name, size, stream):
        nonlocal result
        if name not in expected or name in seen or not 0 < size <= MAX_MEMBER:
            raise ValueError('archive member allowlist or size mismatch')
        seen.add(name)
        if name == binary:
            result = hashlib.file_digest(stream, 'sha256').hexdigest()

    if path.suffix == '.zip':
        with zipfile.ZipFile(path) as archive:
            members = archive.infolist()
            if len(members) != 4:
                raise ValueError('archive member count mismatch')
            for member in members:
                mode = member.external_attr >> 16
                if member.is_dir() or stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                    raise ValueError('archive contains a nonregular member')
                with archive.open(member) as stream:
                    consume(member.filename, member.file_size, stream)
    else:
        with tarfile.open(path, 'r:gz') as archive:
            for member in archive:
                if not member.isfile():
                    raise ValueError('archive contains a nonregular member')
                with archive.extractfile(member) as stream:
                    consume(member.name, member.size, stream)
    if seen != expected:
        raise ValueError('archive member allowlist mismatch')
    return result


def inspect(directory, version, lock_sha, artifact_run_sha=None, windows_lock_sha=None):
    directory = safe_path(directory)
    if (re.fullmatch(r'[0-9][A-Za-z0-9.+-]{0,79}', version) is None
            or re.fullmatch(r'[a-f0-9]{64}', lock_sha) is None):
        raise ValueError('invalid expected candidate version or lockfile digest')
    if windows_lock_sha is not None and re.fullmatch(r'[a-f0-9]{64}', windows_lock_sha) is None:
        raise ValueError('invalid Windows lockfile digest')
    if artifact_run_sha is not None:
        if re.fullmatch(r'[a-f0-9]{40}', artifact_run_sha) is None:
            raise ValueError('invalid candidate run SHA')
        folders = {f'permesh-candidate-{target}-{artifact_run_sha}' for target in TARGETS}
        if {p.name for p in directory.iterdir()} != folders:
            raise ValueError('candidate artifact allowlist mismatch')
    expected, pairs = set(), []
    for target in TARGETS:
        base = f'permesh-{version}-{target}'
        parent = directory if artifact_run_sha is None else safe_path(
            directory / f'permesh-candidate-{target}-{artifact_run_sha}')
        archive = parent / (base + ('.zip' if 'windows' in target else '.tar.gz'))
        inventory = parent / (base + '.dependencies.json')
        pairs.append((target, archive, inventory))
        for path in (archive, inventory):
            expected.update((path, path.with_name(path.name + '.sha256')))
    actual = set(directory.iterdir()) if artifact_run_sha is None else {
        p for folder in directory.iterdir() for p in safe_path(folder).iterdir()}
    if actual != expected:
        raise ValueError('candidate file allowlist mismatch; all five targets are required')
    subjects = sorted(regular(path) for path in expected)
    for target, archive, inventory in pairs:
        for path in (archive, inventory):
            checksum = regular(path.with_name(path.name + '.sha256'), 512).read_text(encoding='ascii')
            if checksum != digest(path) + '  ' + path.name + '\n':
                raise ValueError('candidate checksum mismatch')
        report = json.loads(regular(inventory, 8 * 1024 * 1024).read_bytes())
        if report.get('format') != 'permesh-dependency-inventory' or report.get('format_version') != 1:
            raise ValueError('unexpected dependency inventory format')
        binary = 'permesh.exe' if 'windows' in target else 'permesh'
        if report.get('candidate') != {'name': 'permesh', 'version': version, 'target': target,
                                       'sha256': archive_digest(archive, binary)}:
            raise ValueError('candidate identity or binary digest mismatch')
        allowed_lock_hashes = {lock_sha}
        if 'windows' in target and windows_lock_sha is not None:
            allowed_lock_hashes.add(windows_lock_sha)
        if report.get('cargo_lock_sha256') not in allowed_lock_hashes:
            raise ValueError('candidate lockfile digest mismatch')
    return subjects


def verify_attestations(subjects, source_sha, bundle=None):
    if re.fullmatch(r'[a-f0-9]{40}', source_sha) is None:
        raise ValueError('verification requires an exact reviewed source commit SHA')
    if bundle is not None:
        bundle = regular(bundle, 16 * 1024 * 1024)
    for subject in subjects:
        command = ['gh', 'attestation', 'verify', str(regular(subject)),
                   '--repo', REPOSITORY,
                   '--cert-identity', 'https://github.com/' + WORKFLOW + '@refs/heads/main',
                   '--signer-digest', source_sha, '--source-digest', source_sha,
                   '--source-ref', 'refs/heads/main', '--deny-self-hosted-runners',
                   '--predicate-type', 'https://slsa.dev/provenance/v1']
        if bundle is not None:
            command += ['--bundle', str(bundle)]
        subprocess.run(command, check=True, timeout=120)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--version', required=True)
    parser.add_argument('--lockfile', type=Path, required=True)
    parser.add_argument('--artifact-run-sha', help='Require the exact five same-run download-artifact folders')
    parser.add_argument('--source-sha', help='Verify signed provenance for every file using this reviewed commit')
    parser.add_argument('--bundle', type=Path, help='Use a downloaded Sigstore bundle instead of the API')
    args = parser.parse_args()
    if args.bundle and not args.source_sha:
        parser.error('--bundle requires --source-sha')
    lock_lf = regular(args.lockfile, 4 * 1024 * 1024).read_bytes().replace(b'\r\n', b'\n')
    subjects = inspect(args.directory, args.version, hashlib.sha256(lock_lf).hexdigest(),
                       args.artifact_run_sha, hashlib.sha256(lock_lf.replace(b'\n', b'\r\n')).hexdigest())
    if args.source_sha:
        verify_attestations(subjects, args.source_sha, args.bundle)
        print(f'Validated and verified signed provenance for all {len(subjects)} candidate files.')
    else:
        print(f'Validated all {len(subjects)} candidate files; signatures were not checked.')
