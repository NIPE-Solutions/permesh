"""Run synthetic Unix terminal acceptance against a supplied native CLI binary.

Uses a real controlling PTY, never live providers or valid credentials. Captured
terminal output stays in memory and is deliberately excluded from failures.
"""
import argparse
import errno
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time
import unittest
import uuid

if os.name == 'posix':
    import pty
    import termios


class Terminal:
    def __init__(self, binary, args, directory, env):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            try:
                os.chdir(directory)
                os.execve(str(binary), [str(binary), *args], env)
            finally:
                os._exit(127)
        self.output = bytearray()
        self.status = None
        self.deadline = time.monotonic() + 15

    def __enter__(self):
        return self

    def __exit__(self, *_):
        if self.status is None:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        os.close(self.fd)

    def tick(self):
        if time.monotonic() >= self.deadline:
            raise AssertionError('CLI terminal interaction timed out')
        if select.select([self.fd], [], [], 0.02)[0]:
            try:
                chunk = os.read(self.fd, 8192)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                chunk = b''
            self.output.extend(chunk)
            if len(self.output) > 65536:
                raise AssertionError('CLI terminal output exceeded acceptance limit')
        if self.status is None:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                self.status = os.waitstatus_to_exitcode(status)

    def prompt(self, text):
        while text not in self.output:
            if self.status is not None:
                raise AssertionError('CLI exited before expected terminal prompt')
            self.tick()

    def echo(self):
        return bool(termios.tcgetattr(self.fd)[3] & termios.ECHO)

    def hidden_input_ready(self):
        # The prompt is flushed just before rpassword disables terminal echo.
        while self.echo():
            if self.status is not None:
                raise AssertionError('CLI exited without disabling terminal echo')
            self.tick()

    def send(self, value):
        os.write(self.fd, value)

    def finish(self):
        while self.status is None:
            self.tick()
        # Drain remaining output after waitpid observes exit.
        while select.select([self.fd], [], [], 0)[0]:
            try:
                chunk = os.read(self.fd, 8192)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            self.output.extend(chunk)
        return self.status


class TerminalAcceptance(unittest.TestCase):
    binary = None

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='permesh-terminal-')
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.env = {key: value for key, value in os.environ.items()
                    if not key.startswith('PERMESH_')}
        self.instance = 'terminal-' + uuid.uuid4().hex
        self.env.update(NO_COLOR='1', TOKIO_WORKER_THREADS='2',
                        PERMESH_DATA_DIR=str(self.directory / 'state'))

    def terminal(self, *args):
        return Terminal(self.binary, args, self.directory, self.env)

    def run_json(self, *args):
        result = subprocess.run([str(self.binary), *args, '--json'],
                                cwd=self.directory, env=self.env, capture_output=True,
                                timeout=15)
        self.assertEqual(result.returncode, 0, 'Synthetic setup command failed')
        return json.loads(result.stdout)

    def login_workspace(self):
        self.run_json('init')
        self.run_json('provider', 'add', 'external', '--id', self.instance,
                      '--provider', 'fixture', '--sha256', 'a' * 64,
                      '--credential', f'token=keychain://{self.instance}/token')
        # Unique instance plus unconditional cleanup protects against unexpected
        # future acceptance of synthetic input. Failure output is never printed.
        self.addCleanup(lambda: subprocess.run(
            [str(self.binary), 'auth', 'logout', self.instance, '--json'],
            cwd=self.directory, env=self.env, capture_output=True, timeout=15))
        return (self.directory / 'permesh.yaml').read_bytes()

    def test_interactive_init_consumes_organization_answer(self):
        # Catches regressions that bypass the TTY prompt or discard its answer.
        with self.terminal('init', '--demo') as terminal:
            terminal.prompt(b'Organization name')
            terminal.send(b'Terminal acceptance\n')
            self.assertEqual(terminal.finish(), 0)
        report = self.run_json('doctor')
        self.assertEqual(report['result']['organization'], 'Terminal acceptance')

    def test_json_init_does_not_prompt_on_terminal(self):
        # No input is supplied: any accidental interactive read must time out.
        with self.terminal('init', '--json') as terminal:
            self.assertEqual(terminal.finish(), 0)
            report = json.loads(terminal.output)
            self.assertEqual(report['command'], 'init')
        self.assertEqual(self.run_json('doctor')['result']['organization'], 'My organization')

    def test_hidden_login_eof_restores_echo_without_exposing_input(self):
        original = self.login_workspace()
        with self.terminal('auth', 'login', self.instance) as terminal:
            terminal.prompt(b'Provider token (hidden)')
            terminal.hidden_input_ready()
            self.assertFalse(terminal.echo(), 'Credential entry must disable echo')
            # Clear the synthetic text then send EOF. Never send Enter with
            # nonempty input, even if line editing regresses.
            terminal.send(b'synthetic-terminal-secret\x15\x04')
            self.assertEqual(terminal.finish(), 2)
            self.assertTrue(terminal.echo(), 'Credential entry must restore echo')
            self.assertFalse(b'synthetic-terminal-secret' in terminal.output,
                             'Terminal output exposed synthetic credential input')
            self.assertTrue(b'Cannot read token from terminal' in terminal.output,
                            'Terminal EOF did not report a read error')
        self.assertEqual((self.directory / 'permesh.yaml').read_bytes(), original)

    def test_json_login_refuses_prompt_on_terminal(self):
        original = self.login_workspace()
        with self.terminal('auth', 'login', self.instance, '--json') as terminal:
            self.assertEqual(terminal.finish(), 2)
            self.assertFalse(b'Provider token (hidden)' in terminal.output)
            self.assertTrue(b'--token-stdin' in terminal.output)
            self.assertTrue(terminal.echo())
        self.assertEqual((self.directory / 'permesh.yaml').read_bytes(), original)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    args = parser.parse_args()
    if os.name != 'posix':
        parser.error('Unix controlling PTYs required; Windows terminal acceptance is separate')
    TerminalAcceptance.binary = args.binary.resolve(strict=True)
    unittest.main(argv=[__file__], verbosity=2)
