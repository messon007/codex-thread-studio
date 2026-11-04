#!/usr/bin/env python3
"""Prepare the exact downstream patch in an external source cache (build tool only)."""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
import uuid

VERSION = '0.9.3'
CRATE_SHA256 = '3cf1f994f9fe91cd7f5e0fbb4f7c9982585813c79aacad50d9351a23ee4d1520'
ORIGIN = '.studio-native-paint-origin.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_hash(root):
    rows = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            if path.is_symlink():
                raise RuntimeError(f'Unexpected source symlink: {path}')
            if path.is_file() and path != root / ORIGIN:
                rows[str(path.relative_to(root))] = digest(path)
    return hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest()


def verify(root, patch_sha, crate_sha=CRATE_SHA256):
    record = json.loads((root / ORIGIN).read_text())
    if (record['crateSha256'], record['patchSha256'], record['treeSha256']) != (
            crate_sha, patch_sha, tree_hash(root)):
        raise RuntimeError(f'Prepared source differs from its fingerprint: {root}')


COMPONENTS = {
    'merman-render': (VERSION, CRATE_SHA256, 'native-paint'),
    'merman-export': (VERSION, '0906c39b0c37047d2ae3d7d2ce59d033ce9dc23a82ae3be618fdb494e0f466b0', 'font-generics'),
    'usvg': ('0.47.0', 'd46cf96c5f498d36b7a9693bc6a7075c0bb9303189d61b2249b0dc3d309c07de', 'font-clusters'),
}


def prepare(crate, crate_sha, patch, cache, cargo_home, workspace, offline, link_source, version=VERSION):
    patch_sha = digest(patch)
    cache.mkdir(parents=True, exist_ok=True)
    archives = sorted((cargo_home / 'registry' / 'cache').glob(f'*/{crate}-{version}.crate'))
    archive = next((p for p in archives if digest(p) == crate_sha), None)
    downloaded = cache / f'{crate}-{version}-{crate_sha[:16]}.crate'
    if archive is None and downloaded.is_file() and digest(downloaded) == crate_sha:
        archive = downloaded
    if archive is None and not offline:
        # Download only the pinned crate, before Cargo can resolve its local patch.
        # All other dependencies still come from Cargo and the workspace lockfile.
        url = f'https://static.crates.io/crates/{crate}/{crate}-{version}.crate'
        with urllib.request.urlopen(url, timeout=30) as response:
            body = response.read(16 * 1024 * 1024 + 1)
        if len(body) > 16 * 1024 * 1024 or hashlib.sha256(body).hexdigest() != crate_sha:
            raise RuntimeError('Pinned Merman crate download failed size/checksum validation')
        temporary = cache / f'.archive-{uuid.uuid4().hex}.tmp'
        temporary.write_bytes(body)
        temporary.replace(downloaded)
        archive = downloaded
    if archive is None:
        raise RuntimeError('Pinned Merman crate archive is missing or has an unexpected checksum')
    destination = cache / f'{crate}-{version}-{patch_sha[:16]}'
    if destination.exists():
        verify(destination, patch_sha, crate_sha)
    else:
        stage = Path(tempfile.mkdtemp(prefix='.merman-paint-stage-', dir=cache))
        # Independently bound extraction even though the archive is content-pinned.
        with tarfile.open(archive, 'r:gz') as handle:
            members = handle.getmembers()
            if len(members) > 4096 or sum(m.size for m in members) > 64 * 1024 * 1024:
                raise RuntimeError(f'Crate extraction budget exceeded; stage retained: {stage}')
            for member in members:
                parts = PurePosixPath(member.name).parts
                if (not parts or parts[0] != f'{crate}-{version}' or '..' in parts
                        or not (member.isfile() or member.isdir())):
                    raise RuntimeError(f'Unexpected crate entry: {member.name}')
            handle.extractall(stage, members=members, filter='data')
        source = stage / f'{crate}-{version}'
        try:
            subprocess.run(['patch', '--batch', '--forward', '--fuzz=0', '-p1', '-i', str(patch)], cwd=source, check=True)
        except subprocess.CalledProcessError as error:
            raise RuntimeError(f'Patch failed; staged source retained at {stage}') from error
        record = dict(version=version, crateSha256=crate_sha, patchSha256=patch_sha, treeSha256=tree_hash(source))
        (source / ORIGIN).write_text(json.dumps(record, indent=2) + '\n')
        try:
            source.rename(destination)
        except OSError as error:
            if error.errno not in (errno.EEXIST, errno.ENOTEMPTY):
                raise
            verify(destination, patch_sha, crate_sha)
        shutil.rmtree(stage)  # Only the successful invocation's own staging directory.
        verify(destination, patch_sha, crate_sha)
    if link_source:
        vendor = workspace / 'vendor'
        if vendor.is_symlink():
            raise RuntimeError(f'Preserve the existing vendor directory link before preparing: {vendor}')
        vendor.mkdir(exist_ok=True)
        link = vendor / crate
        if link.exists() and not link.is_symlink():
            raise RuntimeError(f'Preserve the existing source directory before preparing: {link}')
        temporary = vendor / f'.{crate}-{uuid.uuid4().hex}.tmp'
        temporary.symlink_to(destination, target_is_directory=True)
        temporary.replace(link)
    return dict(crate=crate, source=str(destination), patchSha256=patch_sha, linked=link_source)


def main():
    project = Path(__file__).resolve().parent.parent
    worker = project / 'native' / 'mermaid-worker'
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, default=worker)
    parser.add_argument('--component', choices=['all', *COMPONENTS], default='all')
    parser.add_argument('--cache', type=Path)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--no-link', action='store_true')
    parser.add_argument('--cargo-config', type=Path,
                        help='Write canonical patch paths for Cargo dependency identity')
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    target = Path(os.environ.get('CARGO_TARGET_DIR', workspace / 'target'))
    cache = (args.cache or target / 'native-mermaid-paint-sources').resolve()
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')).resolve()
    rows = []
    for crate, (version, crate_sha, name) in COMPONENTS.items():
        if args.component not in ['all', crate]:
            continue
        patch = worker / 'patches' / f'{crate}-{version}-{name}.patch'
        rows.append(prepare(crate, crate_sha, patch, cache, cargo_home, workspace, args.offline, not args.no_link, version))
    if args.cargo_config:
        # Cargo dep-info can retain the previous canonical source paths when a
        # stable vendor symlink is retargeted. Make the content-addressed source
        # directory part of Cargo's package identity, without cleaning other work.
        config = args.cargo_config.resolve()
        config.write_text('[patch.crates-io]\n' + ''.join(
            f'{row["crate"]} = {{ path = {json.dumps(row["source"], ensure_ascii=False)} }}\n'
            for row in rows))
    print(json.dumps(dict(**rows[0], components=rows)))


if __name__ == '__main__':
    main()
