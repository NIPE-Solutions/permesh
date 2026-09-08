# SPDX-License-Identifier: MIT OR Apache-2.0
"""Offline native-provider workspace queries, approval boundaries and credential delivery."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

CAPABILITIES = ('accounts', 'identities', 'resources', 'groups', 'memberships', 'grants')
TOKEN = 'synthetic-fixture-token'


def check():
    suffix = '.exe' if os.name == 'nt' else ''
    target = Path('target/debug').resolve()
    binary = target / ('permesh' + suffix)
    fixture = target / ('permesh-test-external-peer' + suffix)
    with tempfile.TemporaryDirectory(prefix='permesh-queries-') as temporary:
        root = Path(temporary).resolve()
        state = root / 'state'
        env = dict(os.environ, PERMESH_DATA_DIR=str(state), TOKIO_WORKER_THREADS='2',
                   NO_COLOR='1', PERMESH_FIXTURE_TOKEN=TOKEN)
        workspace = root / 'original'
        workspace.mkdir()
        path = workspace / 'permesh.yaml'
        digest = hashlib.sha256(fixture.read_bytes()).hexdigest()

        def run(*args, code=0, cwd=workspace, environment=None):
            output = subprocess.run([str(binary), *args, '--json'], cwd=cwd,
                                    env=environment or env, capture_output=True, timeout=80)
            assert output.returncode == code, (args, output.returncode, output.stdout, output.stderr)
            assert output.stderr == b'', output.stderr
            assert b'\x1b' not in output.stdout
            assert TOKEN.encode() not in output.stdout + output.stderr
            return json.loads(output.stdout)

        run('init')
        run('provider', 'add', 'external', '--id', 'internal-main', '--provider', 'fixture',
            '--sha256', digest, '--credential', 'token=env://PERMESH_FIXTURE_TOKEN',
            '--setting', 'mode=normal', '--authoritative')
        original = path.read_bytes()
        assert run('doctor', code=3)['complete'] is False
        assert not state.exists(), 'Query must not create trust storage'
        args = ['provider', 'external', 'trust', str(fixture), '--id', 'fixture',
                '--sha256', digest, '--accept-risk']
        for capability in CAPABILITIES:
            args += ['--capability', capability]
        run(*args)
        run('doctor', code=3)  # Registration alone never authorizes workspace execution.
        assert not (state / 'workspace-approvals').exists()

        def approve(instance='internal-main'):
            review = run('provider', 'external', 'review', instance)['result']
            fingerprint = review['fingerprint']
            run('provider', 'external', 'approve', instance, '--fingerprint', fingerprint,
                '--accept-risk')
            return fingerprint

        run('provider', 'external', 'approve', 'internal-main', '--fingerprint', '0' * 64,
            '--accept-risk', code=2)
        fingerprint = approve()
        assert run('doctor')['complete'] is True
        assert run('provider', 'status', 'internal-main')['complete'] is True
        assert run('provider', 'capabilities', 'internal-main')['result']['metadata']['capabilities']
        assert run('auth', 'status')['complete'] is True
        user = run('user', 'alice@example.com')
        assert user['result']['access'], user
        assert run('user', 'alice-dev')['result']['access'] == user['result']['access']
        # Selecting a different trusted digest must not redirect pinned queries.
        # This non-executable header fixture would fail if the selected binary ran.
        replacement = root / 'never-executed-provider'
        replacement.write_bytes(b'\x7fELFsynthetic-retained-pin-test')
        replacement_digest = hashlib.sha256(replacement.read_bytes()).hexdigest()
        replacement_args = ['provider', 'external', 'trust', str(replacement), '--id', 'fixture',
                            '--sha256', replacement_digest, '--accept-risk']
        for capability in CAPABILITIES:
            replacement_args += ['--capability', capability]
        run(*replacement_args)
        review = run('provider', 'external', 'review', 'internal-main')['result']
        assert review['registration']['sha256'] == digest and review['approved'] is True
        assert run('user', 'alice@example.com')['result']['access'] == user['result']['access']
        assert run('admins')['result']['access']
        assert run('orphaned')['result']
        human = subprocess.run([str(binary), 'user', 'alice@example.com'], cwd=workspace,
                               env=env, capture_output=True, timeout=80)
        assert human.returncode == 0 and b'backend' in human.stdout.lower(), (human.returncode, human.stdout, human.stderr)
        assert TOKEN.encode() not in human.stdout + human.stderr
        assert path.read_bytes() == original, 'Inspection must not rewrite config'
        clone = root / 'clone'
        clone.mkdir()
        shutil.copyfile(path, clone / 'permesh.yaml')
        run('doctor', code=3, cwd=clone)
        missing = dict(env)
        missing.pop('PERMESH_FIXTURE_TOKEN')
        run('doctor', code=3, environment=missing)
        # Changing a reference/config invalidates approval before credentials are resolved.
        path.write_text(original.decode().replace('mode: normal', 'mode: partial'))
        run('doctor', code=3)
        run('provider', 'external', 'approve', 'internal-main', '--fingerprint', fingerprint,
            '--accept-risk', code=2)
        approve()
        partial = run('user', 'alice@example.com', code=4)
        assert partial['complete'] is False and partial['result']['access']
        orphaned = run('orphaned', code=4)['result']
        assert orphaned['authority_complete'] is False
        assert all(account['reason'] == 'unassessed' for account in orphaned['accounts'])
        # Protocol strings cannot reflect known credentials, including escaped JSON strings.
        for mode in ('echo', 'echo-escaped', 'echo-key'):
            path.write_text(original.decode().replace('mode: normal', 'mode: ' + mode))
            approve()
            run('user', 'alice@example.com', code=3)
        path.write_text(original.decode().replace('mode: normal', 'mode: echo-stderr'))
        approve()
        assert run('user', 'alice@example.com')['result']['access']
        path.write_text(original.decode().replace('mode: normal', 'mode: checkfail'))
        approve()
        run('doctor', code=3)
        run('user', 'alice@example.com', code=3)
        path.write_bytes(original)
        approve()
        run('provider', 'external', 'revoke', 'internal-main')
        run('doctor', code=3)
        # A failed external instance must preserve built-in results and mark incompleteness.
        config = {'version': 1, 'organization': {'name': 'Synthetic'},
            'providers': [{'id': 'demo', 'type': 'demo'}, {'id': 'internal-main', 'type': 'external',
                'external': {'provider': 'fixture', 'sha256': digest, 'configuration': {},
                             'credentials': {'token': 'env://PERMESH_FIXTURE_TOKEN'}}}],
            'identity': {'sources': [{'provider': 'demo', 'authoritative': True}]}}
        path.write_text(json.dumps(config))  # JSON is valid YAML; no extra parser dependency.
        mixed = run('user', 'alice@example.com', code=4)
        assert mixed['result']['access'] and mixed['complete'] is False
        # Distinct instances receive only their own named credential values.
        config['providers'] = []
        config['identity']['sources'] = []
        for name, secret in (('two-first', 'first-private-slot'), ('two-second', 'second-private-slot')):
            variable = name.upper().replace('-', '_')
            env[variable] = secret
            config['providers'].append({'id': name, 'type': 'external', 'external': {
                'provider': 'fixture', 'sha256': digest, 'configuration': {},
                'credentials': {'client_secret': 'env://' + variable}}})
        path.write_text(json.dumps(config))
        for name in ('two-first', 'two-second'):
            approve(name)
        assert run('doctor')['complete'] is True
        # SIGINT reaches an active workspace process and waits for supervised cleanup.
        if os.name != 'nt':
            config['providers'] = [config['providers'][0]]
            config['providers'][0]['external']['configuration'] = {'mode': 'test-hang'}
            path.write_text(json.dumps(config))
            approve('two-first')
            child = subprocess.Popen([str(binary), 'doctor', '--json'], cwd=workspace,
                                     env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                ready = state / 'providers/fixture/ready'
                deadline = time.monotonic() + 10
                while not ready.exists() and child.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.02)
                assert ready.exists() and child.poll() is None, 'External health process did not become ready'
                child.send_signal(signal.SIGINT)
                stdout, stderr = child.communicate(timeout=10)
                assert child.returncode == 130, (child.returncode, stdout, stderr)
                assert json.loads(stdout)['error'] and not stderr
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate()
    print('External workspace queries: approvals, credentials, health, access, partial results and cancellation passed.')


if __name__ == '__main__':
    check()
