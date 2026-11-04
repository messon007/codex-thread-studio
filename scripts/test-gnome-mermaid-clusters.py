#!/usr/bin/env python3
"""Test the pinned usvg patch without changing the prepared source fingerprint."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path,
                        help='New private test directory; existing results are preserved')
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    worker = project / 'native/mermaid-worker'
    helper = project / 'scripts/prepare-gnome-mermaid.py'
    spec = importlib.util.spec_from_file_location('prepare_mermaid', helper)
    prepare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(prepare)
    source = (worker / 'vendor/usvg').resolve(strict=True)
    patch = worker / 'patches/usvg-0.47.0-font-clusters.patch'
    patch_sha = prepare.digest(patch)
    prepare.verify(source, patch_sha, prepare.COMPONENTS['usvg'][1])
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    test_source = out / 'usvg'
    shutil.copytree(source, test_source)
    # Dependency dev-tests are not runnable from another Cargo workspace. A copy
    # can resolve its dev-dependencies without altering the admitted source cache.
    shutil.copy2(worker / 'Cargo.lock', test_source / 'Cargo.lock')
    command = ['cargo', 'test', '--release', '--offline', '--manifest-path',
               str(test_source / 'Cargo.toml'), '--lib', 'fallback_cluster_tests']
    result = subprocess.run(command, capture_output=True, text=True, timeout=300)
    log = result.stdout + result.stderr
    (out / 'tests.log').write_text(log)
    prepare.verify(source, patch_sha, prepare.COMPONENTS['usvg'][1])
    count = re.search(r'test result: ok\. (\d+) passed; 0 failed;', log)
    passed = result.returncode == 0 and count is not None and int(count[1]) >= 6
    record = dict(status='passed' if passed else 'failed',
                  patchSha256=patch_sha, command=command,
                  testsPassed=int(count[1]) if count else 0,
                  layoutSha256=hashlib.sha256((source / 'src/text/layout.rs').read_bytes()).hexdigest(),
                  preparedSourcePreserved=True)
    (out / 'VALIDATED.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))
    if not passed:
        raise SystemExit(f'Inspect test log: {out / "tests.log"}')


if __name__ == '__main__':
    main()
