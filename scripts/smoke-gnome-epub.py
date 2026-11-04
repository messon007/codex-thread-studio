#!/usr/bin/env python3
"""Bounded native EPUB API and real Wayland controls; private books/providers only."""
import argparse
import zlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import zipfile
from urllib.error import HTTPError
from urllib.request import Request

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment, OwnedMutterInput
spec = importlib.util.spec_from_file_location('epub_fixture', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def book(path, canary, version=3, extra=None):
    chapters = ['Text/%E7%AB%A0%E8%8A%82%20one.xhtml', 'Text/two.xhtml', 'Text/three.xhtml']
    items = ''.join(f'<item id="{name}" href="{href}" media-type="application/xhtml+xml"/>' for name, href in zip(['one','two','three'],chapters))
    nav = '<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>' if version == 3 else '<item id="nav" href="toc.ncx" media-type="application/x-dtbncx+xml"/>'
    opf = f'''<package xmlns="http://www.idpf.org/2007/opf" version="{version}.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Native EPUB · 你好</dc:title><dc:creator>Private fixture</dc:creator><dc:language>en</dc:language></metadata><manifest>{items}{nav}<item id="css" href="Styles/book.css" media-type="text/css"/><item id="pixel" href="Images/pixel.png" media-type="image/png"/><item id="script" href="bad.js" media-type="application/javascript"/>{'<item id="extra" href="large.png" media-type="image/png"/>' if extra else ''}</manifest><spine toc="nav"><itemref idref="one"/><itemref idref="two"/><itemref idref="three"/></spine></package>'''
    navigation = ''.join(f'<li><a href="{href}">Chapter {i+1}</a></li>' for i, href in enumerate(chapters))
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('mimetype', 'application/epub+zip', compress_type=zipfile.ZIP_STORED)
        archive.writestr('META-INF/container.xml', '<container><rootfiles><rootfile full-path="OPS/package.opf"/></rootfiles></container>')
        archive.writestr('OPS/package.opf', opf)
        archive.writestr('OPS/nav.xhtml', f'<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol>{navigation}</ol></nav></body></html>')
        points = ''.join(f'<navPoint id="p{i}"><navLabel><text>Chapter {i+1}</text></navLabel><content src="{href}"/></navPoint>' for i, href in enumerate(chapters))
        archive.writestr('OPS/toc.ncx', f'<ncx><navMap>{points}</navMap></ncx>')
        archive.writestr('OPS/Styles/book.css', '.book-color { color: rgb(14,100,76); } table {border-collapse:collapse;width:100%} td,th {border:1px solid #ccd8d2;padding:12px;text-align:left} img {width:48px;height:48px}')
        for i, name in enumerate(['章节 one','two','three']):
            paragraphs = ''.join(f'<p id="p{n}">Paragraph {n}. Unicode 你好. Read-only EPUB content preserves its source anchors, styling and chapter navigation.</p>' for n in range(1, 101 if i == 1 else 8))
            html = f'''<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Chapter {i+1}</title><link rel="stylesheet" href="../Styles/book.css"/></head><body><h1>Chapter {i+1} · 你好</h1><p id="copy">Needle and native text selection.</p><p id="studio-native-reader-style">Source ID remains untouched.</p><p class="book-color">Book stylesheet</p><table><tr><th>Column</th><th>Value</th></tr><tr><td>Unicode</td><td>中文 and العربية</td></tr></table><img src="../Images/pixel.png"/><script>document.body.setAttribute('data-script-ran','yes');fetch('{canary}/script')</script><img src="{canary}/image" onerror="document.body.setAttribute('data-event-ran','yes')"/><p style="background-image:url('{canary}/css')">Remote canary</p>{paragraphs}</body></html>'''
            archive.writestr(f'OPS/Text/{name}.xhtml', html)
        def chunk(kind, data):
            return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
        png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0\x0e\x64\x4c\xff'))+chunk(b'IEND',b'')
        archive.writestr('OPS/Images/pixel.png',png)
        archive.writestr('OPS/bad.js', 'throw new Error("must not execute")')
        if extra:
            archive.writestr('OPS/large.png', extra)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native', help='native executable, or gateway for --api-only')
    parser.add_argument('gateway', nargs='?')
    parser.add_argument('--api-only', action='store_true')
    parser.add_argument('--headless', action='store_true', help='use an owned Wayland compositor and valid keyboard input')
    parser.add_argument('--reference', action='append', default=[], help='read a private copy of a real EPUB (repeatable); tests content/navigation separately from the full control fixture')
    args = parser.parse_args()
    if not args.gateway and not args.api_only:
        parser.error('both executables are required for Wayland checks')
    native = str(Path(args.native).resolve())
    gateway = str(Path(args.gateway or args.native).resolve())
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if args.headless and os.environ.get('STUDIO_OWNED_MUTTER_INPUT') != '1':
        with tempfile.TemporaryDirectory(prefix='studio-epub-desktop-',dir='/tmp') as directory:
            private_root=Path(directory)
            environment=dict(os.environ,STUDIO_OWNED_MUTTER_INPUT='1',GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(private_root/'config'),XDG_DATA_HOME=str(private_root/'data'),XDG_CACHE_HOME=str(private_root/'cache'))
            command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
            with isolated_environment(environment,headless=True) as private:
                subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),*sys.argv[1:]],env=private,check=True,timeout=240)
        return
    base = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='epub-ui-', dir=base))
    requests = []

    class Canary(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            self.send_response(200)
            self.end_headers()
        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Canary)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    canary = f'http://127.0.0.1:{server.server_port}'
    runtime.create_profile(root)
    project = root / 'project'
    book(project / 'sample.epub', canary)
    book(project / 'old.epub', canary, version=2)
    book(project / 'large.epub', canary, extra=b'x'*(17*1024*1024))
    with zipfile.ZipFile(project / 'unsafe.epub', 'w') as archive:
        archive.writestr('../outside', 'bad')
    (project / 'escape.epub').symlink_to('/etc/passwd')
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    service = runtime.fixture.Gateway(Path(gateway), root, 'codex')

    def post(operation, authorized=True, **fields):
        if operation == 'open':
            fields.setdefault('id', str(uuid.uuid4()))
        headers = {'Content-Type': 'application/json'}
        if authorized:
            headers['Authorization'] = 'Bearer ' + service.token
        request = Request(service.origin + '/studio/native/epub', data=json.dumps({'operation': operation, **fields}).encode(), headers=headers)
        try:
            with service.client.open(request, timeout=35) as response:
                return response.status, response.read()
        except HTTPError as error:
            return error.code, error.read()

    def opened(path='sample.epub'):
        status, value = post('open', root=str(project), path=path)
        assert status == 200, (status, value[:500])
        return json.loads(value)

    def reading(metadata, cfi):
        return dict(root=str(project), path=metadata['path'], bookHash=metadata['hash'], cfi=cfi, chapterLabel='Chapter 1', progress=0.0, fontScale=1.0, theme='light', flow='scrolled', tocOpen=False)

    def asset(data):
        assert data[:8] == b'STEPUB01', data[:300]
        size = struct.unpack('<I', data[8:12])[0]
        return data[12:12+size].decode(), data[12+size:]

    try:
        assert post('open', False, root=str(project), path='sample.epub')[0] == 401
        meta = opened()
        identity = meta['documentId']
        assert len(meta['spine']) == 3 and len(meta['toc']) == 3 and meta['title'].endswith('你好'), meta
        assert post('open', id=identity, root=str(project), path='sample.epub')[0] == 409
        status, data = post('asset', id=identity, path=meta['spine'][0]['href'])
        assert status == 200
        mime, source = asset(data)
        assert mime == 'application/xhtml+xml' and '你好'.encode() in source
        assert post('asset', id=identity, path='META-INF/container.xml')[0] == 400
        assert post('asset', id=identity, path='../../etc/passwd')[0] == 400
        assert post('asset', id=identity, path='OPS/bad.js')[0] == 415
        late = str(uuid.uuid4())
        assert post('close', id=late)[0] == 200
        assert post('open', id=late, root=str(project), path='sample.epub')[0] == 410
        cfi = 'epubcfi(/6/2[one]!/4/2/1:0)'
        current = reading(meta, cfi)
        assert post('save', id=identity, update={'revision':5, 'reading':current})[0] == 200
        stale = reading(meta, 'epubcfi(/6/4[two]!/4/2/1:0)')
        assert json.loads(post('save', id=identity, update={'revision':3, 'reading':stale})[1])['stale']
        wrong = dict(current, bookHash='wrong')
        assert post('save', id=identity, update={'revision':6, 'reading':wrong})[0] == 400
        status, value = post('save', id=identity, update={'revision':6, 'reading':dict(current, chapterLabel='你好'*600)})
        assert status == 200 and len(json.loads(value)['chapterLabel'].encode()) <= 1024
        assert post('close', id=identity, update={'revision':7, 'reading':current})[0] == 200
        assert post('asset', id=identity, path=meta['spine'][0]['href'])[0] == 410
        restored = opened()
        assert restored['saved']['cfi'] == cfi
        post('close', id=restored['documentId'])
        old = opened('old.epub')
        assert len(old['toc']) == 3 and old['toc'][1]['label'] == 'Chapter 2'
        post('close', id=old['documentId'])
        large = opened('large.epub')
        assert post('asset', id=large['documentId'], path='OPS/large.png')[0] == 413
        post('close', id=large['documentId'])
        assert post('open', root=str(project), path='unsafe.epub')[0] == 400
        assert post('open', root=str(project), path='escape.epub')[0] == 403
        assert not runtime.starts(root), 'EPUB API started a model'
    finally:
        service.close()
    if args.api_only:
        server.shutdown()
        server.server_close()
        print(json.dumps({'ok':True, 'artifacts':str(root), 'epubApi':True}))
        return

    backend = runtime.CODEX.replace("assert params['excludeTurns'] is True", 'pass').replace("    elif method=='turn/start':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name, source in [('fake-codex.py', backend), ('fake-opencode.py', runtime.OPENCODE)]:
        file = root / name
        file.write_text(source)
        file.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway, CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_EPUB='1')
    environment.pop('DISPLAY', None)
    environment.pop('XAUTHORITY', None)
    memory = []
    checks = []
    controlled=OwnedMutterInput() if os.environ.get('STUDIO_OWNED_MUTTER_INPUT')=='1' else None
    try:
        for phase in args.reference or [False, True]:
            restore = phase is True
            env = dict(environment)
            if isinstance(phase,str):
                shutil.copyfile(phase,project/'sample.epub')
                env['STUDIO_GNOME_SMOKE_EPUB_REFERENCE']='1'

            if restore:
                env['STUDIO_GNOME_SMOKE_EPUB_RESTORE'] = '1'
            with (root / ('restore.log' if restore else 'app.log')).open('w') as log:
                child = subprocess.Popen([native], cwd=root, env=env, stdout=log, stderr=log)
                observer = ProcessMemory(child.pid, gateway, include_web_content=True)
                try:
                    if controlled:
                        time.sleep(1)
                        if child.poll() is None:controlled.focus_only_window()
                    try:
                        child.wait(timeout=90)
                    except subprocess.TimeoutExpired:
                        child.terminate()
                        child.wait(timeout=10)
                        raise AssertionError(f'EPUB fixture timed out: {root}')
                finally:
                    memory.append(observer.close())
            assert child.returncode == 0, (root, child.returncode)
            for _ in range(100):
                if not observer.web_content_alive():
                    break
                time.sleep(.05)
            assert not observer.web_content_alive(), ('EPUB renderer survived app close', root)
            for name in ['backend.pid', 'opencode.pid']:
                file = root/name
                if file.exists():
                    identity = file.read_text().strip()
                    for _ in range(100):
                        if not Path('/proc',identity).exists():break
                        time.sleep(.05)
                    assert not Path('/proc',identity).exists(), ('Owned backend survived close',identity)

            file = root / ('epub-reference.json' if args.reference else 'epub-restored.json' if restore else 'epub-ui.json')
            result = json.loads(file.read_text())
            checks.append(result)
            if args.reference:
                saved=root/('reference-'+Path(phase).stem)
                saved.mkdir(exist_ok=True)
                shutil.copyfile(file,saved/'result.json')
                for image in root.glob('epub-reference-*.png'):
                    shutil.move(image,saved/image.name)
        assert not requests, ('EPUB fetched a remote resource', requests)
        assert not runtime.starts(root), 'EPUB UI started a model'
        print(json.dumps({'ok':all(item['ok'] for item in checks), 'artifacts':str(root), 'epubApi':True, 'nativeControls':checks[0] if not args.reference else None, 'references':checks if args.reference else None, 'restored':result if not args.reference else None, 'networkCanaryRequests':requests, 'observedMemory':memory}))
        assert all(item['ok'] for item in checks), (root, checks)
    finally:
        if controlled:controlled.close()
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
