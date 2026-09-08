"""Exercise only synthetic demo data in an automatically removed workspace."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def smoke(binary):
    binary = Path(binary).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='permesh-candidate-') as directory:
        workspace = Path(directory)
        env = {key: value for key, value in os.environ.items() if not key.startswith('PERMESH_')}
        env['NO_COLOR'] = '1'

        def run(*args):
            result = subprocess.run([str(binary), *args], cwd=workspace, env=env,
                                    text=True, capture_output=True, timeout=60, check=True)
            if '\x1b' in result.stdout:
                raise RuntimeError('non-terminal output contains ANSI escapes')
            return result.stdout

        run('--help')
        run('--version')
        run('init', '--demo')
        config = workspace / 'permesh.yaml'
        original = config.read_bytes()
        run('--config', str(config), 'doctor')
        human = run('--config', str(config), 'user', 'alice@example.com')
        if 'acme/payments-api' not in human or 'team/backend' not in human:
            raise RuntimeError('demo user access missing')
        for command in [('user', 'alice@example.com'), ('admins',)]:
            report = json.loads(run('--config', str(config), *command, '--json'))
            if report['schema_version'] != 1 or report['complete'] is not True or not report['result']['access']:
                raise RuntimeError('unexpected demo report')
        if config.read_bytes() != original:
            raise RuntimeError('inspection changed shared configuration')
    print('Synthetic candidate smoke passed; temporary workspace removed.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    smoke(parser.parse_args().binary)
