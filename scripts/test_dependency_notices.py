"""Exercise source notice collection without a registry or network dependency."""
import copy
from pathlib import Path
import tempfile
import unittest

import dependency_notices as notices


class NoticeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / 'workspace'
        self.root.mkdir()
        (self.root / 'LICENSE').write_text('Project MIT terms\n')
        self.cli = self.make_package('permesh-cli', local=True)
        self.runtime = self.make_package('runtime')
        self.builder = self.make_package('builder')
        self.dev = self.make_package('devonly')
        self.metadata = {
            'packages': [self.cli, self.runtime, self.builder, self.dev],
            'resolve': {'nodes': [
                {'id': self.cli['id'], 'deps': [
                    {'pkg': self.runtime['id'], 'dep_kinds': [{'kind': None}]},
                    {'pkg': self.builder['id'], 'dep_kinds': [{'kind': 'build'}]},
                    {'pkg': self.dev['id'], 'dep_kinds': [{'kind': 'dev'}]},
                ]},
                *[{'id': p['id'], 'deps': []} for p in (self.runtime, self.builder, self.dev)],
            ]},
        }

    def make_package(self, name, local=False):
        directory = (self.root if local else self.root.parent) / name
        directory.mkdir()
        (directory / 'Cargo.toml').write_text(f'[package]\nname = "{name}"\n')
        if not local:
            (directory / 'LICENSE').write_text('Shared complete MIT terms\n')
        return {'id': name, 'name': name, 'version': '1.0.0',
                'source': None if local else 'registry+https://example.test/index',
                'manifest_path': str(directory / 'Cargo.toml'), 'license': 'MIT',
                'license_file': None}

    def runtime_directory(self):
        return Path(self.runtime['manifest_path']).parent

    def test_runtime_and_build_not_dev_include_nested_notices_and_deduplicate(self):
        nested = self.runtime_directory() / 'vendor' / 'legal'
        nested.mkdir(parents=True)
        (nested / 'attribution.txt').write_text('Full vendor attribution\n')
        result = notices.bundle(self.root, self.metadata).decode()
        self.assertIn('runtime 1.0.0', result)
        self.assertIn('builder 1.0.0', result)
        self.assertNotIn('devonly 1.0.0', result)
        self.assertIn('Full vendor attribution', result)
        self.assertIn('vendor/legal/attribution.txt -> SHA-256', result)
        self.assertEqual(result.count('Shared complete MIT terms'), 1)
        self.assertIn('Project MIT terms', result)
        self.assertNotIn(str(self.root.parent), result)
        reordered = copy.deepcopy(self.metadata)
        reordered['packages'].reverse()
        reordered['resolve']['nodes'].reverse()
        self.assertEqual(notices.bundle(self.root, reordered).decode(), result)

    def test_explicit_nonstandard_license_filename_is_included(self):
        directory = self.runtime_directory()
        (directory / 'LICENSE').unlink()
        (directory / 'terms.txt').write_text('All explicit terms\n')
        self.runtime['license_file'] = 'terms.txt'
        self.runtime['license'] = None
        self.assertIn(b'All explicit terms', notices.bundle(self.root, self.metadata))

    def test_missing_empty_oversized_or_non_utf8_notices_fail_closed(self):
        license_path = self.runtime_directory() / 'LICENSE'
        for content in (None, b'\n ', b'x' * (notices.MAX_NOTICE_BYTES + 1), b'\xff'):
            with self.subTest(content=None if content is None else len(content)):
                if content is None:
                    license_path.unlink()
                else:
                    license_path.write_bytes(content)
                with self.assertRaises((ValueError, UnicodeError)):
                    notices.bundle(self.root, self.metadata)

    def test_explicit_license_must_stay_within_dependency(self):
        for relative in ('../workspace/LICENSE', str(self.root / 'LICENSE')):
            with self.subTest(relative=relative):
                self.runtime['license_file'] = relative
                with self.assertRaisesRegex(ValueError, 'leaves its package'):
                    notices.bundle(self.root, self.metadata)

    def test_linked_license_is_not_followed(self):
        license_path = self.runtime_directory() / 'LICENSE'
        license_path.unlink()
        try:
            license_path.symlink_to(self.root / 'LICENSE')
        except OSError:
            self.skipTest('symlink creation unavailable')
        with self.assertRaisesRegex(ValueError, 'links or junctions'):
            notices.bundle(self.root, self.metadata)

    def test_missing_license_metadata_fails_closed(self):
        self.runtime['license'] = None
        with self.assertRaisesRegex(ValueError, 'license metadata is missing'):
            notices.bundle(self.root, self.metadata)

    def test_missing_or_duplicate_cli_root_and_broken_edges_fail_closed(self):
        for mutation in ('missing', 'duplicate', 'edge', 'resolve'):
            with self.subTest(mutation=mutation):
                metadata = copy.deepcopy(self.metadata)
                if mutation == 'missing':
                    metadata['packages'].remove(metadata['packages'][0])
                elif mutation == 'duplicate':
                    metadata['packages'].append(dict(self.cli, id='another-cli'))
                elif mutation == 'edge':
                    metadata['resolve']['nodes'][0]['deps'][0]['pkg'] = 'missing'
                else:
                    metadata['resolve'] = None
                with self.assertRaises(ValueError):
                    notices.bundle(self.root, metadata)


if __name__ == '__main__':
    unittest.main()
