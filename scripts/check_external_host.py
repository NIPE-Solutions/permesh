# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exercise explicit local registration and discovery with the native test peer."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import time
import subprocess
import tempfile


def check():
    suffix = '.exe' if os.name == 'nt' else ''
    target = Path('target/debug').resolve()
    binary = target / ('permesh' + suffix)
    fixture = target / ('permesh-test-external-peer' + suffix)
    with tempfile.TemporaryDirectory(prefix='permesh-host-') as temporary:
        root = Path(temporary).resolve()
        source = root / ('reviewed-peer' + suffix)
        shutil.copyfile(fixture, source)
        env = dict(os.environ, PERMESH_DATA_DIR=str(root / 'state'),
                   TOKIO_WORKER_THREADS='2', NO_COLOR='1')
        (root / 'permesh.yaml').write_text('invalid: workspace must not load\n')

        def run(*args, code=0):
            result = subprocess.run([str(binary), 'provider', 'external', *args, '--json'],
                                    env=env, cwd=root, capture_output=True, timeout=80)
            assert result.returncode == code, (args, result.returncode, result.stdout, result.stderr)
            assert result.stderr == b'', result.stderr
            assert b'\x1b' not in result.stdout
            return json.loads(result.stdout)

        assert run('list')['result']['registrations'] == []
        assert not (root / 'state').exists()
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        assert run('inspect', str(source))['result']['inspection']['sha256'] == digest
        run('trust', str(source), '--id', 'fixture', '--sha256', digest, code=2)
        run('trust', str(source), '--id', 'fixture', '--sha256', '0' * 64,
            '--accept-risk', code=2)
        trusted = run('trust', str(source), '--id', 'fixture', '--sha256', digest,
                      '--accept-risk')['result']['registration']
        assert trusted == {'schema': 1, 'id': 'fixture', 'sha256': digest, 'capabilities': []}
        run('trust', str(source), '--id', 'fixture', '--sha256', digest,
            '--accept-risk', code=2)
        source.write_bytes(b'changed original must not affect managed copy')
        snapshot = run('discover', 'fixture', '--instance', 'normal')
        assert snapshot['complete'] is True
        assert snapshot['result']['snapshot']['complete'] is True
        partial = run('discover', 'fixture', '--instance', 'partial', code=4)
        assert partial['complete'] is False
        assert partial['result']['snapshot']['limitations']
        malformed = run('discover', 'fixture', '--instance', 'malformed', code=3)
        assert 'SENTINEL_SECRET' not in json.dumps(malformed)
        assert run('list')['result']['registrations'] == [trusted]
        if os.name != 'nt':
            child = subprocess.Popen([str(binary), 'provider', 'external', 'discover',
                                      'fixture', '--instance', 'cancel', '--json'],
                                     env=env, cwd=root, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE)
            try:
                ready = root / 'state/providers/fixture/ready'
                deadline = time.monotonic() + 10
                while not ready.exists() and child.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.02)
                assert ready.exists(), 'Native peer did not become ready'
                child.send_signal(signal.SIGINT)
                stdout, stderr = child.communicate(timeout=10)
                assert child.returncode == 130, (child.returncode, stdout, stderr)
                assert json.loads(stdout)['error'] and not stderr
                assert (ready.parent / 'cancelled').exists()
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate()
                for name in ('ready', 'cancelled'):
                    (root / 'state/providers/fixture' / name).unlink(missing_ok=True)
        managed = root / 'state/providers/fixture/provider.exe'
        managed.write_bytes(b'tampered managed executable')
        run('discover', 'fixture', '--instance', 'normal', code=2)
        run('remove', 'fixture')
        assert run('list')['result']['registrations'] == []
        assert not managed.exists()
    print('Native external CLI: inspection, explicit trust, discovery, tamper rejection and removal passed.')


if __name__ == '__main__':
    check()
