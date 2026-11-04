#!/usr/bin/env python3
"""Scoped appearance API and two real Wayland launches with private fake providers."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
spec = importlib.util.spec_from_file_location('appearance_fixture', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def api_checks(executable, root):
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    gateway = runtime.fixture.Gateway(Path(executable), root, 'codex')
    endpoint = '/studio/native/appearance'
    try:
        assert gateway.api(endpoint, authorized=False)[0] == 401
        assert gateway.api(endpoint, {}, method='PATCH', authorized=False)[0] == 401
        status, base = gateway.api(endpoint)
        assert status == 200 and base['values']['typography'] is None, base
        typography = {'uiFontFamily': '"DejaVu Sans", system-ui', 'uiFontWeight': 400,
                      'codeFontFamily': 'monospace', 'codeFontSize': 15, 'codeFontWeight': 500,
                      'highContrast': False, 'futureTypography': {'keep': True}}

        def patch(revision, values):
            return gateway.api(endpoint, {'expectedRevision': revision, 'values': values}, method='PATCH')

        status, saved = patch(base['revision'], {'typography': typography})
        assert status == 200 and saved['fonts']['ui'] == ['DejaVu Sans', 'sans-serif'], saved
        assert saved['fonts']['workspace'][0] == 'Noto Sans CJK SC', saved
        # Concurrent unrelated edits must survive without forcing a false conflict.
        raw = gateway.preferences()
        raw['futureAppearanceFixture'] = {'keep': True}
        gateway.settings.write_text(json.dumps(raw))
        status, wide = patch(saved['revision'], {'contentWidth': 'wide'})
        assert status == 200 and wide['values']['typography'] == typography, wide
        assert gateway.preferences()['futureAppearanceFixture'] == {'keep': True}
        assert patch(saved['revision'], {'contentWidth': 'full'})[0] == 409
        for values in [{'contentWidth': 'infinite'}, {'theme': 'dark'}, {'typography': {}},
                       {'typography': {**typography, 'uiFontFamily': 'sans-serif; opacity:0'}},
                       {'typography': {**typography, 'codeFontSize': 99}},
                       {'typography': {**typography, 'uiFontWeight': 900}},
                       {'typography': {**typography, 'uiFontFamily': '"bad\nfont"'}},
                       {'typography': {**typography, 'uiFontFamily': ','.join(['serif'] * 17)}}]:
            assert patch(wide['revision'], values)[0] == 400, values
            assert gateway.api(endpoint)[1]['revision'] == wide['revision']
        assert gateway.api(endpoint, {'expectedRevision': wide['revision'], 'values': {}, 'unknown': 1}, method='PATCH')[0] == 400
        status, reset = patch(wide['revision'], {'contentWidth': 'comfortable', 'typography': None})
        assert status == 200 and reset['fonts']['ui'] == [], reset
        raw=gateway.preferences();raw['markdown']={'mode':'technical','futureReading':{'keep':True}};raw['mermaid']={'style':'auto','density':'loose','curve':'rounded','layout':'auto','fontSize':15}
        gateway.settings.write_text(json.dumps(raw))
        status,base=gateway.api(endpoint);assert status==200 and base['values']['markdownMode']=='technical'
        status,reading=patch(base['revision'],{'markdownMode':'reading'});assert status==200 and reading['values']['markdownMode']=='reading'
        assert gateway.preferences()['markdown']['futureReading']=={'keep':True}
        assert gateway.preferences()['mermaid']['density']=='loose'
        assert patch(base['revision'],{'markdownMode':'compact'})[0]==409
        for mode in ['invalid',42,True,{},[]]:
            assert patch(reading['revision'],{'markdownMode':mode})[0]==400
            assert gateway.api(endpoint)[1]['revision']==reading['revision']
        status,defaults=patch(reading['revision'],{'markdownMode':None});assert status==200 and defaults['values']['markdownMode']=='technical'
        assert gateway.preferences()['markdown']['futureReading']=={'keep':True}
        raw = gateway.preferences()
        raw['mermaid'].update(style='handDrawn', layout='elk', futureDiagram={'keep': True})
        gateway.settings.write_text(json.dumps(raw))
        status, base = gateway.api(endpoint)
        assert status == 200
        status, diagram = patch(base['revision'], {
            'mermaidFontSize': 20, 'mermaidDensity': 'compact', 'mermaidCurve': 'linear'})
        assert status == 200
        for key, value in [('style', 'handDrawn'), ('layout', 'elk'), ('futureDiagram', {'keep': True})]:
            assert gateway.preferences()['mermaid'][key] == value
        for values in [{'mermaidFontSize': 21}, {'mermaidFontSize': 15.5},
                       {'mermaidDensity': 'invalid'}, {'mermaidCurve': 'invalid'},
                       {'mermaidStyle': 'invalid'}, {'mermaidLayout': 'invalid'}]:
            assert patch(diagram['revision'], values)[0] == 400
            assert gateway.api(endpoint)[1]['revision'] == diagram['revision']
        assert patch(base['revision'], {'mermaidFontSize': 12})[0] == 409
        status, base = patch(diagram['revision'], {
            'mermaidFontSize': None, 'mermaidDensity': None, 'mermaidCurve': None})
        assert status == 200
        assert gateway.preferences()['futureSetting']['preserve']
        assert not runtime.starts(root), 'Appearance settings triggered a model turn'
    finally:
        gateway.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--api-only', action='store_true')
    parser.add_argument('--headless', action='store_true', help='run in an owned private Wayland compositor')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = [str(Path(value).resolve()) for value in [args.native, args.gateway]]
    if args.headless and not args.api_only and os.environ.get('STUDIO_APPEARANCE_OWNED_DESKTOP')!='1':
        with tempfile.TemporaryDirectory(prefix='studio-appearance-desktop-',dir='/tmp') as directory:
            private_root=Path(directory)
            environment=dict(os.environ,STUDIO_APPEARANCE_OWNED_DESKTOP='1',GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(private_root/'config'),XDG_DATA_HOME=str(private_root/'data'),XDG_CACHE_HOME=str(private_root/'cache'))
            command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
            with isolated_environment(environment,headless=True) as private:
                subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),*sys.argv[1:]],env=private,check=True,timeout=200)
        return
    artifacts = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    artifacts.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='appearance-ui-', dir=artifacts))
    runtime.create_profile(root)
    api_checks(gateway, root)
    if args.api_only:
        print(json.dumps({'ok': True, 'appearanceApi': True, 'artifacts': str(root)}))
        return
    (root/'project/diagram-settings.md').write_text('```mermaid\nflowchart LR\n A[Native 中文] --> B[Second node]\n A --> C[Third node]\n```\n')
    backend = runtime.CODEX.replace("assert params['excludeTurns'] is True", 'pass').replace(
        "    elif method=='turn/start':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name, source in [('fake-codex.py', backend), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root / name
        path.write_text(source)
        path.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
                       CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
                       CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
                       CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
                       XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
                       READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland',
                       STUDIO_GNOME_SMOKE_DIR=str(root))
    for key in ['DISPLAY', 'XAUTHORITY']:
        environment.pop(key, None)
    results = []
    for phase in ['edit', 'restore']:
        environment['STUDIO_GNOME_SMOKE_APPEARANCE'] = phase
        with (root / f'app-{phase}.log').open('w') as log:
            child = subprocess.Popen([native], env=environment, cwd=root, stdout=log, stderr=log)
            try:
                child.wait(timeout=70)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)
                raise AssertionError(f'Native appearance fixture timed out: {root}')
        assert child.returncode == 0, (root, phase, child.returncode)
        result = json.loads((root / f'appearance-{phase}.json').read_text())
        assert result['ok'], (root, result)
        results.append(result)
        assert not runtime.starts(root), 'Native appearance UI triggered a model turn'
        for name in ['backend.pid', 'opencode.pid']:
            if not (root / name).exists():
                continue
            identity = (root / name).read_text().strip()
            for _ in range(100):
                if not Path('/proc', identity).exists():
                    break
                time.sleep(.05)
            assert not Path('/proc', identity).exists(), f'Owned backend {identity} survived appearance fixture'
        log = (root / f'app-{phase}.log').read_text()
        assert not any(message in log for message in ['CRITICAL', 'panicked', 'Theme parser error']), (root, log[-3000:])
    settings = json.loads((root / 'config/codex-thread-studio/settings.json').read_text())
    assert settings['futureSetting']['preserve'] and settings['futureAppearanceFixture']['keep']
    assert settings['queueDepth'] == 3 and settings['router']['futureRouter'] == 42
    assert settings['mermaid']['style'] == 'handDrawn'
    assert settings['mermaid']['layout'] == 'elk'
    assert settings['mermaid']['futureDiagram'] == {'keep': True}
    print(json.dumps({'ok': True, 'artifacts': str(root), 'appearanceApi': True,
                      'launches': results, 'unrelatedPreferences': True, 'backendCleanup': True}))


if __name__ == '__main__':
    main()
