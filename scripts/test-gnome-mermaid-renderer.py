#!/usr/bin/env python3
"""Run pinned renderer library tests without modifying its prepared source cache."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
import urllib.request

COMMIT = '3b915f69c02d55b873d252cf6c8a6048e7f3427d'
FIXTURES = {
    'upstream-svgs/state/upstream_stateDiagram_concurrent_state_spec.svg':
        '4e0ee9998ecf613720070cdfef42703c5d55f458413e4c2550d6f459a8f9e340',
    'sequence/upstream_pkgtests_sequencediagram_spec_094.mmd':
        '3e0aef38fc294c09bbebd40cdc9ea5d2a4f24b6af5d25cbea6f8fdbe054c27a4',
    'sequence/upstream_duplicate_participants_single_actor_spec.mmd':
        '906e522ebaf841c18cf6a80328023c1c152faa4526dd9944a6f8f5da7c605f8a',
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--fixture-cache', type=Path)
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    helper = repo / 'scripts/prepare-gnome-mermaid.py'
    spec = importlib.util.spec_from_file_location('prepare_renderer', helper)
    prepare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(prepare)
    source = (repo / 'native/mermaid-worker/vendor/merman-render').resolve(strict=True)
    patch = repo / 'native/mermaid-worker/patches/merman-render-0.9.3-native-paint.patch'
    patch_sha = digest(patch)
    prepare.verify(source, patch_sha)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    workspace = out / 'workspace'
    test_source = workspace / 'crates/merman-render'
    shutil.copytree(source, test_source)
    manifest = test_source / 'Cargo.toml'
    # Isolate this root package from an enclosing Studio workspace, even when
    # output is inside its target folder. Its development lock retains the
    # worker's exact shared dependency versions, plus test-only dependencies.
    manifest.write_text(manifest.read_text() + '\n[workspace]\n')
    test_lock = repo / 'native/mermaid-worker/renderer-tests.Cargo.lock'
    worker_lock = repo / 'native/mermaid-worker/Cargo.lock'
    test_packages = tomllib.loads(test_lock.read_text())['package']
    worker_packages = tomllib.loads(worker_lock.read_text())['package']
    test_versions = {(p['name'], p['version']) for p in test_packages}
    test_names = {p['name'] for p in test_packages}
    if any((p['name'], p['version']) not in test_versions for p in worker_packages if p['name'] in test_names):
        raise RuntimeError('Renderer tests drifted from shared worker dependency versions')
    shutil.copy2(test_lock, test_source / 'Cargo.lock')
    cache = args.fixture_cache or Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')) / 'studio-merman-test-fixtures' / COMMIT
    cache = cache.resolve()
    fixtures = []
    for relative, expected in FIXTURES.items():
        url = f'https://raw.githubusercontent.com/Latias94/merman/{COMMIT}/fixtures/{relative}'
        cached = cache / relative
        if cached.exists():
            if digest(cached) != expected:
                raise RuntimeError(f'Preserve modified fixture cache and choose another: {cached}')
        else:
            if args.offline:
                raise RuntimeError(f'Missing pinned test fixture: {cached}')
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read(1024 * 1024 + 1)
            if len(data) > 1024 * 1024 or hashlib.sha256(data).hexdigest() != expected:
                raise RuntimeError(f'Fixture failed pinned size/hash validation: {relative}')
            cached.parent.mkdir(parents=True, exist_ok=True)
            cached.write_bytes(data)
        destination = workspace / 'fixtures' / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(cached, destination)
        fixtures.append(dict(path=relative, url=url, sha256=expected))
    # A unique target is deliberate: root-package reuse across copied workspaces
    # can retain an earlier env!(CARGO_MANIFEST_DIR), invalidating fixture tests.
    environment = dict(os.environ, CARGO_TARGET_DIR=str(out / 'target'),
                       CARGO_BUILD_JOBS='2', CARGO_PROFILE_DEV_DEBUG='0')
    command = ['cargo', 'test', '--manifest-path', str(manifest), '--locked', '--lib',
               '--features', 'math,diagram-flowchart,diagram-sequence']
    if args.offline:
        command.append('--offline')
    command += ['--', '--test-threads=2']
    result = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=900)
    log = result.stdout + result.stderr
    (out / 'tests.log').write_text(log)
    prepare.verify(source, patch_sha)
    passed = re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored', log)
    ok = result.returncode == 0 and passed is not None and int(passed[1]) >= 1029 and int(passed[2]) == 0
    record = dict(status='passed' if ok else 'failed', command=command,
                  testsPassed=int(passed[1]) if passed else 0,
                  features=['math', 'diagram-flowchart', 'diagram-sequence'],
                  preparedSourcePreserved=True, patchSha256=patch_sha, fixtures=fixtures,
                  testLockSha256=digest(test_lock), sharedWorkerDependencyVersions=True,
                  conditionalNodeBrowserHelpersExercised=False)
    (out / 'VALIDATED.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))
    if not ok:
        # Keep the full log as an artifact, but expose a bounded diagnostic in
        # CI too: an early build failure otherwise reports only zero tests.
        for stream, output in [('stdout', result.stdout), ('stderr', result.stderr)]:
            print(f'Renderer {stream}:')
            print('\n'.join(output.splitlines()[-60:]))
        raise SystemExit(f'Inspect renderer library tests: {out / "tests.log"}')


if __name__ == '__main__':
    main()
