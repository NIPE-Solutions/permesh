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


def check_negotiated():
    root = Path(__file__).resolve().parents[1]
    subprocess.run([sys.executable, '-m', 'unittest', 'discover',
                    '-s', 'examples/negotiated-provider', '-p', 'test_*.py'],
                   cwd=root, check=True, timeout=30)
    for operation in ('discover', 'check'):
        requests = b''.join(json.dumps(value).encode() + b'\n' for value in (
            {'protocol': 5, 'id': 'handshake', 'method': 'handshake',
             'instance': 'example-main', 'operation': operation},
            {'protocol': 5, 'id': operation, 'method': operation,
             'configuration': {}, 'credentials': {'token': 'SYNTHETIC_PRIVATE_INPUT'}},
        ))
        peer = subprocess.run([sys.executable, str(root / 'examples/negotiated-provider/provider.py')],
                              input=requests, capture_output=True, timeout=10, check=True)
        if peer.stderr or b'SYNTHETIC_PRIVATE_INPUT' in peer.stdout:
            raise RuntimeError('Negotiated Python peer leaked diagnostics or credentials')
        command = ['cargo', 'run', '--locked', '--quiet', '-p', 'permesh-provider-protocol',
                   '--example', 'validate_negotiated', '--', 'synthetic-example', 'example-main', operation]
        result = subprocess.run(command, cwd=root, input=peer.stdout, capture_output=True, timeout=120)
        if result.returncode != 0 or result.stderr:
            raise RuntimeError('Rust validator rejected negotiated Python exchange')
        summary = json.loads(result.stdout)
        if summary['protocol_version'] != 5 or not summary['valid'] or summary['operation'] != operation:
            raise RuntimeError('Unexpected negotiated summary')
        if operation == 'discover' and summary['counts'] != {
                'identities': 1, 'accounts': 1, 'resources': 2, 'groups': 1, 'memberships': 1, 'grants': 1}:
            raise RuntimeError('Unexpected negotiated records')
        if operation == 'discover' and summary.get('complete') is not True:
            raise RuntimeError('Synthetic discovery unexpectedly incomplete')
        if operation == 'check' and summary.get('limitations') != 0:
            raise RuntimeError('Synthetic health unexpectedly limited')
        # Decoder errors must never print input; valid peer output does not excuse trailing bytes.
        hostile = peer.stdout + b'{"private":"SYNTHETIC_PRIVATE_INPUT"}\n'
        rejected = subprocess.run(command, cwd=root, input=hostile, capture_output=True, timeout=120)
        if rejected.returncode != 2 or rejected.stdout or not rejected.stderr:
            raise RuntimeError('Malformed negotiated exchange did not fail closed')
        if b'SYNTHETIC_PRIVATE_INPUT' in rejected.stderr:
            raise RuntimeError('Negotiated validator reflected malformed input')
    print('Negotiated Python-to-Rust discovery and health validation passed.')


if __name__ == '__main__':
    check()
    check_negotiated()
