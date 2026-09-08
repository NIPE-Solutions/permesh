"""Run the explicitly selected synthetic Python peer through the offline Rust validator."""
import json
from pathlib import Path
import subprocess
import sys


def check():
    root = Path(__file__).resolve().parents[1]
    requests = (b'{"protocol":1,"id":"handshake","method":"handshake","instance":"example-main"}\n'
                b'{"protocol":1,"id":"discover","method":"discover"}\n')
    peer = subprocess.run([sys.executable, str(root / 'examples/external-provider/provider.py')],
                          input=requests, capture_output=True, timeout=10, check=True)
    if peer.stderr or peer.stdout != (root / 'examples/external-provider/discovery.ndjson').read_bytes():
        raise RuntimeError('Python peer differs from the synthetic fixture')
    command = ['cargo', 'run', '--locked', '--quiet', '-p', 'permesh-provider-protocol',
               '--example', 'validate', '--', 'synthetic-example', 'example-main']
    result = subprocess.run(command, cwd=root, input=peer.stdout, capture_output=True, timeout=120)
    if result.returncode != 0 or result.stderr:
        raise RuntimeError('Rust validator rejected the Python discovery exchange')
    expected = {'protocol_version': 1, 'valid': True, 'complete': True,
                'counts': dict.fromkeys(('identities', 'accounts', 'resources', 'groups', 'memberships', 'grants'), 1)}
    if json.loads(result.stdout) != expected:
        raise RuntimeError('Unexpected validation summary')
    hostile = b'{"protocol":1,"protocol":1,"secret":"SENTINEL_PROTOCOL_SECRET"}\n'
    rejected = subprocess.run(command, cwd=root, input=hostile, capture_output=True, timeout=120)
    if rejected.returncode != 2 or rejected.stdout or not rejected.stderr:
        raise RuntimeError('Malformed exchange did not fail closed')
    if b'SENTINEL_PROTOCOL_SECRET' in rejected.stderr:
        raise RuntimeError('Validator reflected malformed input')
    print('Synthetic Python-to-Rust protocol validation passed.')


if __name__ == '__main__':
    check()
