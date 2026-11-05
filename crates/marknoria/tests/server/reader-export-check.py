#!/usr/bin/env python3
"""Check deterministic fixed packages and strict extracted-file verification."""
import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('first')
parser.add_argument('second')
parser.add_argument('--temporary-root', default='target')
args = parser.parse_args()
sys.dont_write_bytecode = True
first, second = Path(args.first), Path(args.second)
with first.open('rb') as stream:
    one = hashlib.file_digest(stream, 'sha256').hexdigest()
with second.open('rb') as stream:
    two = hashlib.file_digest(stream, 'sha256').hexdigest()
assert one == two, 'Same-revision package hashes differ'
checks = 1
with tempfile.TemporaryDirectory(prefix='reader-export-contract-', dir=args.temporary_root) as temporary:
    temporary = Path(temporary).resolve()
    with tarfile.open(first, 'r:gz') as archive:
        archive.extractall(temporary, filter='data')
    root = Path(temporary) / 'marknoria-reader'
    spec = importlib.util.spec_from_file_location('reader_package_verify', root / 'scripts/verify-reader-package.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    report = module.verify(root)
    assert report['ok']
    checks += 1
    manifest = json.loads((root / 'reader-manifest.json').read_text())
    assert all(not name.startswith(('native/', 'ffi/', 'src/native/', 'node_modules/', 'target/'))
               for name in manifest['files'])
    assert tomllib.loads((root / '.cargo/config.toml').read_text()) == {
        'env': {f'{kind}_{arch}_pc_windows_msvc': '/utf-8'
                for kind in ['CFLAGS', 'CXXFLAGS']
                for arch in ['x86_64', 'aarch64', 'i686']}}
    checks += 1
    readme = root / 'README.md'
    original = readme.read_bytes()
    readme.write_bytes(original + b'\nchanged\n')
    try:
        module.verify(root)
        raise AssertionError('Changed source was accepted')
    except ValueError as error:
        assert 'README.md' in str(error)
    checks += 1
    readme.write_bytes(original)
    extra = root / 'unexpected-private-document.txt'
    extra.write_text('Synthetic check only')
    try:
        module.verify(root)
        raise AssertionError('Unexpected file was accepted')
    except ValueError as error:
        assert 'unexpected-private-document.txt' in str(error)
    checks += 1
    extra.unlink()
    link = root / 'unexpected-link'
    link.symlink_to(readme)
    try:
        module.verify(root)
        raise AssertionError('Symlink was accepted')
    except ValueError as error:
        assert 'Symlinks' in str(error)
    checks += 1
    link.unlink()
    assert module.verify(root)['treeSha256'] == report['treeSha256']
    checks += 1
    # Windows-style Git checkout settings must not rewrite reviewed source or
    # third-party notices. Keep Git metadata outside the verified inventory.
    assert '* -text' in (root / '.gitattributes').read_text()
    git_directory = Path(temporary) / 'inventory.git'
    subprocess.run(['git', 'init', '--quiet', '--bare', str(git_directory)], check=True)
    attributes = Path(temporary) / 'empty-attributes'
    attributes.write_text('')
    git = ['git', '--git-dir', str(git_directory), '--work-tree', str(root),
           '-c', 'core.autocrlf=true', '-c', 'core.safecrlf=false',
           '-c', f'core.attributesFile={attributes}']
    subprocess.run(git + ['add', '--all'], cwd=root, check=True, capture_output=True)
    entries = subprocess.check_output(git + ['ls-files', '--stage', '-z'], cwd=root).split(b'\0')
    expected = {**manifest['files'], 'reader-manifest.json': hashlib.sha256((root / 'reader-manifest.json').read_bytes()).hexdigest()}
    actual = {}
    with subprocess.Popen(git + ['cat-file', '--batch'], cwd=root,
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE) as objects:
        for entry in entries:
            if not entry:
                continue
            metadata, name = entry.split(b'\t', 1)
            mode, object_id, stage = metadata.split()
            assert mode in (b'100644', b'100755') and stage == b'0'
            objects.stdin.write(object_id + b'\n')
            objects.stdin.flush()
            header = objects.stdout.readline().split()
            assert len(header) == 3 and header[1] == b'blob'
            remaining = int(header[2])
            digest = hashlib.sha256()
            while remaining:
                chunk = objects.stdout.read(min(remaining, 1024 * 1024))
                assert chunk, 'Truncated Git inventory blob'
                digest.update(chunk)
                remaining -= len(chunk)
            assert objects.stdout.read(1) == b'\n'
            actual[name.decode()] = digest.hexdigest()
        objects.stdin.close()
        assert objects.wait() == 0
    assert actual == expected, 'Git checkout settings changed the package inventory'
    checks += 1
print(json.dumps({'ok': True, 'checks': checks, 'sha256': one, **report}, sort_keys=True))
