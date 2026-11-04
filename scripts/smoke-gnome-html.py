#!/usr/bin/env python3
"""Passive native HTML, original-file editing and owned Wayland input."""
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
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import urlopen

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment, OwnedMutterInput
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('html_profile', REPO/'scripts/smoke-gnome-source-comments.py')
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def run(native, gateway, root):
    owned = OwnedMutterInput()
    owned.enable_clipboard_reader()
    with (root/'app.log').open('w') as log:
        app = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
        memory = ProcessMemory(app.pid, gateway, include_web_content=True)
        try:
            deadline = time.monotonic()+160
            while app.poll() is None:
                owned.respond(root)
                if time.monotonic() > deadline:
                    raise TimeoutError(('HTML acceptance timed out', root))
                time.sleep(.01)
        finally:
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=10)
            measured = memory.close()
            owned.close()
    result = json.loads((root/'html-result.json').read_text())
    assert app.returncode == 0 and result['ok'], (root, result, (root/'app.log').read_text()[-2000:])
    assert not source.runtime.starts(root), 'HTML acceptance sent a provider prompt'
    assert (root/'project/passive.html').read_bytes() == '<h1>Inserted before outline</h1>\r\n<h1>Saved addition 中文😀</h1>\r\n'.encode()+(root/'original.bin').read_bytes()
    assert (root/'readonly/shared.html').read_text()=='<h1>Read-only HTML</h1><p>Shared readable content 中文😀</p>'
    assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
    for name in ['backend.pid', 'opencode.pid']:
        if (root/name).exists():
            source.runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(), seconds=10)
    deadline=time.monotonic()+10
    while memory.web_content_alive() and time.monotonic()<deadline:
        time.sleep(.1)
    assert not memory.web_content_alive(), 'Owned HTML content survived native close'
    result.update(memory=measured, noProviderPrompts=True, exactFileBytes=True, ownedCleanup=True, artifacts=str(root))
    (root/'measurement.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root',type=Path)
    args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=[str(Path(p).resolve()) for p in [args.native,args.gateway]]
    if args.private_root:
        return run(native,gateway,args.private_root)
    base=REPO/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='html-',dir=base))
    source.runtime.create_profile(root)
    requests=[]
    class Canary(BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            self.send_response(200);self.end_headers();self.wfile.write(b'owned canary')
        def log_message(self,*args):pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Canary)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    url=f'http://127.0.0.1:{server.server_port}'
    (root/'canary-url.txt').write_text(url+'/independent-csp-probe')
    with urlopen(url+'/positive-control',timeout=3) as response:assert response.read()==b'owned canary'
    assert requests==['/positive-control'];requests.clear()
    (root/'private-secret.txt').write_text('private HTML file canary')
    html=f'''<!doctype html><html><head><title>Fixture</title></head><body>
<h1>Original title</h1><p>Keyword one.</p><pre>Literal selection 中文😀  </pre>
<script>globalThis.__studio_attack=1;fetch('{url}/script');window.open('{url}/popup')</script>
<img src="{url}/image" onerror="globalThis.__studio_attack=2"><img src="{(root/'private-secret.txt').as_uri()}">
<style>@import url('{url}/style');</style><link rel=stylesheet href="{url}/link">
<iframe src="{url}/frame">Attack secret</iframe><form action="{url}/form">Attack secret</form>
<p style="background:url('{url}/attribute')" onclick="fetch('{url}/event')">Safe paragraph</p>
{''.join(f'<p>Before context {i}</p>' for i in range(80))}
<h2>Second chapter</h2><p>Keyword two.</p><table><tr><th>Name</th><td>Table value 中文😀</td></tr></table>
{''.join(f'<p>After context {i}</p>' for i in range(80))}
</body></html>
'''.replace('\n','\r\n').encode()
    (root/'project/passive.html').write_bytes(html);(root/'original.bin').write_bytes(html)
    (root/'project/oversize.html').write_bytes(b'x'*(2*1024*1024+1))
    (root/'project/large-heading.html').write_text('<h1>'+'中文😀'*8192+'</h1>')
    (root/'project/other.rs').write_text('// other file\n')
    (root/'readonly').mkdir()
    (root/'readonly/shared.html').write_text('<h1>Read-only HTML</h1><p>Shared readable content 中文😀</p>')
    settings=root/'config/codex-thread-studio/settings.json'
    raw=json.loads(settings.read_text());raw['sharedDocumentDirectories']=[str(root/'readonly')];settings.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    source.write_providers(root)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
        CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
        CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),
        XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',
        GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),
        STUDIO_GNOME_SMOKE_HTML='1',STUDIO_OWNED_MUTTER_INPUT='1')
    try:
        with isolated_environment(env,headless=True) as private:
            subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11',
                '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,
                str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=190)
        assert requests==[],('HTML fetched passive resources',requests)
        (root/'canary.json').write_text(json.dumps(dict(positiveControl=True,requests=requests,noResourceRequests=True))+'\n')
    finally:
        server.shutdown();server.server_close();thread.join(timeout=3)
        (root/'canary-requests.json').write_text(json.dumps(requests)+'\n')


if __name__=='__main__':main()
