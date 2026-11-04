#!/usr/bin/env python3
"""Explicit public HTTPS image acceptance with private providers and Wayland UI.

The fixture contains ORIGIN.json, expected.png/svg and the upstream artwork
license. Its public immutable URLs are fetched by the production native client;
no model prompts, browser engine, credentials or local content are transmitted.
"""
import argparse
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment, OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('remote_outputs', Path(__file__).with_name('smoke-gnome-outputs.py'))
outputs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(outputs)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def child(native, gateway, root):
    calls = []
    class Canary(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            calls.append(self.path)
            self.send_response(404)
            self.end_headers()
        def log_message(self, *args):
            pass
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Canary)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    origin = json.loads((root/'ORIGIN.json').read_text())
    svg_url = origin['files']['expected.svg']['url']
    document = root/'project/remote.md'
    document.write_text('# Remote images\n\n![Remote reader SVG]('+svg_url+')\n\n![Blocked private image](http://127.0.0.1:'+str(server.server_port)+'/blocked)\n')
    before = sha(document)
    plans = [dict(interface='FileChooser', method='SaveFile', title='Save original image', options=dict(multiple=False, directory=False, current_name='image.'+ext), uris=[(root/('saved-remote.'+ext)).as_uri()]) for ext in ['png','svg']]
    controlled = OwnedMutterInput()
    portal = FilePortal(root, plans)
    try:
        with (root/'app.log').open('w') as log:
            app = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
            observer = ProcessMemory(app.pid, gateway, include_web_content=True)
            try:
                deadline = time.monotonic()+90
                while app.poll() is None:
                    portal.iterate()
                    controlled.respond(root)
                    if portal.failure:
                        raise AssertionError((root, portal.failure))
                    if time.monotonic() > deadline:
                        raise TimeoutError(('Remote image fixture timed out', root))
                    time.sleep(.02)
                portal.iterate()
            except BaseException:
                app.terminate()
                app.wait(timeout=10)
                raise
            finally:
                memory = observer.close()
    finally:
        portal.close()
        controlled.close()
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)
    result = json.loads((root/'remote-images.json').read_text())
    assert app.returncode == 0 and result['ok'], (root, result, (root/'app.log').read_text()[-2000:])
    assert not portal.failure and not portal.plan and len(portal.calls)==2
    assert not calls, ('Private network image was fetched', calls)
    assert sha(document)==before
    for ext in ['png','svg']:
        assert (root/('saved-remote.'+ext)).read_bytes() == (root/('expected.'+ext)).read_bytes()
    assert not outputs.runtime.starts(root), 'Remote image fixture sent a model prompt'
    assert memory['webContentProcessesObserved']==0, 'Image path started a web engine'
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Gtk-WARNING'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():
            assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Provider survived'
    print(json.dumps(dict(result, productionDnsAndTls=True, privateNetworkUnfetched=True, originalBytesExact=True, sourceUnchanged=True, noModelPrompts=True, noWebEngine=True, memory=memory, artifacts=str(root))))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native', type=Path)
    parser.add_argument('gateway', type=Path)
    parser.add_argument('--fixture', required=True, type=Path)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    native,gateway = [str(p.resolve(strict=True)) for p in [args.native,args.gateway]]
    if args.private_root:
        return child(native,gateway,args.private_root)
    fixture = args.fixture.resolve(strict=True)
    origin = json.loads((fixture/'ORIGIN.json').read_text())
    for name in ['expected.png','expected.svg','ARTWORK-LICENSE.md']:
        assert sha(fixture/name)==origin['files'][name]['sha256']
        assert origin['files'][name]['url'].startswith('https://raw.githubusercontent.com/rust-lang/rust-artwork/'+origin['commit']+'/')
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='remote-images-', dir=base))
    outputs.runtime.create_profile(root)
    for name in ['expected.png','expected.svg','ARTWORK-LICENSE.md','ORIGIN.json']:
        shutil.copy2(fixture/name,root/name)
    png_url = origin['files']['expected.png']['url']
    items = [{'id':'remote-user','type':'userMessage','content':[{'type':'text','text':'Native HTTPS image acceptance'},{'type':'image','url':png_url}]}, {'id':'remote-assistant','type':'agentMessage','text':'![Remote chat PNG]('+png_url+')'}]
    (root/'raw-items.json').write_text(json.dumps(items))
    (root/'expected-user.txt').write_text('Native HTTPS image acceptance')
    (root/'expected-assistant.txt').write_text('![Remote chat PNG]('+png_url+')')
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    for name,source in zip(['fake-codex.py','fake-opencode.py'], outputs.providers()):
        path=root/name
        path.write_text(source)
        path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_REMOTE_IMAGES='1',STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--fixture',str(fixture),'--private-root',str(root)],env=private,cwd=root,check=True,timeout=110)

if __name__=='__main__':
    main()
