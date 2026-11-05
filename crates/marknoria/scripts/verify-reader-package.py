#!/usr/bin/env python3
"""Verify a fixed reader package without source checkout, network or credentials."""
import argparse
import hashlib
import json
from pathlib import Path


def verify(root):
    root = Path(root).resolve()
    manifest = json.loads((root / 'reader-manifest.json').read_text())
    if manifest.get('version') != 1:
        raise ValueError('Unsupported reader package manifest')
    expected = manifest['files']
    actual = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'Symlinks are not allowed: {path.relative_to(root)}')
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative == 'reader-manifest.json':
            continue
        with path.open('rb') as stream:
            actual[relative] = hashlib.file_digest(stream, 'sha256').hexdigest()
    if actual != expected:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        changed = sorted(key for key in set(actual) & set(expected) if actual[key] != expected[key])
        raise ValueError(f'Package mismatch: missing={missing}, extra={extra}, changed={changed}')
    digest = hashlib.sha256(json.dumps(expected, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    if digest != manifest['treeSha256']:
        raise ValueError('Package tree digest mismatch')
    return {'ok': True, 'revision': manifest['revision'], 'files': len(actual), 'treeSha256': digest}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory')
    print(json.dumps(verify(parser.parse_args().directory), sort_keys=True))
