import hashlib
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

import package_candidate as package


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        for name in ('LICENSE-MIT', 'LICENSE-APACHE'):
            (self.root / name).write_text(name)
        (self.root / 'permesh.local.yaml').write_text('SECRET')

    def binary(self, target):
        path = self.root / 'target' / target / 'release' / ('permesh.exe' if 'windows' in target else 'permesh')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'synthetic binary')
        return path

    def test_archives_deterministic_allowlisted_and_checksummed(self):
        for target in ('aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            with self.subTest(target=target):
                binary = self.binary(target)
                one = package.package(self.root, target, '0.1.0-dev', self.root / (target + '-one'))
                binary.touch()
                two = package.package(self.root, target, '0.1.0-dev', self.root / (target + '-two'))
                self.assertEqual(one.read_bytes(), two.read_bytes())
                expected = {binary.name, 'LICENSE-MIT', 'LICENSE-APACHE', 'INSTALL.txt'}
                if one.suffix == '.zip':
                    with zipfile.ZipFile(one) as archive:
                        self.assertEqual(set(archive.namelist()), expected)
                        self.assertEqual(archive.read(binary.name), b'synthetic binary')
                        self.assertEqual(archive.getinfo(binary.name).external_attr >> 16 & 0o777, 0o755)
                else:
                    with tarfile.open(one) as archive:
                        self.assertEqual(set(archive.getnames()), expected)
                        self.assertTrue(all(member.isfile() for member in archive.getmembers()))
                        self.assertEqual(archive.getmember(binary.name).mode, 0o755)
                checksum = one.with_name(one.name + '.sha256').read_text()
                self.assertEqual(checksum, hashlib.sha256(one.read_bytes()).hexdigest() + '  ' + one.name + '\n')
                self.assertEqual(len(list(one.parent.iterdir())), 2)

    def test_rejects_missing_binary_wrong_target_and_unsafe_version(self):
        for target, version in [('x86_64-unknown-linux-gnu', '0.1'), ('../../secret', '0.1'), ('aarch64-apple-darwin', '../secret'), ('aarch64-apple-darwin', 'x\ny')]:
            with self.subTest(target=target, version=version), self.assertRaises((ValueError, FileNotFoundError)):
                package.package(self.root, target, version, self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_rejects_symlink_license_or_output(self):
        self.binary('aarch64-apple-darwin')
        license_file = self.root / 'LICENSE-MIT'
        license_file.unlink()
        try:
            license_file.symlink_to(self.root / 'permesh.local.yaml')
        except OSError:
            self.skipTest('symlink creation unavailable')
        with self.assertRaises(ValueError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1', self.root / 'out')
        license_file.unlink()
        license_file.write_text('MIT')
        destination = self.root / 'private'
        destination.mkdir()
        (self.root / 'out').symlink_to(destination, target_is_directory=True)
        with self.assertRaises(ValueError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1', self.root / 'out')
        self.assertEqual(list(destination.iterdir()), [])

    def test_rejects_existing_output_without_changing_it(self):
        self.binary('aarch64-apple-darwin')
        output = self.root / 'out'
        output.mkdir()
        sentinel = output / 'keep'
        sentinel.write_text('keep')
        with self.assertRaises(FileExistsError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1', output)
        self.assertEqual(sentinel.read_text(), 'keep')

    def test_rejects_symlink_input_and_parent(self):
        binary = self.binary('aarch64-apple-darwin')
        binary.unlink()
        try:
            binary.symlink_to(self.root / 'permesh.local.yaml')
        except OSError:
            self.skipTest('symlink creation unavailable')
        with self.assertRaises(ValueError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1', self.root / 'out')
        binary.unlink()
        binary.parent.rmdir()
        binary.parent.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1', self.root / 'out')


if __name__ == '__main__':
    unittest.main()
