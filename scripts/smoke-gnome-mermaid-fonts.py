#!/usr/bin/env python3
"""Check strict mixed-script rendering through the real native font/sandbox path."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from xml.etree import ElementTree as ET


LABELS = {
    'mixed-scripts': 'Hello Ελληνικά Русский العربية 中文',
    'ligatures': 'office ffi affluent Ελληνικά العربية 中文',
    'combining': 'a\u0301 o\u0308 Ελληνικά العربية 中文',
    'rtl': 'العربية עברית 中文 office',
    'rtl-marks': 'שָׁלוֹם عَرَبِيّ 中文 office',
    'rtl-first': '中文 العربية Ελληνικά Русский ffi',
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('worker', type=Path)
    parser.add_argument('parent', type=Path)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    worker = args.worker.resolve(strict=True)
    parent = args.parent.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    rows = []
    for name, label in LABELS.items():
        request = dict(version=3, dark=False, style=dict(style='classic', layout='dagre'),
                       source=f'flowchart LR\nA["{label}"] --> B[Read]')
        (out / f'{name}.request.json').write_text(json.dumps(request, indent=2) + '\n')
        row = dict(name=name)
        for role, executable in [('current', worker), ('baseline', baseline)]:
            if executable is None:
                continue
            result = subprocess.run([str(parent), str(executable), '--auto-fonts'],
                                    input=json.dumps(request), text=True,
                                    capture_output=True, timeout=12)
            (out / f'{name}.{role}.stderr').write_text(result.stderr)
            row[role] = dict(ok=result.returncode == 0, exitCode=result.returncode)
            if result.returncode:
                assert not result.stdout, (name, role, 'partial output')
                row[role]['error'] = result.stderr.splitlines()[-1] if result.stderr else ''
                continue
            data = result.stdout.encode()
            assert len(data) <= 4 * 1024 * 1024
            svg = ET.fromstring(data)
            tags = [e.tag.rsplit('}', 1)[-1] for e in svg.iter()]
            assert not any(t in tags for t in ['text', 'foreignObject', 'image'])
            assert tags.count('path') > 0
            (out / f'{name}.{role}.svg').write_bytes(data)
            row[role]['svgSha256'] = hashlib.sha256(data).hexdigest()
        rows.append(row)
    passed = all(r['current']['ok'] for r in rows)
    regression_reproduced = baseline is None or (
        not rows[0]['baseline']['ok']
        and 'closed font set cannot shape this label' in rows[0]['baseline']['error'])
    record = dict(status='passed' if passed and regression_reproduced else 'failed',
                  workerSha256=digest(worker), parentSha256=digest(parent),
                  baselineSha256=digest(baseline) if baseline else None,
                  baselineMixedScriptFailureReproduced=regression_reproduced if baseline else None,
                  cases=rows, applicationLaunchPerformed=False)
    (out / 'VALIDATED.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))
    assert passed and regression_reproduced, f'Inspect retained font fixtures at {out}'


if __name__ == '__main__':
    main()
