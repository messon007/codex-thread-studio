#!/usr/bin/env python3
"""Verify a worker component before packaging/installing its declared files."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(component):
    repo = Path(__file__).resolve().parent.parent
    source = repo / 'native/mermaid-worker'
    component = component.resolve(strict=True)
    manifest = json.loads((component / 'MERMAID-WORKER.json').read_text())
    version = int(re.search(r'pub const VERSION: u8 = (\d+);', (source / 'src/protocol.rs').read_text())[1])
    if manifest['protocolVersion'] != version:
        raise RuntimeError('Mermaid worker protocol does not match this native application')
    files = []
    for relative, expected in manifest['filesSha256'].items():
        path = PurePosixPath(relative)
        if path.is_absolute() or '..' in path.parts or not path.parts:
            raise RuntimeError('Invalid worker component pathname')
        item = component / relative
        if not item.resolve(strict=True).is_relative_to(component) or item.is_symlink() or digest(item) != expected:
            raise RuntimeError(f'Mermaid component differs from its manifest: {relative}')
        if relative != 'codex-thread-studio-mermaid' and not relative.startswith('licenses/mermaid/'):
            raise RuntimeError(f'Unexpected worker package destination: {relative}')
        files.append(relative)
    if 'codex-thread-studio-mermaid' not in files or not os.access(component / 'codex-thread-studio-mermaid', os.X_OK) or digest(component / 'codex-thread-studio-mermaid') != manifest['binarySha256']:
        raise RuntimeError('Missing or changed Mermaid executable')
    for relative, expected in manifest['sourceSha256'].items():
        if relative not in {str(p.relative_to(source)) for p in (source / 'src').glob('*.rs')} or digest(source / relative) != expected:
            raise RuntimeError(f'Mermaid worker source does not match this checkout: {relative}')
    for relative, expected in manifest['patchesSha256'].items():
        if relative not in {p.name for p in (source / 'patches').glob('*.patch')} or digest(source / 'patches' / relative) != expected:
            raise RuntimeError(f'Mermaid worker patch does not match this checkout: {relative}')
    if set(manifest['sourceSha256']) != {str(p.relative_to(source)) for p in (source / 'src').glob('*.rs')}:
        raise RuntimeError('Worker manifest omits current source files')
    if set(manifest['patchesSha256']) != {p.name for p in (source / 'patches').glob('*.patch')}:
        raise RuntimeError('Worker manifest omits current renderer patches')
    return component, manifest, files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('component', type=Path)
    parser.add_argument('--install-to', type=Path)
    args = parser.parse_args()
    component, manifest, files = verify(args.component)
    if args.install_to:
        out = args.install_to.resolve()
        for relative in files:
            dest = out / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(component / relative, dest)
            if digest(dest) != manifest['filesSha256'][relative]:
                raise RuntimeError(f'Worker file changed during packaging: {relative}')
        shutil.copy2(component / 'MERMAID-WORKER.json', out / 'MERMAID-WORKER.json')
    print(json.dumps(dict(status='passed', workerSha256=manifest['binarySha256'], files=len(files))))


if __name__ == '__main__':
    main()
