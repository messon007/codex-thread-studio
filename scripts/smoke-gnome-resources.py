#!/usr/bin/env python3
"""Native session resources, source anchors and shared favorites on owned Wayland."""
import argparse
import hashlib
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
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment, OwnedMutterInput


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = (str(Path(value).resolve()) for value in [args.native, args.gateway])
    runtime = module('navigation_runtime', 'smoke-gnome-router-runtime.py')
    if args.private_root:
        root = Path(args.private_root)
        originals = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in [*((root/'project').rglob('*')), *((root/'shared').rglob('*'))] if p.is_file()}
        owned=OwnedMutterInput()
        owned.enable_clipboard_reader()
        with (root/'native.log').open('w') as log:
            app = subprocess.Popen([native], cwd=root, stdout=log, stderr=log)
            memory = ProcessMemory(app.pid, gateway, include_web_content=True)
            try:
                deadline=time.monotonic()+150
                while app.poll() is None:
                    owned.respond(root)
                    if time.monotonic()>deadline:raise TimeoutError(('Navigation timed out',root))
                    time.sleep(.01)
            finally:
                if app.poll() is None:
                    app.terminate()
                    app.wait(timeout=10)
                measured = memory.close()
                owned.close()
        result = json.loads((root/'resources-result.json').read_text())
        assert app.returncode == 0 and result['ok'], (root, result)
        assert not any(word in (root/'native.log').read_text() for word in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
        assert not runtime.starts(root), 'Viewer tests started a model turn'
        assert originals == {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                             for p in [*((root/'project').rglob('*')), *((root/'shared').rglob('*'))] if p.is_file()}, 'Viewer tests changed source files'
        for name in ['backend.pid', 'opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Owned provider survived'
        deadline = time.monotonic()+10
        while memory.web_content_alive() and time.monotonic() < deadline:
            time.sleep(.1)
        assert not memory.web_content_alive(), 'Owned EPUB process survived'
        print(json.dumps(dict(ok=True, checks=result, memory=measured, sourceFilesUnchanged=True, ownedCleanup=True, artifacts=str(root))))
        return
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='session-resources-', dir=base))
    runtime.create_profile(root)
    project = root/'project'
    (project/'README.md').write_text('# Resource document\n\nNative resource context 中文😀.\n')
    (project/'source.rs').write_text('fn first() {}\n\nfn resource() {}\n')
    (project/'docs').mkdir()
    (project/'docs/nested.md').write_text('# Nested folder resource\n')
    (project/'docs/中文.md').write_text('# Encoded resource 中文\n')
    (root/'shared').mkdir()
    (root/'shared/shared-resource.md').write_text('# Shared resource\n')
    (project/'sample.png').write_bytes(module('resource_image', 'smoke-gnome-images.py').png(320, 240))
    (root/'codex-turns.json').write_text(json.dumps({'target':[
        {'id':'resource-old','status':'completed','items':[{'id':'resource-old-answer','type':'agentMessage','text':'[Old](old-only.md)'}]},
        {'id':'resource-turn','status':'completed','items':[
            {'id':'resource-question','type':'userMessage','content':[{'type':'text','text':'Review `README.md`'}]},
            {'id':'resource-answer','type':'agentMessage','text':'[Read](README.md)\n\n![Image](sample.png)\n\n[Code](source.rs:3)\n\n[Folder](docs/)\n\nhttps://example.invalid/docs\n\n[Blocked](/outside/secret.md)\n\n[Encoded](docs/%E4%B8%AD%E6%96%87.md)\n\n[Shared]('+str(root/'shared/shared-resource.md')+')'}]}
    ]}))
    code = runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':list(reversed([{'turnId':t['id'],'item':i} for t in turns.get(session,[]) for i in t['items']])),'nextCursor':None})\n    elif method=='thread/read':")
    for name, body in [('fake-codex.py', code), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root/name
        path.write_text(body)
        path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    preferences = root/'config/codex-thread-studio/settings.json'
    raw = json.loads(preferences.read_text())
    raw['language'] = 'en'
    preferences.write_text(json.dumps(raw))
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
               CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_HOME=str(root/'codex'),
               XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'),
               READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', GIO_USE_VFS='local',
               GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root),
               STUDIO_GNOME_SMOKE_RESOURCES='1')
    env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless', '--wayland',
                        '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--',
                        sys.executable, str(Path(__file__).resolve()), native, gateway, '--private-root', str(root)],
                       cwd=root, env=private, check=True, timeout=170)


if __name__ == '__main__':
    main()
