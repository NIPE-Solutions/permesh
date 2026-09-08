# SPDX-License-Identifier: MIT
"""Offline end-to-end provider forms, local answers and reviewed workspace setup."""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time

CAPS = ('accounts', 'identities', 'resources', 'groups', 'memberships', 'grants')


def check():
    suffix = '.exe' if os.name == 'nt' else ''
    target = Path('target/debug').resolve()
    binary = target / ('permesh' + suffix)
    fixture = target / ('permesh-test-external-peer' + suffix)
    with tempfile.TemporaryDirectory(prefix='permesh-setup-') as temporary:
        root = Path(temporary).resolve()
        env = dict(os.environ, PERMESH_DATA_DIR=str(root / 'state'),
                   TOKIO_WORKER_THREADS='2', NO_COLOR='1', TERM='dumb',
                   PERMESH_SETUP_TOKEN='synthetic-fixture-token')
        workspace = root / 'workspace'
        workspace.mkdir()
        answers = root / 'answers.yaml'
        config = workspace / 'permesh.yaml'

        def run(*args, code=0, cwd=workspace):
            result = subprocess.run([str(binary), *args, '--json'], cwd=cwd, env=env,
                                    capture_output=True, timeout=80)
            assert result.returncode == code, (args, result.returncode, result.stdout, result.stderr)
            assert not result.stderr and b'\x1b' not in result.stdout
            assert b'SENTINEL_PRIVATE' not in result.stdout
            assert b'synthetic-fixture-token' not in result.stdout
            return json.loads(result.stdout)

        run('provider', 'setup', 'fixture', '--describe', code=2)
        assert not (root / 'state').exists()
        digest = hashlib.sha256(fixture.read_bytes()).hexdigest()
        args = ['provider', 'external', 'trust', str(fixture), '--id', 'fixture',
                '--sha256', digest, '--accept-risk']
        for cap in CAPS:
            args += ['--capability', cap]
        run(*args)
        described = run('provider', 'setup', 'fixture', '--describe')
        assert described['command'] == 'provider_setup_describe'
        spec = described['result']['spec']
        assert spec['schema_version'] == 1
        assert len(spec['steps']) == 4
        assert not config.exists(), 'Describe must not load or create a workspace'
        run('init')
        before = config.read_bytes()
        for values in ({}, {'token': 'SENTINEL_PRIVATE'}, {'token': 'env://TOKEN', 'client_id': 'inactive'},
                       {'token': 'env://TOKEN', 'unknown': True}, {'token': 'keychain://other/token'},
                       {'token': 'env://TOKEN', 'port': 65536}, {'token': 'env://TOKEN', 'enabled': 'true'}):
            answers.write_text(json.dumps({'version': 1, 'answers': values}))
            run('provider', 'setup', 'fixture', '--id', 'internal-main', '--answers', str(answers), code=2)
            assert config.read_bytes() == before
        answers.write_text('version: 1\nanswers:\n  token: env://PERMESH_SETUP_TOKEN\n  regions: [eu, us]\n  metadata: {owner: synthetic}\n')
        run('provider', 'setup', 'fixture', '--id', 'internal-main', '--answers', str(answers), '--authoritative')
        assert not (root / 'state/workspace-approvals').exists()
        review = run('provider', 'external', 'review', 'internal-main')['result']
        assert review['approved'] is False and review['registration']['sha256'] == digest
        values = review['configuration']
        assert values['port'] == 443 and values['enabled'] is True
        assert values['regions'] == ['eu', 'us'] and values['metadata'] == {'owner': 'synthetic'}
        assert 'client_id' not in values and 'token' not in values
        assert review['credential_references'] == {'token': 'env://PERMESH_SETUP_TOKEN'}
        run('doctor', code=3)
        run('provider', 'external', 'approve', 'internal-main', '--fingerprint', review['fingerprint'], '--accept-risk')
        assert run('doctor')['complete'] is True
        assert run('user', 'alice@example.com')['result']['access']
        after = config.read_bytes()
        run('provider', 'setup', 'fixture', '--id', 'internal-main', '--answers', str(answers), code=2)
        assert config.read_bytes() == after
        answers.write_text('version: 1\nanswers: {auth_method: service, client_id: synthetic-client, client_secret: keychain://service-main/client_secret}\n')
        run('provider', 'setup', 'fixture', '--id', 'service-main', '--answers', str(answers))
        review = run('provider', 'external', 'review', 'service-main')['result']
        assert review['configuration']['auth_method'] == 'service'
        assert review['configuration']['client_id'] == 'synthetic-client'
        assert review['credential_references'] == {'client_secret': 'keychain://service-main/client_secret'}
        assert review['approved'] is False

        if os.name != 'nt':
            import pty

            def interactive(name, action):
                directory = root / name
                directory.mkdir()
                run('init', cwd=directory)
                path = directory / 'permesh.yaml'
                original = path.read_bytes()
                master, slave = pty.openpty()
                child = subprocess.Popen([str(binary), 'provider', 'setup', 'fixture', '--id', name],
                                         cwd=directory, env=env, stdin=slave, stderr=slave,
                                         stdout=subprocess.PIPE)
                os.close(slave)
                try:
                    transcript = b''
                    deadline = time.monotonic() + 15
                    while b'> ' not in transcript and child.poll() is None and time.monotonic() < deadline:
                        if select.select([master], [], [], 0.1)[0]:
                            transcript += os.read(master, 65536)
                    assert b'> ' in transcript, transcript
                    if action == 'cancel':
                        child.send_signal(signal.SIGINT)
                        expected = 130
                    elif action == 'eof':
                        os.write(master, b'\x04')
                        expected = 130
                    else:
                        if action == 'change':
                            original = original.replace(b'My organization', b'Changed during setup')
                            path.write_bytes(original)
                        os.write(master, b'\nenv://PERMESH_SETUP_TOKEN\n\n\n\n\n\n')
                        expected = 2 if action == 'change' else 0
                    stdout, _ = child.communicate(timeout=15)
                    assert child.returncode == expected, (child.returncode, stdout, transcript)
                    if action != 'success':
                        assert path.read_bytes() == original
                    else:
                        assert path.read_bytes() != original
                        review = run('provider', 'external', 'review', name, cwd=directory)['result']
                        assert review['configuration']['port'] == 443 and review['approved'] is False
                finally:
                    if child.poll() is None:
                        child.kill()
                        child.communicate()
                    os.close(master)

            for name, action in [('interactive-main', 'success'), ('cancel-main', 'cancel'),
                                 ('eof-main', 'eof'), ('changed-main', 'change')]:
                interactive(name, action)
    print('Provider setup: describe, typed conditional answers, references, queries, prompts and cancellation passed.')


if __name__ == '__main__':
    check()
