#!/usr/bin/env python3
"""Private Wayland page uploads and network downloads through GTK's real portal API."""
import argparse
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
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
spec = importlib.util.spec_from_file_location('browser_file_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
BLOB = b'Native browser download\x00' + '中文😀\r\n'.encode() * 16384
PAGE = '''<!doctype html><html><head><meta charset="utf-8"><title>Browser file portal fixture</title>
<style>body{font:18px sans-serif;margin:24px}input,a{display:block;margin:24px 0;width:280px;padding:16px}h1{font-size:24px}</style></head>
<body><h1>Native browser file access 中文</h1><label>Single explicit file<input id="single" type="file" accept="text/plain"></label>
<label>Multiple explicit files<input id="multiple" type="file" accept="text/plain" multiple></label><a id="download" href="/blob">Download private binary</a>
<script>document.addEventListener('pointerdown',e=>document.body.dataset.pointer=JSON.stringify({x:e.clientX,y:e.clientY,target:e.target.id}),true);for(const input of document.querySelectorAll('input')){input.onclick=()=>document.body.dataset.click=input.id;input.onchange=async()=>{
const payload=await Promise.all([...input.files].map(async f=>({name:f.name,text:await f.text()})));
const response=await fetch('/upload',{method:'POST',body:JSON.stringify(payload)});
if(response.ok)document.body.dataset.uploads=JSON.stringify(payload);
};}</script></body></html>'''.encode()


def plan(root):
    upload = dict(interface='FileChooser', method='OpenFile', title='Choose files for 127.0.0.1', options=dict(multiple=False, directory=False))
    multiple = dict(upload, options=dict(multiple=True, directory=False))
    save = dict(interface='FileChooser', method='SaveFile', title='Save browser download', options=dict(multiple=False, directory=False, current_name='native-download.bin'))
    uri = lambda name: (root / name).as_uri()
    selected = uri('project/upload 中文 #.txt')
    return [dict(upload, status=1), dict(upload, uris=[selected]),
            dict(multiple, uris=[selected, uri('project/second.txt')]),
            dict(upload, uris=['https://example.invalid/unselected.txt']), dict(upload, uris=[]),
            dict(upload, hold=True, lateUris=[selected]), dict(upload, hold=True, lateUris=[selected]),
            dict(upload, uris=[selected]), dict(save, status=1),
            dict(save, uris=[uri('downloaded original 中文.bin')]),
            dict(save, uris=['https://example.invalid/download.bin']),
            dict(save, hold=True, lateUris=[uri('cancelled-download.bin')]),
            dict(save, hold=True, lateUris=[uri('exited-download.bin')]),
            dict(upload, hold=True, lateUris=[selected]), dict(save, uris=[uri('keep-existing.bin')]), dict(save, hold=True, lateUris=[uri('policy-cancelled.bin')])]


def run(native, gateway, root):
    requests, uploads = [], []
    class Pages(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            blob = self.path.startswith('/blob?case=')
            if not blob and self.path not in ['/one', '/two', '/favicon.ico']:
                self.send_error(404); return
            body = BLOB if blob else PAGE
            self.send_response(200)
            self.send_header('Content-Type', 'application/octet-stream' if blob else 'text/html; charset=utf-8')
            if blob:
                self.send_header('Content-Disposition', 'attachment; filename="native-download.bin"')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass  # Explicitly cancelled fixture transfers.
        def do_POST(self):
            length = int(self.headers.get('Content-Length', '0'))
            if self.path != '/upload' or not 0 < length < 32768:
                self.send_error(400); return
            uploads.append(json.loads(self.rfile.read(length)))
            self.send_response(204); self.end_headers()
        def log_message(self, *_):
            pass
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Pages)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    portal = FilePortal(root, plan(root))
    controlled = OwnedMutterInput()
    env = dict(os.environ, STUDIO_GNOME_SMOKE_BROWSER_URL=f'http://127.0.0.1:{server.server_port}/one')
    try:
        with (root/'app.log').open('w') as log:
            child = subprocess.Popen([native], cwd=root, env=env, stdout=log, stderr=log)
            memory = ProcessMemory(child.pid, gateway, include_web_content=True)
            try:
                deadline = time.monotonic() + 150
                while child.poll() is None:
                    portal.iterate(); controlled.respond(root)
                    guard=root/'browser-close-guard.json'
                    if guard.exists() and not (root/'browser-close-release').exists():
                        state=json.loads(guard.read_text());assert state==dict(mapped=True,waiting=True),(root,state)
                        (root/'browser-close-release').write_bytes(b'release')
                    if portal.failure:
                        raise AssertionError((root, portal.failure))
                    if time.monotonic() > deadline:
                        raise TimeoutError(('Native browser file fixture timed out', root))
                    if 'panicked' in (root/'app.log').read_text():
                        raise AssertionError((root, (root/'app.log').read_text()[-2500:]))
                    time.sleep(.015)
                portal.iterate()
            except BaseException:
                child.terminate(); child.wait(timeout=10); raise
            finally:
                measured = memory.close()
        assert child.returncode == 0 and (root/'browser-ui.json').is_file(), (root, child.returncode, (root/'app.log').read_text()[-2000:])
        result = json.loads((root/'browser-ui.json').read_text())
        assert child.returncode == 0 and result['ok'], (root, child.returncode, result, (root/'app.log').read_text()[-2000:])
        assert not portal.failure and not portal.plan and not portal.objects, (root, portal.failure, portal.plan, list(portal.objects))
        assert len(portal.calls) == 16 and len(portal.closed) == 6, (root, portal.calls, portal.closed)
        assert sum(t.get('event') == 'late-response' for t in portal.trace) == 6, (root, portal.trace)
        single = [dict(name='upload 中文 #.txt', text='Explicit upload 中文😀\r\n')]
        assert uploads == [single, single + [dict(name='second.txt', text='Second explicit upload\n')], single], (root, uploads)
        assert (root/'downloaded original 中文.bin').read_bytes() == BLOB, 'Downloaded bytes differ'
        assert (root/'keep-existing.bin').read_bytes() == b'Existing destination must survive', 'Download overwrote existing bytes'
        assert not (root/'policy-cancelled.bin').exists(), 'Policy cancellation allowed a late write'
        assert not (root/'cancelled-download.bin').exists() and not (root/'exited-download.bin').exists(), 'Late chooser response wrote a file'
        assert (root/'browser-close-cleaned').is_file() and not (root/'close-never-published.bin').exists(), 'Window closed before owned stage cleanup'
        assert not list(root.glob('*.wkdownload')) and not list(root.glob('.studio-download-*')), 'Download retained an owned staging file'
        for relative, digest in json.loads((root/'original-hashes.json').read_text()).items():
            assert hashlib.sha256((root/relative).read_bytes()).hexdigest() == digest, 'Upload changed source bytes'
        assert not runtime.starts(root), 'Browser file access started a model turn'
        for name in ['backend.pid', 'opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Owned provider survived'
        for _ in range(100):
            if not memory.web_content_alive(): break
            time.sleep(.05)
        assert not memory.web_content_alive(), 'Browser content survived Exit'
        text = (root/'app.log').read_text()
        assert not any(s in text for s in ['CRITICAL', 'panicked', 'Theme parser error']), (root, text[-2000:])
        print(json.dumps(dict(result, portalCalls=len(portal.calls), portalCancellations=len(portal.closed), exactNetworkUploads=True, exactNetworkDownload=True, downloadedBytes=len(BLOB), noLateWrites=True, memory=measured, requests=requests, artifacts=str(root))))
    finally:
        controlled.close(); portal.close(); server.shutdown(); server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native'); parser.add_argument('gateway'); parser.add_argument('--private-root')
    args = parser.parse_args(); resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = str(Path(args.native).resolve()), str(Path(args.gateway).resolve())
    if args.private_root:
        return run(native, gateway, Path(args.private_root))
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'; base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='browser-files-', dir=base)); runtime.create_profile(root)
    (root/'project/upload 中文 #.txt').write_bytes('Explicit upload 中文😀\r\n'.encode())
    (root/'project/second.txt').write_text('Second explicit upload\n')
    (root/'project/unselected-private.txt').write_text('This file must never be uploaded')
    hashes = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in (root/'project').glob('*.txt')}
    (root/'original-hashes.json').write_text(json.dumps(hashes))
    (root/'keep-existing.bin').write_bytes(b'Existing destination must survive')
    backend = runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name, source in [('fake-codex.py', backend), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root/name; path.write_text(source); path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway, CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', GTK_USE_PORTAL='1', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_BROWSER_FILES='1', STUDIO_OWNED_MUTTER_INPUT='1')
    compositor = [shutil.which('mutter') or 'mutter', '--headless', '--wayland', '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--']
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', *compositor, sys.executable, str(Path(__file__).resolve()), native, gateway, '--private-root', str(root)], cwd=root, env=private, check=True, timeout=180)
if __name__ == '__main__':
    main()
