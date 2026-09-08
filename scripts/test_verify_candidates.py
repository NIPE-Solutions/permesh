import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import stat
import unittest
import zipfile

import verify_candidates as verify
from package_candidate import TARGETS

VERSION = '0.1.0-test'


def make_candidates(directory):
    for target in TARGETS:
        binary = 'permesh.exe' if 'windows' in target else 'permesh'
        base = f'permesh-{VERSION}-{target}'
        archive = directory / (base + ('.zip' if 'windows' in target else '.tar.gz'))
        entries = {binary: b'not executed', 'LICENSE': b'MIT',
                   'THIRD-PARTY-NOTICES.txt': b'Notices', 'INSTALL.txt': b'Install'}
        if archive.suffix == '.zip':
            with zipfile.ZipFile(archive, 'w') as output:
                for name, data in entries.items():
                    output.writestr(name, data)
        else:
            with tarfile.open(archive, 'w:gz') as output:
                for name, data in entries.items():
                    info = tarfile.TarInfo(name)
                    info.size = len(data)
                    output.addfile(info, io.BytesIO(data))
        inventory = directory / (base + '.dependencies.json')
        inventory.write_text(json.dumps({
            'format': 'permesh-dependency-inventory', 'format_version': 1,
            'candidate': {'name': 'permesh', 'version': VERSION, 'target': target,
                          'sha256': hashlib.sha256(b'not executed').hexdigest()},
            'cargo_lock_sha256': 'a' * 64,
        }))
        for artifact in (archive, inventory):
            checksum(artifact)


def checksum(path):
    path.with_name(path.name + '.sha256').write_text(
        hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.name + '\n')


class VerificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        make_candidates(self.directory)

    def test_validates_all_five_native_archives_without_extracting_or_executing(self):
        subjects = verify.inspect(self.directory, VERSION, 'a' * 64)
        self.assertEqual(len(subjects), 20)
        self.assertEqual(set(subjects), set(self.directory.iterdir()))
        self.assertEqual(len(list(self.directory.iterdir())), 20)

    def test_same_run_download_layout_rejects_extra_artifacts_before_merging(self):
        sha = 'b' * 40
        for target in TARGETS:
            folder = self.directory / f'permesh-candidate-{target}-{sha}'
            folder.mkdir()
            for path in list(self.directory.glob(f'permesh-{VERSION}-{target}.*')):
                path.rename(folder / path.name)
        subjects = verify.inspect(self.directory, VERSION, 'a' * 64, artifact_run_sha=sha)
        self.assertEqual(len(subjects), 20)
        (self.directory / 'permesh-candidate-extra').mkdir()
        with self.assertRaisesRegex(ValueError, 'artifact allowlist'):
            verify.inspect(self.directory, VERSION, 'a' * 64, artifact_run_sha=sha)

    def test_windows_checkout_may_use_crlf_but_unix_must_match_lf(self):
        windows = next(self.directory.glob('*windows*.dependencies.json'))
        report = json.loads(windows.read_bytes())
        report['cargo_lock_sha256'] = 'b' * 64
        windows.write_text(json.dumps(report))
        checksum(windows)
        self.assertEqual(len(verify.inspect(self.directory, VERSION, 'a' * 64,
                                            windows_lock_sha='b' * 64)), 20)
        unix = next(self.directory.glob('*apple*.dependencies.json'))
        report = json.loads(unix.read_bytes())
        report['cargo_lock_sha256'] = 'b' * 64
        unix.write_text(json.dumps(report))
        checksum(unix)
        with self.assertRaisesRegex(ValueError, 'lockfile'):
            verify.inspect(self.directory, VERSION, 'a' * 64, windows_lock_sha='b' * 64)

    def test_rejects_missing_extra_and_tampered_files(self):
        archive = next(self.directory.glob('*.tar.gz'))
        original = archive.read_bytes()
        archive.write_bytes(original + b'tampered')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            verify.inspect(self.directory, VERSION, 'a' * 64)
        archive.write_bytes(original)
        extra = self.directory / 'unexpected'
        extra.write_bytes(b'extra')
        with self.assertRaisesRegex(ValueError, 'allowlist'):
            verify.inspect(self.directory, VERSION, 'a' * 64)
        extra.unlink()
        archive.unlink()
        with self.assertRaisesRegex(ValueError, 'allowlist'):
            verify.inspect(self.directory, VERSION, 'a' * 64)

    def test_rejects_mismatched_inventory_even_with_recomputed_checksum(self):
        path = next(self.directory.glob('*.dependencies.json'))
        original = json.loads(path.read_bytes())
        for field in ('version', 'target', 'sha256'):
            data = json.loads(json.dumps(original))
            data['candidate'][field] = 'wrong'
            path.write_text(json.dumps(data))
            checksum(path)
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify.inspect(self.directory, VERSION, 'a' * 64)
        path.write_text(json.dumps(original))
        checksum(path)
        with self.assertRaisesRegex(ValueError, 'lockfile'):
            verify.inspect(self.directory, VERSION, 'b' * 64)

    def test_rejects_archive_links_and_extra_or_duplicate_members(self):
        path = next(self.directory.glob('*.tar.gz'))
        for mutation in ('link', 'extra', 'duplicate'):
            make_candidates(self.directory)
            with tarfile.open(path) as source:
                members = [(item.name, source.extractfile(item).read()) for item in source]
            with tarfile.open(path, 'w:gz') as output:
                for name, data in members:
                    info = tarfile.TarInfo(name)
                    if mutation == 'link' and name == 'permesh':
                        info.type, info.linkname = tarfile.SYMTYPE, '/private/secret'
                        output.addfile(info)
                    else:
                        info.size = len(data)
                        output.addfile(info, io.BytesIO(data))
                if mutation != 'link':
                    info = tarfile.TarInfo('permesh' if mutation == 'duplicate' else '../escape')
                    output.addfile(info, io.BytesIO())
            checksum(path)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                verify.inspect(self.directory, VERSION, 'a' * 64)
        self.assertFalse((self.directory.parent / 'escape').exists())

    def test_rejects_zip_links_and_unexpected_members(self):
        path = next(self.directory.glob('*.zip'))
        for mutation in ('link', 'extra'):
            make_candidates(self.directory)
            with zipfile.ZipFile(path) as source:
                members = [(item.filename, source.read(item)) for item in source.infolist()]
            with zipfile.ZipFile(path, 'w') as output:
                for name, data in members:
                    info = zipfile.ZipInfo(name)
                    if mutation == 'link' and name == 'permesh.exe':
                        info.create_system = 3
                        info.external_attr = (stat.S_IFLNK | 0o777) << 16
                    output.writestr(info, data)
                if mutation == 'extra':
                    output.writestr('../escape', b'bad')
            checksum(path)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                verify.inspect(self.directory, VERSION, 'a' * 64)

    def test_rejects_linked_downloads(self):
        path = next(self.directory.glob('*.dependencies.json'))
        data = path.read_bytes()
        path.unlink()
        other = self.directory.parent / (self.directory.name + '-linked')
        other.write_bytes(data)
        self.addCleanup(other.unlink)
        try:
            path.symlink_to(other)
        except OSError:
            self.skipTest('symlink creation unavailable')
        with self.assertRaisesRegex(ValueError, 'linked'):
            verify.inspect(self.directory, VERSION, 'a' * 64)


if __name__ == '__main__':
    unittest.main()
