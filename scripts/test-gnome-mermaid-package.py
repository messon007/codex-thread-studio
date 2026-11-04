#!/usr/bin/env python3
"""Verify component rejection/copy boundaries using only an owned package copy."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('component', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    verifier = repo / 'scripts/verify-gnome-mermaid.py'
    source = args.component.resolve(strict=True)
    subprocess.run(['python3', str(verifier), str(source)], check=True, capture_output=True)
    # A full app bundle may also contain large gateway/PDF/Java binaries. Copy
    # only the verified worker component for these mutation tests.
    record = json.loads((source / 'MERMAID-WORKER.json').read_text())
    component = out / 'component'
    component.mkdir()
    for relative in [*record['filesSha256'], 'MERMAID-WORKER.json']:
        dest = component / relative
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / relative, dest)
    manifest = component / 'MERMAID-WORKER.json'
    original = json.loads(manifest.read_text())
    command = ['python3', str(verifier), str(component)]
    assert subprocess.run(command, capture_output=True).returncode == 0
    checks = []
    mutations = [
        ('protocol', lambda m: m.update(protocolVersion=m['protocolVersion'] + 1)),
        ('source', lambda m: m['sourceSha256'].update({'src/main.rs': '0' * 64})),
        ('patch', lambda m: m['patchesSha256'].update({next(iter(m['patchesSha256'])): '0' * 64})),
        ('absolute', lambda m: m['filesSha256'].update({'/tmp/outside': '0' * 64})),
        ('traversal', lambda m: m['filesSha256'].update({'../outside': '0' * 64})),
        ('destination', lambda m: m['filesSha256'].update({'unrelated-file': '0' * 64})),
    ]
    for name, mutate in mutations:
        changed = json.loads(json.dumps(original))
        mutate(changed)
        manifest.write_text(json.dumps(changed))
        target = out / name
        result = subprocess.run(command + ['--install-to', str(target)], capture_output=True, text=True)
        (out / f'{name}.stderr').write_text(result.stderr)
        assert result.returncode and not target.exists(), name
        checks.append(name)
    manifest.write_text(json.dumps(original))
    binary = component / 'codex-thread-studio-mermaid'
    size = binary.stat().st_size
    with binary.open('ab') as file:
        file.write(b'changed')
    assert subprocess.run(command, capture_output=True).returncode
    with binary.open('r+b') as file:
        file.truncate(size)
    checks.append('binary tamper')
    mode = binary.stat().st_mode
    binary.chmod(mode & ~0o111)
    assert subprocess.run(command, capture_output=True).returncode
    binary.chmod(mode)
    checks.append('executable permission')
    subprocess.run(command + ['--install-to', str(out / 'bundle')], check=True, capture_output=True)
    subprocess.run(['python3', str(repo / 'scripts/verify-gnome-mermaid.py'), str(out / 'bundle')],
                   check=True, capture_output=True)
    checks.append('verified declared-file copy')
    record = dict(status='passed', checks=checks, workerSha256=original['binarySha256'])
    (out / 'VALIDATED.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))


if __name__ == '__main__':
    main()
