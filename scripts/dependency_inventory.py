"""Bounded release inventory; deliberately not a CycloneDX or SPDX SBOM."""
import hashlib
import json
import re
import tomllib

from dependency_notices import reachable

CRATES_IO = 'registry+https://github.com/rust-lang/crates.io-index'


def bundle(metadata, lock_bytes, target, version, binary):
    packages = reachable(metadata)
    cli = next(p for p in packages if p['name'] == 'permesh-cli')
    if cli['version'] != version:
        raise ValueError('candidate version differs from locked package metadata')
    locked = {(p['name'], p['version'], p.get('source')): p
              for p in tomllib.loads(lock_bytes.decode('utf-8')).get('package', [])}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    identifiers = {}
    for package in packages:
        if (not re.fullmatch(r'[A-Za-z0-9_-]+', package['name'])
                or not re.fullmatch(r'[0-9][A-Za-z0-9.+-]*', package['version'])):
            raise ValueError('unsafe inventory package identity')
        identifiers[package['id']] = f"{package['name']}@{package['version']}"
    if len(set(identifiers.values())) != len(identifiers):
        raise ValueError('ambiguous inventory package identity')
    result = []
    for package in packages:
        source = package.get('source')
        if source not in (None, CRATES_IO):
            raise ValueError('inventory supports only local or public crates.io packages')
        item = {'id': identifiers[package['id']], 'name': package['name'],
                'version': package['version'], 'source': 'local' if source is None else 'crates.io'}
        if source == CRATES_IO:
            entry = locked.get((package['name'], package['version'], source), {})
            checksum = entry.get('checksum', '')
            if not re.fullmatch(r'[a-f0-9]{64}', checksum):
                raise ValueError('inventory registry package lacks a locked SHA-256 checksum')
            item['registry_checksum_sha256'] = checksum
        dependencies = []
        for dependency in nodes[package['id']]['deps']:
            kinds = sorted({kind['kind'] or 'normal' for kind in dependency['dep_kinds']
                            if kind['kind'] in (None, 'build')})
            if kinds:
                dependencies.append({'package': identifiers[dependency['pkg']], 'kinds': kinds})
        item['dependencies'] = sorted(dependencies, key=lambda value: value['package'])
        result.append(item)
    document = {
        'format': 'permesh-dependency-inventory', 'format_version': 1,
        'candidate': {'name': 'permesh', 'version': version, 'target': target,
                      'sha256': hashlib.sha256(binary).hexdigest()},
        'cargo_lock_sha256': hashlib.sha256(lock_bytes).hexdigest(),
        'scope': {'resolution': 'cargo-metadata-filter-platform-default-features',
                  'includes': ['normal', 'build'], 'excludes': ['dev-only'],
                  'linked_binary_inventory': False, 'system_libraries_included': False},
        'packages': result,
    }
    return (json.dumps(document, indent=2, sort_keys=True) + '\n').encode('utf-8')
