import json
import unittest

import dependency_inventory as inventory

LOCAL = 'path+file:///private/build/permesh-cli#0.1.0'
RUNTIME = 'registry+https://github.com/rust-lang/crates.io-index#runtime@1.2.3'
BUILDER = 'registry+https://github.com/rust-lang/crates.io-index#builder@2.0.0'
DEV = 'registry+https://github.com/rust-lang/crates.io-index#devonly@3.0.0'


def fixture():
    metadata = {'packages': [], 'resolve': {'nodes': []}}
    for ref, name, version in [(LOCAL, 'permesh-cli', '0.1.0'), (RUNTIME, 'runtime', '1.2.3'),
                               (BUILDER, 'builder', '2.0.0'), (DEV, 'devonly', '3.0.0')]:
        metadata['packages'].append({'id': ref, 'name': name, 'version': version,
            'source': None if ref == LOCAL else inventory.CRATES_IO, 'license': 'MIT',
            'manifest_path': '/private/build/SECRET/Cargo.toml', 'description': 'SECRET',
            'authors': ['SECRET'], 'metadata': {'secret': 'SECRET'}})
        metadata['resolve']['nodes'].append({'id': ref, 'deps': []})
    metadata['resolve']['nodes'][0]['deps'] = [
        {'pkg': RUNTIME, 'dep_kinds': [{'kind': None}]},
        {'pkg': BUILDER, 'dep_kinds': [{'kind': 'build'}]},
        {'pkg': DEV, 'dep_kinds': [{'kind': 'dev'}]},
    ]
    lock = ('version = 4\n' + ''.join(
        f'[[package]]\nname = "{name}"\nversion = "{version}"\nsource = "{inventory.CRATES_IO}"\nchecksum = "' + 'a' * 64 + '"\n'
        for name, version in [('runtime', '1.2.3'), ('builder', '2.0.0'), ('devonly', '3.0.0')])).encode()
    return metadata, lock


class InventoryTests(unittest.TestCase):
    def test_allowlisted_locked_graph_excludes_dev_and_private_metadata(self):
        metadata, lock = fixture()
        data = inventory.bundle(metadata, lock, 'aarch64-apple-darwin', '0.1.0', b'binary')
        result = json.loads(data)
        self.assertEqual(result['format'], 'permesh-dependency-inventory')
        self.assertEqual(result['candidate']['version'], '0.1.0')
        self.assertEqual(result['candidate']['target'], 'aarch64-apple-darwin')
        self.assertEqual(result['candidate']['sha256'], '9a3a45d01531a20e89ac6ae10b0b0beb0492acd7216a368aa062d1a5fecaf9cd')
        self.assertEqual([p['name'] for p in result['packages']], ['builder', 'permesh-cli', 'runtime'])
        cli = result['packages'][1]
        self.assertEqual(cli['dependencies'], [
            {'package': 'builder@2.0.0', 'kinds': ['build']},
            {'package': 'runtime@1.2.3', 'kinds': ['normal']},
        ])
        self.assertEqual(result['packages'][2]['registry_checksum_sha256'], 'a' * 64)
        for forbidden in (b'SECRET', b'/private', b'file:', b'devonly', b'CycloneDX', b'SPDXID'):
            self.assertNotIn(forbidden, data)
        self.assertEqual(data, inventory.bundle(metadata, lock, 'aarch64-apple-darwin', '0.1.0', b'binary'))

    def test_rejects_stale_version_missing_checksum_and_private_registry(self):
        for mutation in ('version', 'checksum', 'source'):
            metadata, lock = fixture()
            version = '0.1.0'
            if mutation == 'version':
                version = '0.2.0'
            elif mutation == 'checksum':
                lock = b'version = 4\n'
            else:
                metadata['packages'][1]['source'] = 'registry+https://secret@private.example/index'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                inventory.bundle(metadata, lock, 'aarch64-apple-darwin', version, b'binary')


if __name__ == '__main__':
    unittest.main()
