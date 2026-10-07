"""Verify the actual packaged executable, checksum, notices and headless startup."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('archive', type=Path)
args = parser.parse_args()
archive = args.archive.resolve()
expected, filename = archive.with_suffix(archive.suffix + '.sha256').read_text().strip().split('  ', 1)
assert filename == archive.name
assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected
with tempfile.TemporaryDirectory(prefix='rtop-package-smoke-') as directory:
    root = Path(directory)
    with tarfile.open(archive) as tar:
        members = tar.getmembers()
        top = {Path(member.name).parts[0] for member in members}
        assert len(top) == 1
        for member in members:
            path = Path(member.name)
            assert not path.is_absolute() and '..' not in path.parts
            assert member.isdir() or member.isfile(), 'unexpected link or special archive entry'
            destination = root / path
            if member.isdir():
                destination.mkdir(parents=True, exist_ok=True)
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                with tar.extractfile(member) as source:
                    destination.write_bytes(source.read())
                destination.chmod(member.mode & 0o777)
    bundle = root / next(iter(top))
    binary = bundle / 'rtop'
    info = json.loads((bundle / 'BUILD-INFO.json').read_text())
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == info['binary_sha256']
    notices = json.loads((bundle / 'THIRD-PARTY.json').read_text())
    assert len(notices) == info['dependency_count'] and len(notices) > 0
    for notice in notices:
        assert notice['license'] and notice['files']
        for file in notice['files']:
            assert (bundle / file).is_file()
    assert (bundle / 'LICENSE').is_file() and (bundle / 'config/example.toml').is_file()
    env = {**os.environ, 'XDG_CONFIG_HOME': str(root / 'config')}
    version = subprocess.check_output([str(binary), '--version'], text=True, env=env, timeout=3).strip()
    assert version == 'rtop ' + info['version']
    sysroot = root / 'empty-sys'
    sysroot.mkdir()
    doctor = subprocess.check_output([str(binary), 'doctor', '--hardware-sysfs', str(sysroot),
                                     '--disable-intel', '--disable-nvml'], env=env, text=True, timeout=3)
    assert 'No GPU devices' in doctor and 'No temperature sensors' in doctor
    sample = subprocess.check_output([str(binary), '--collect', '1'], env=env, text=True, timeout=3)
    assert 'memory\t' in sample
report = {'status': 'PASS', 'version': info['version'], 'target': info['target'],
          'binary_sha256': info['binary_sha256'], 'dependencies_with_notices': len(notices),
          'checks': ['archive checksum', 'binary checksum', 'extract', 'licenses',
                     'packaged version', 'doctor without GPU', 'headless collection']}
Path('docs/benchmarks/m6-package-verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
