import hashlib
import json
from pathlib import Path
import tarfile
import tempfile
import subprocess
import unittest
from unittest.mock import patch
import zipfile

import package_candidate as package


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / 'project'
        self.root.mkdir()
        (self.root / 'src').mkdir()
        (self.root / 'src/main.rs').write_text('fn main() {}')
        (self.root / 'Cargo.toml').write_text("""[package]
name = "permesh-cli"
version = "0.1.0-alpha.1"
edition = "2021"
license = "MIT"
[dependencies]
runtime = { path = "../runtime" }
[build-dependencies]
builder = { path = "../builder" }
[dev-dependencies]
devonly = { path = "../devonly" }
[target.'cfg(windows)'.dependencies]
windowsonly = { path = "../windowsonly" }
""")
        for name in ('runtime', 'builder', 'devonly', 'windowsonly'):
            directory = self.root.parent / name
            directory.mkdir()
            (directory / 'src').mkdir()
            (directory / 'src/lib.rs').write_text('')
            (directory / 'Cargo.toml').write_text(
                f'[package]\nname = "{name}"\nversion = "1.0.0"\nlicense = "MIT"\n')
            (directory / 'LICENSE').write_text(f'Complete {name} license text.\n')
        subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=self.root,
                       check=True, capture_output=True)
        self.license = (Path(__file__).resolve().parents[1] / 'LICENSE').read_bytes()
        (self.root / 'LICENSE').write_bytes(self.license)
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
                one = package.package(self.root, target, '0.1.0-alpha.1', self.root / (target + '-one'))
                binary.touch()
                two = package.package(self.root, target, '0.1.0-alpha.1', self.root / (target + '-two'))
                self.assertEqual(one.read_bytes(), two.read_bytes())
                expected = {binary.name, 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'INSTALL.txt'}
                if one.suffix == '.zip':
                    with zipfile.ZipFile(one) as archive:
                        self.assertEqual(set(archive.namelist()), expected)
                        self.assertEqual(archive.read(binary.name), b'synthetic binary')
                        self.assertEqual(archive.read('LICENSE'), self.license)
                        notices = archive.read('THIRD-PARTY-NOTICES.txt')
                        self.assertEqual(archive.getinfo(binary.name).external_attr >> 16 & 0o777, 0o755)
                else:
                    with tarfile.open(one) as archive:
                        self.assertEqual(set(archive.getnames()), expected)
                        self.assertTrue(all(member.isfile() for member in archive.getmembers()))
                        self.assertEqual(archive.getmember(binary.name).mode, 0o755)
                        with archive.extractfile('LICENSE') as license_file:
                            self.assertEqual(license_file.read(), self.license)
                        with archive.extractfile('THIRD-PARTY-NOTICES.txt') as notice_file:
                            notices = notice_file.read()
                self.assertIn(b'Complete runtime license text.', notices)
                self.assertIn(b'Complete builder license text.', notices)
                self.assertNotIn(b'Complete devonly license text.', notices)
                self.assertEqual(b'Complete windowsonly license text.' in notices, 'windows' in target)
                checksum = one.with_name(one.name + '.sha256').read_text()
                self.assertEqual(checksum, hashlib.sha256(one.read_bytes()).hexdigest() + '  ' + one.name + '\n')
                inventory = one.parent / f'permesh-0.1.0-alpha.1-{target}.dependencies.json'
                document = json.loads(inventory.read_bytes())
                self.assertEqual(document['format'], 'permesh-dependency-inventory')
                self.assertEqual(document['candidate']['target'], target)
                self.assertEqual(document['candidate']['sha256'], hashlib.sha256(binary.read_bytes()).hexdigest())
                self.assertNotIn(b'SECRET', inventory.read_bytes())
                self.assertNotIn(str(self.root).encode(), inventory.read_bytes())
                self.assertEqual(inventory.with_suffix('.json.sha256').read_text(),
                                 hashlib.sha256(inventory.read_bytes()).hexdigest() + '  ' + inventory.name + '\n')
                self.assertEqual(len(list(one.parent.iterdir())), 4)

    def test_missing_dependency_notice_fails_before_creating_output(self):
        self.binary('aarch64-apple-darwin')
        (self.root.parent / 'runtime/LICENSE').unlink()
        with self.assertRaisesRegex(ValueError, 'missing source license/notice'):
            package.package(self.root, 'aarch64-apple-darwin', '0.1.0-alpha.1', self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_stale_lockfile_fails_before_creating_output(self):
        self.binary('aarch64-apple-darwin')
        (self.root / 'Cargo.lock').unlink()
        with self.assertRaises(FileNotFoundError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1.0-alpha.1', self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_changed_lockfile_fails_before_creating_output(self):
        self.binary('aarch64-apple-darwin')
        actual_run = subprocess.run

        def change_after_metadata(*args, **kwargs):
            result = actual_run(*args, **kwargs)
            with (self.root / 'Cargo.lock').open('a') as lock:
                lock.write('\n# concurrent edit\n')
            return result

        with patch.object(package.subprocess, 'run', side_effect=change_after_metadata):
            with self.assertRaisesRegex(ValueError, 'lockfile changed'):
                package.package(self.root, 'aarch64-apple-darwin', '0.1.0-alpha.1', self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_stale_dependency_and_mismatched_candidate_version_fail(self):
        self.binary('aarch64-apple-darwin')
        with self.assertRaisesRegex(ValueError, 'version differs'):
            package.package(self.root, 'aarch64-apple-darwin', '0.2.0', self.root / 'out')
        manifest = self.root.parent / 'runtime/Cargo.toml'
        manifest.write_text(manifest.read_text().replace('1.0.0', '1.1.0'))
        with self.assertRaises(subprocess.CalledProcessError):
            package.package(self.root, 'aarch64-apple-darwin', '0.1.0-alpha.1', self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_rejects_missing_binary_wrong_target_and_unsafe_version(self):
        for target, version in [('x86_64-unknown-linux-gnu', '0.1'), ('../../secret', '0.1'), ('aarch64-apple-darwin', '../secret'), ('aarch64-apple-darwin', 'x\ny')]:
            with self.subTest(target=target, version=version), self.assertRaises((ValueError, FileNotFoundError)):
                package.package(self.root, target, version, self.root / 'out')
        self.assertFalse((self.root / 'out').exists())

    def test_rejects_symlink_license_or_output(self):
        self.binary('aarch64-apple-darwin')
        license_file = self.root / 'LICENSE'
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
            package.package(self.root, 'aarch64-apple-darwin', '0.1.0-alpha.1', output)
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
