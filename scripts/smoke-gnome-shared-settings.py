#!/usr/bin/env python3
"""Scoped folder-policy API and native Save/Reload/conflict/restart acceptance.

Uses private fixture folders and fake providers. Folder-chooser portals need manual testing.
"""
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
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('shared_fixture', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def api_checks(executable, root):
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    gateway = runtime.fixture.Gateway(Path(executable), root, 'codex')
    endpoint = '/studio/native/shared-directories'
    project, shared = root / 'project', root / 'shared docs 中文 #'
    missing = str(root / 'unmounted 中文 # ')
    def patch(revision, paths, **extra):
        return gateway.api(endpoint, {'expectedRevision': revision, 'directories': paths, **extra}, method='PATCH')
    def roots():
        return gateway.api('/studio/native/files/roots', {'root': str(project)})[1]['roots']
    def location():
        return gateway.api('/studio/native/files/location', {'root': str(project), 'path': str(shared / 'notes.md')})
    try:
        assert gateway.api(endpoint, authorized=False)[0] == 401
        assert gateway.api(endpoint, {}, authorized=False, method='PATCH')[0] == 401
        request = Request(gateway.origin + endpoint, headers={'Authorization': 'Bearer ' + gateway.token, 'Origin': 'https://foreign.example'})
        try:
            gateway.client.open(request)
            raise AssertionError('Foreign origin could read shared folders')
        except HTTPError as error:
            assert error.code == 403
        code, base = gateway.api(endpoint)
        assert code == 200 and base['directories'] == [] and len(roots()) == 1
        assert location()[0] == 403
        # Unrelated trusted changes do not invalidate the scoped revision or disappear on Save.
        raw = gateway.preferences()
        raw.update(futureSharedSettings={'preserve': True}, language='zh-CN')
        gateway.settings.write_text(json.dumps(raw))
        code, saved = patch(base['revision'], [str(shared), missing])
        assert code == 200 and saved['directories'] == [str(shared), missing], saved
        assert len(roots()) == 2 and roots()[1]['readOnly'] and location()[1]['readOnly']
        raw = gateway.preferences()
        assert raw['futureSharedSettings']['preserve'] and raw['language'] == 'zh-CN'
        before = gateway.settings.read_bytes()
        for paths in [['relative'], [''], ['/bad\npath'], ['/x' * 2049], [str(shared)] * 257]:
            assert patch(saved['revision'], paths)[0] == 400
            assert gateway.settings.read_bytes() == before
        assert patch(saved['revision'], [], unknown=True)[0] == 400
        assert patch(base['revision'], [str(root / 'different')])[0] == 409
        assert gateway.settings.read_bytes() == before and location()[0] == 200
        code, reset = patch(saved['revision'], [])
        assert code == 200 and len(roots()) == 1 and location()[0] == 403
        # Reload installs trusted external changes into live policy without rewriting the file.
        raw = gateway.preferences()
        raw['sharedDocumentDirectories'] = [str(shared)]
        gateway.settings.write_text(json.dumps(raw))
        before = gateway.settings.read_bytes()
        code, external = gateway.api(endpoint)
        assert code == 200 and location()[0] == 200 and gateway.settings.read_bytes() == before
        assert patch(external['revision'], [])[0] == 200 and location()[0] == 403
        assert gateway.preferences()['futureSetting']['preserve']
        assert not runtime.starts(root), 'Folder settings triggered a model turn'
    finally:
        gateway.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--api-only', action='store_true')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = [str(Path(p).resolve()) for p in [args.native, args.gateway]]
    artifacts = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    artifacts.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='shared-settings-', dir=artifacts))
    runtime.create_profile(root)
    (root / 'shared docs 中文 #').mkdir()
    (root / 'shared docs 中文 #/notes.md').write_text('# Shared notes\n')
    (root / 'concurrent shared').mkdir()
    api_checks(gateway, root)
    if args.api_only:
        print(json.dumps({'ok': True, 'sharedSettingsApi': True, 'artifacts': str(root)}))
        return
    backend = runtime.CODEX.replace("    elif method=='thread/list':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':", 1)
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
        environment['STUDIO_GNOME_SMOKE_SHARED_SETTINGS'] = phase
        with (root / f'app-{phase}.log').open('w') as log:
            child = subprocess.Popen([native], env=environment, cwd=root, stdout=log, stderr=log)
            try:
                child.wait(timeout=80)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)
                raise AssertionError(f'Shared-folder controls timed out: {root}')
        assert child.returncode == 0, (root, phase, child.returncode)
        result = json.loads((root / f'shared-settings-{phase}.json').read_text())
        assert result['ok'], (root, result)
        results.append(result)
        assert not runtime.starts(root), 'Settings UI triggered a model turn'
        for name in ['backend.pid', 'opencode.pid']:
            if not (root / name).exists():
                continue
            identity = (root / name).read_text().strip()
            for _ in range(100):
                if not Path('/proc', identity).exists():
                    break
                time.sleep(.05)
            assert not Path('/proc', identity).exists(), f'Owned backend {identity} survived settings fixture'
        log = (root / f'app-{phase}.log').read_text()
        assert not any(s in log for s in ['CRITICAL', 'panicked', 'Theme parser error']), (root, log[-2500:])
    settings = json.loads((root / 'config/codex-thread-studio/settings.json').read_text())
    assert settings['futureSetting']['preserve'] and settings['futureSharedSettings']['preserve']
    assert settings['queueDepth'] == 3 and settings['router']['futureRouter'] == 42
    assert settings['sharedDocumentDirectories'] == [str(root / 'shared docs 中文 #'), str(root / 'unmounted 中文 # ')]
    print(json.dumps({'ok': True, 'artifacts': str(root), 'sharedSettingsApi': True,
                      'launches': results, 'unrelatedPreferences': True, 'backendCleanup': True}))


if __name__ == '__main__':
    main()
