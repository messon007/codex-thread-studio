#!/usr/bin/env python3
"""Measure real GTK split-pane allocations with isolated fake providers on Wayland."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('native_fixture', Path(__file__).with_name('smoke-gnome-native.py'))
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--probe', action='store_true', help='record existing failures for diagnosis; not an acceptance pass')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    artifacts = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke')
    artifacts.mkdir(parents=True,exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='layout-ui-', dir=artifacts))
    (root / 'project/nested').mkdir(parents=True)
    (root / 'project/nested/note.md').write_text('# Linked document\n')
    (root / 'project/report.md').write_text('# Layout report\n\nReadable content stays within its pane.\n\n| Name | Value |\n|---|---|\n| Alpha | One |\n')
    profile = root / 'config/codex-thread-studio'
    profile.mkdir(parents=True)
    (profile / 'settings.json').write_text(json.dumps({'queueDepth': 3, 'contentWidth': 'comfortable'}))
    for name, source in [('fake-codex.py', fixture.FAKE_CODEX), ('fake-opencode.py', Path(__file__).with_name('test-support').joinpath('gnome-opencode.py').read_text())]:
        path = root / name
        path.write_text(source)
        path.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=str(Path(args.gateway).resolve()),
                       CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
                       CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
                       CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
                       XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
                       READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland',
                       STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_LAYOUT='1')
    for key in ['DISPLAY', 'XAUTHORITY', 'STUDIO_GNOME_LAYOUT_PROBE']:
        environment.pop(key, None)
    if args.probe:
        environment['STUDIO_GNOME_LAYOUT_PROBE'] = '1'
    with (root / 'app.log').open('w') as log:
        child = subprocess.Popen([str(Path(args.native).resolve())], env=environment, cwd=root, stdout=log, stderr=log)
        try:
            child.wait(timeout=55)
        except subprocess.TimeoutExpired:
            child.terminate()
            child.wait(timeout=10)
            raise AssertionError(f'Native layout fixture timed out: {root}')
    assert child.returncode == 0, (root, child.returncode)
    result = json.loads((root / 'layout.json').read_text())
    assert result['ok'], (root, result.get('error'), result.get('failures'))
    if not args.probe:
        assert len(result['samples']) == 20, (root, 'Missing layout cases')
        for sample in result['samples']:
            effective=max(sample['minimumPosition'],min(sample['requested'],sample['maximumPosition']))
            assert abs(sample['effective']-effective)<=1, (root,sample['phase'],'GTK did not enforce the measured pane limits')
            assert sample['conversation']['width']==sample['effective'], (root,sample['phase'],'Divider allocation differs from the visible pane')
            assert sample['composer']['width'] > 0 and sample['composer']['height'] > 0
            for control in sample['controls'].values():
                if control['width'] > 0 and control['height'] > 0:
                    bounds = control['bounds']
                    assert bounds['y'] >= 0 and bounds['y'] + bounds['height'] <= sample['conversation']['height'] + 1, (root, sample['phase'], 'Control outside visible pane height')
        assert result['modelPopup']['wrapped'] and result['modelPopup']['selectionMarker']
    for name in ['backend.pid', 'opencode.pid']:
        if not (root / name).exists():
            continue
        identity = (root / name).read_text().strip()
        for _ in range(100):
            if not Path('/proc', identity).exists():
                break
            time.sleep(.05)
        assert not Path('/proc', identity).exists(), f'Owned backend {identity} survived layout fixture'
    calls = [json.loads(line) for line in (root / 'requests.jsonl').read_text().splitlines()]
    assert all(call.get('method') != 'turn/start' for call in calls), 'Layout fixture sent a prompt'
    log = (root / 'app.log').read_text()
    assert not any(message in log for message in ['CRITICAL', 'panicked', 'Theme parser error']), (root, log[-3000:])
    print(json.dumps({'ok': result['ok'], 'acceptance': not args.probe, 'cases': len(result['samples']), 'modelPopup': result['modelPopup'], 'failures': result['failures'], 'artifacts': str(root), 'backendCleanup': True}))


if __name__ == '__main__':
    main()
