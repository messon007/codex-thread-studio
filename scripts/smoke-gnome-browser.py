#!/usr/bin/env python3
"""Exercise native browser controls on Wayland with private pages and fake providers."""
import argparse
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import threading
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('browser_fixture', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    args = parser.parse_args()
    native, gateway = str(Path(args.native).resolve()), str(Path(args.gateway).resolve())
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    base = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='browser-ui-', dir=base))
    requests = []

    class Pages(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            if self.path not in ('/one', '/two', '/favicon.ico'):
                self.send_error(404)
                return
            body = ('''<!doctype html><html><head><meta charset="utf-8"><title>Private native browser %s</title><style>body{font:18px sans-serif;margin:32px;color:#1f332f}h1{color:#087f76}p{max-width:65ch;line-height:1.7}table{border-collapse:collapse}td,th{padding:12px;border:1px solid #b8d0cc;text-align:left}</style></head><body><h1>Native browser 你好</h1><p id="quote">Selected native browser text.</p><p>needle one. needle two.</p><p>Remote page content stays inside an isolated content engine. Native GTK controls provide tabs, navigation, page search, zoom and comments.</p><table><tr><th>Control</th><th>Owner</th></tr><tr><td>Tabs and address</td><td>GTK4 / libadwaita</td></tr><tr><td>Page layout</td><td>Isolated WebKitGTK 6</td></tr></table><p><a href="/one">First page</a> · <a href="/two">Second page</a></p><script>document.documentElement.dataset.scriptRan='yes';</script></body></html>''' % self.path).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Pages)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    runtime.create_profile(root)
    backend = runtime.CODEX.replace("assert params['excludeTurns'] is True", 'pass').replace("    elif method=='turn/start':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name, source in [('fake-codex.py', backend), ('fake-opencode.py', runtime.OPENCODE)]:
        file = root / name
        file.write_text(source)
        file.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway, CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_BROWSER_URL=f'http://127.0.0.1:{server.server_port}/one')
    environment.pop('DISPLAY', None)
    environment.pop('XAUTHORITY', None)
    try:
        with (root/'app.log').open('w') as log:
            child = subprocess.Popen([native], cwd=root, env=environment, stdout=log, stderr=log)
            observer = ProcessMemory(child.pid, gateway, include_web_content=True)
            try:
                child.wait(timeout=100)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)
                raise AssertionError(f'Browser acceptance timed out: {root}')
            finally:
                memory = observer.close()
        assert child.returncode == 0, (root, child.returncode)
        for _ in range(100):
            if not observer.web_content_alive():
                break
            time.sleep(.05)
        assert not observer.web_content_alive(), ('Browser content survived application exit', root)
        for name in ('backend.pid', 'opencode.pid'):
            file = root/name
            if file.exists():
                identity = file.read_text().strip()
                for _ in range(100):
                    if not Path('/proc', identity).exists():
                        break
                    time.sleep(.05)
                assert not Path('/proc', identity).exists(), ('Owned backend survived application exit', identity)
        result = json.loads((root/'browser-ui.json').read_text())
        assert not runtime.starts(root), 'Browser controls started a model turn'
        print(json.dumps(dict(result, artifacts=str(root), observedMemory=memory, requests=requests)))
        assert result['ok'], (root, result)
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
