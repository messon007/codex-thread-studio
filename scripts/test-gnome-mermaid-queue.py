#!/usr/bin/env python3
"""Exercise the real prepared renderer/image cache using private font/ELF copies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
from xml.sax.saxutils import escape


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, env, timeout):
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise
    return process.returncode, stdout, stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('worker', type=Path)
    parser.add_argument('--output', required=True, type=Path,
                        help='New owned directory; no existing output is replaced')
    parser.add_argument('--test-binary', type=Path)
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    (out / 'FIXTURE').write_text('owned-mermaid-queue-fixture\n')
    worker = args.worker.resolve(strict=True)
    original = digest(worker)
    shutil.copy2(worker, out / 'mermaid')
    fonts = out / 'fonts'
    fonts.mkdir()
    font = fonts / 'DejaVuSans.ttf'
    shutil.copy2('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', font)
    font_original = digest(font)
    config = out / 'fonts.conf'
    config.write_text(f'<fontconfig><dir>{escape(str(fonts))}</dir>'
                      f'<cachedir>{escape(str(out / "font-cache"))}</cachedir></fontconfig>')
    binary = args.test_binary
    if binary is None:
        command = ['cargo', 'test', '--manifest-path', str(repo / 'native/gnome/Cargo.toml'),
                   '--locked', '--lib', '--no-run', '--message-format=json']
        if args.offline:
            command.append('--offline')
        code, stdout, stderr = run(command, os.environ.copy(), 600)
        (out / 'build.jsonl').write_text(stdout)
        (out / 'build.stderr').write_text(stderr)
        if code:
            raise SystemExit(f'Test build failed: {out / "build.stderr"}')
        executables = [p['executable'] for line in stdout.splitlines()
                       if (p := json.loads(line)).get('reason') == 'compiler-artifact'
                       and p.get('executable') and p['target']['kind'] == ['lib']
                       and p['profile']['test']]
        if len(executables) != 1:
            raise SystemExit('Could not identify the native library test executable')
        binary = Path(executables[0])
    binary = binary.resolve(strict=True)
    env = os.environ.copy()
    env.pop('FONTCONFIG_PATH', None)
    env.pop('FC_DEBUG', None)
    env.update(FONTCONFIG_FILE=str(config), STUDIO_GNOME_TEST_RESOURCE_ROOT=str(out))
    name = 'mermaid_prepared::tests::real_prepared_resources_invalidate_image_cache_and_preserve_vector_zoom'
    command = [str(binary), '--exact', name, '--ignored', '--nocapture', '--test-threads=1']
    code, stdout, stderr = run(command, env, 90)
    (out / 'tests.log').write_text(stdout + stderr)
    # libtest can prefix a nocapture JSON line with `test <name> ... `.
    reports = []
    for line in stdout.splitlines():
        _, separator, payload = line.partition('{')
        if separator:
            try:
                report = json.loads('{' + payload)
            except json.JSONDecodeError:
                continue
            if isinstance(report, dict) and 'preparations' in report:
                reports.append(report)
    passed = code == 0 and len(reports) == 1 and reports[0]['status'] == 'passed'
    unchanged = digest(worker) == original
    restored = digest(out / 'mermaid') == original and digest(font) == font_original
    record = dict(status='passed' if passed and unchanged and restored else 'failed',
                  workerSha256=original, testBinarySha256=digest(binary),
                  installedResourcesUnmodified=unchanged, fixtureBytesRestored=restored,
                  result=reports[0] if reports else None)
    (out / 'VALIDATED.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record))
    if record['status'] != 'passed':
        raise SystemExit(f'Inspect {out / "tests.log"}')


if __name__ == '__main__':
    main()
