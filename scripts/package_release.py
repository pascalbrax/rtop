"""Build a Linux release candidate, notices and SHA-256 archive; no publication."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    args = parser.parse_args()
    if platform.system() != 'Linux':
        parser.error('Linux host required; cross compilation is not supported by this script')
    flags = ['--locked'] + (['--offline'] if args.offline else [])
    subprocess.run(['cargo', 'build', '--release', *flags], cwd=ROOT, check=True)
    host = next(line.split(': ', 1)[1] for line in output('rustc', '-vV').splitlines()
                if line.startswith('host: '))
    metadata = json.loads(output('cargo', 'metadata', '--format-version', '1',
                                 '--filter-platform', host, *flags))
    project = next(p for p in metadata['packages'] if p['id'] in metadata['workspace_members'])
    binary = Path(metadata['target_directory']) / 'release' / 'rtop'
    version = project['version']
    if output(str(binary), '--version') != f'rtop {version}':
        raise RuntimeError('binary version does not match Cargo metadata')
    name = f'rtop-{version}-{host}'
    args.output.mkdir(parents=True, exist_ok=True)
    # Include the entire resolved host dependency graph (also build dependencies).
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    with tempfile.TemporaryDirectory(prefix='rtop-package-') as directory:
        bundle = Path(directory) / name
        bundle.mkdir()
        shutil.copy2(binary, bundle / 'rtop')
        for file in ['LICENSE', 'README.md', 'CHANGELOG.md', 'config/example.toml',
                     'ROADMAP.md', 'MILESTONES.md']:
            destination = bundle / file
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / file, destination)
        running_soak = ROOT / 'docs/benchmarks/m6-tui-soak.status.json'
        skip_soak = running_soak.exists() and json.loads(running_soak.read_text())['state'] != 'passed'
        shutil.copytree(ROOT / 'docs', bundle / 'docs',
                        ignore=lambda directory, names: [name for name in names
                        if skip_soak and name.startswith('m6-tui-soak')])
        notices = []
        for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
            if package['id'] not in resolved or package['id'] == project['id']:
                continue
            source = Path(package['manifest_path']).parent
            files = [f for f in source.iterdir() if f.is_file() and
                     f.name.lower().startswith(('license', 'licence', 'copying', 'notice', 'copyright'))]
            if not files or not package.get('license'):
                raise RuntimeError(f"missing license information for {package['name']} {package['version']}")
            destination = bundle / 'licenses' / f"{package['name']}-{package['version']}"
            destination.mkdir(parents=True)
            for file in files:
                shutil.copy2(file, destination / file.name)
            notices.append({'name': package['name'], 'version': package['version'],
                            'license': package['license'], 'repository': package.get('repository'),
                            'files': [str((destination / f.name).relative_to(bundle)) for f in files]})
        (bundle / 'THIRD-PARTY.json').write_text(json.dumps(notices, indent=2) + '\n')
        # Host-built GNU binary, not a universal/static Linux executable.
        build = {'version': version, 'target': host, 'rustc': output('rustc', '--version'),
                 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                 'source_revision': os.environ.get('RTOP_SOURCE_REVISION', 'unspecified'),
                 'libc': platform.libc_ver(), 'system': platform.platform(),
                 'dynamic_dependencies': re.sub(r'\s+\(0x[0-9a-fA-F]+\)', '', output('ldd', str(binary))),
                 'cargo_lock_sha256': hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
                 'dependency_count': len(notices), 'status': 'release candidate; see docs/RELEASE.md'}
        (bundle / 'BUILD-INFO.json').write_text(json.dumps(build, indent=2) + '\n')
        archive = args.output / f'{name}.tar.gz'
        # Stable file order, owner, permissions and timestamps for the same build/content.
        with archive.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode='w') as tar:
                for file in [bundle, *sorted(bundle.rglob('*'))]:
                    info = tar.gettarinfo(str(file), arcname=str(file.relative_to(bundle.parent)))
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ''
                    info.mode = 0o755 if file.is_dir() or file.name == 'rtop' else 0o644
                    if file.is_file():
                        with file.open('rb') as contents:
                            tar.addfile(info, contents)
                    else:
                        tar.addfile(info)
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_suffix(archive.suffix + '.sha256').write_text(f'{checksum}  {archive.name}\n')
        (args.output / 'build-info.json').write_text(json.dumps(build, indent=2) + '\n')
        print(json.dumps({'archive': str(archive), 'sha256': checksum, **build}, indent=2))


if __name__ == '__main__':
    main()
