#!/usr/bin/env python3
"""Import only the externally pinned, unmodified public reader source package."""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path, PurePosixPath
import tarfile
import tempfile

MAX_ARCHIVE = 256 * 1024 * 1024
MAX_TOTAL = 512 * 1024 * 1024
MAX_FILE = 128 * 1024 * 1024
MAX_FILES = 8192


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        while data := stream.read(1024 * 1024):
            result.update(data)
    return result.hexdigest()


def verify(root, pin):
    root = Path(root)
    manifest_path = root / 'reader-manifest.json'
    if manifest_path.is_symlink() or manifest_path.stat().st_size > 4 * 1024 * 1024:
        raise ValueError('Invalid reader manifest')
    manifest = json.loads(manifest_path.read_text())
    for key in ('version', 'revision', 'treeSha256', 'apiVersion', 'bridgeVersion'):
        if manifest.get(key) != pin[key]:
            raise ValueError(f'Reader {key} does not match reviewed pin')
    expected = manifest.get('files')
    if not isinstance(expected, dict) or len(expected) > MAX_FILES:
        raise ValueError('Invalid reader inventory')
    actual, total = {}, 0
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError('Reader package contains a symlink')
        if path.is_dir():
            continue
        if not path.is_file() or path.stat().st_size > MAX_FILE:
            raise ValueError('Invalid reader inventory entry')
        total += path.stat().st_size
        if total > MAX_TOTAL or len(actual) > MAX_FILES:
            raise ValueError('Reader inventory exceeds budget')
        name = path.relative_to(root).as_posix()
        if name != 'reader-manifest.json':
            actual[name] = digest(path)
    if actual != expected:
        raise ValueError('Reader files differ from the fixed inventory')
    tree = hashlib.sha256(json.dumps(expected, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    if tree != pin['treeSha256']:
        raise ValueError('Reader inventory digest does not match reviewed pin')
    return {'revision': pin['revision'], 'files': len(actual), 'treeSha256': tree}


def unpack(archive, staging, pin):
    archive, staging = Path(archive), Path(staging)
    if archive.stat().st_size > MAX_ARCHIVE or digest(archive) != pin['archiveSha256']:
        raise ValueError('Reader archive does not match reviewed SHA-256')
    seen, total, files = set(), 0, 0
    with tarfile.open(archive, 'r:gz') as source:
        for member in source:
            name = PurePosixPath(member.name)
            if (not name.parts or name.parts[0] != 'marknoria-reader'
                    or name.is_absolute() or '\\' in member.name or '\0' in member.name
                    or any(part in ('', '.', '..') for part in member.name.split('/'))
                    or name.as_posix() in seen):
                raise ValueError('Unsafe or duplicate reader archive path')
            seen.add(name.as_posix())
            if len(seen) > MAX_FILES + 256:
                raise ValueError('Reader archive entry budget exceeded')
            destination = staging.joinpath(*name.parts)
            if member.isdir():
                destination.mkdir(parents=True, exist_ok=True)
                continue
            if not member.isfile() or member.size < 0 or member.size > MAX_FILE:
                raise ValueError('Unsupported reader archive entry')
            files += 1
            total += member.size
            if files > MAX_FILES or total > MAX_TOTAL:
                raise ValueError('Reader archive byte/file budget exceeded')
            destination.parent.mkdir(parents=True, exist_ok=True)
            with source.extractfile(member) as incoming, destination.open('xb') as outgoing:
                remaining = member.size
                while remaining:
                    data = incoming.read(min(remaining, 1024 * 1024))
                    if not data:
                        raise ValueError('Truncated reader archive entry')
                    outgoing.write(data)
                    remaining -= len(data)
            destination.chmod(0o755 if member.mode & 0o111 else 0o644)
    result = staging / 'marknoria-reader'
    verify(result, pin)
    return result


def install(archive, root, pin, check=False):
    root = Path(root)
    crates = root / 'crates'
    crates.mkdir(exist_ok=True)
    destination = crates / 'marknoria'
    if destination.is_symlink():
        raise ValueError('Reader destination must not be a symlink')
    staging = Path(tempfile.mkdtemp(prefix='.marknoria-import-', dir=crates))
    keep_staging = False
    try:
        candidate = unpack(archive, staging, pin)
        report = verify(candidate, pin)
        if not check:
            previous = staging / 'previous'
            existed = destination.exists()
            if existed:
                os.replace(destination, previous)
            try:
                os.replace(candidate, destination)
            except BaseException:
                if existed:
                    try:
                        os.replace(previous, destination)
                    except OSError as rollback_error:
                        keep_staging = True
                        raise RuntimeError(f'Import failed; previous source preserved at {previous}') from rollback_error
                raise
        return {**report, 'installed': not check}
    finally:
        if not keep_staging:
            shutil.rmtree(staging)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', nargs='?', help='Reviewed local .tar.gz; no checkout or network access')
    parser.add_argument('--check', action='store_true', help='Verify archive without replacing existing source')
    parser.add_argument('--verify', action='store_true', help='Verify the installed inventory without building')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    pin = json.loads((root / 'scripts/marknoria-release.json').read_text())
    if args.verify:
        if args.archive or args.check:
            parser.error('--verify takes no archive or --check')
        report = verify(root / 'crates/marknoria', pin)
    else:
        if not args.archive:
            parser.error('a reviewed local archive is required')
        report = install(args.archive, root, pin, args.check)
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
