#!/usr/bin/env python3
"""Protected quote lookup and actual native Comments→Source, private copies only."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
from pathlib import Path
import resource
import sqlite3
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote
from urllib.request import Request
from urllib.error import HTTPError

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment
spec = importlib.util.spec_from_file_location('location_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

SOURCE = 'Target **中文😀** and &amp; code.  '
PREVIEW = 'Target 中文😀 and & code.'
ORIGINAL = '# Original\r\n\r\n' + SOURCE + '\r\n'
MOVED = '# New heading\r\n\r\nintro\r\n\r\n## Moved\r\n\r\ncontext\r\n' + SOURCE + '\r\n'


def api_checks(gateway, root, reference, races=False):
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    service = runtime.fixture.Gateway(Path(gateway), root, 'codex')
    project = root/'project'
    (project/'guard.md').write_text('# Guard\n\nOriginal guard source.\n')
    records, measurements = [], []
    memory = ProcessMemory(service.process.pid, gateway)

    def opened(path):
        status, data = service.api('/studio/native/markdown/open-readonly', dict(root=str(project), path=str(path)), timeout=100)
        assert status == 200, data
        identity = data['readerId']
        deadline = time.monotonic() + 100
        while time.monotonic() < deadline:
            status, index = service.api(f'/studio/native/markdown/{identity}/document')
            assert status == 200 and index['state'] != 'error', index
            if index['state'] == 'ready':
                return identity, index['index']
            time.sleep(.04)
        raise TimeoutError(('Index', path, root))

    def close(identity):
        assert service.api('/studio/markdown/close', dict(id=identity))[0] == 200

    def status_only(endpoint, method, body=None):
        request = Request(service.origin+endpoint, method=method,
                          data=json.dumps(body).encode() if body is not None else None,
                          headers={'Authorization': 'Bearer '+service.token, 'Content-Type': 'application/json'})
        try:
            with service.client.open(request, timeout=10) as response:
                return response.status
        except HTTPError as error:
            return error.code

    def locate(identity, index, query, status='unique'):
        started = time.monotonic()
        code, data = service.api(f'/studio/native/markdown/{identity}/locate-quote', dict(revision=index['revision'], query=query), timeout=100)
        assert code == 200 and data['revision'] == index['revision'] and data['located']['status'] == status, data
        measurements.append(dict(sourceBytes=index['sourceBytes'], kind=query['anchor'].get('textKind', 'auto'),
                                 status=status, roundtripMs=round((time.monotonic()-started)*1000)))
        return data['located']

    try:
        for name, kind in [('moved-source', 'source'), ('moved-preview', 'preview'), ('legacy', None)]:
            path = project/(name+'.md')
            path.write_bytes(ORIGINAL.encode())
            identity, old = opened(path)
            close(identity)
            path.write_bytes(MOVED.encode())
            anchor = dict(baseHash=old['revision'], startLine=3, endLine=3)
            if kind:
                anchor['textKind'] = kind
            excerpt = SOURCE if kind == 'source' else PREVIEW
            query = dict(quote=excerpt, anchor=anchor)
            identity, index = opened(path)
            located = locate(identity, index, query)
            # Preview anchors retain the containing reader block; Source spans
            # retain the exact selected physical lines within that block.
            assert located['position']['line'] == (8 if kind == 'source' else 7) and located['position']['endLine'] == 8, located
            endpoint = f'/studio/native/markdown/{identity}/locate-quote'
            assert service.api(endpoint, dict(revision=index['revision'], query=query), authorized=False)[0] == 401
            assert service.api(endpoint, dict(revision='stale', query=query))[0] == 409
            assert service.api(endpoint, dict(revision=index['revision'], query=dict(quote='x'*16385, anchor=anchor)))[0] == 400
            assert status_only(endpoint, 'POST', dict(revision=index['revision'], query=query, path='/etc/passwd')) in [400, 422]
            assert status_only(endpoint, 'GET') == 405
            assert path.read_bytes() == MOVED.encode(), 'Lookup wrote the source file'
            close(identity)
            assert service.api(endpoint, dict(revision=index['revision'], query=query))[0] == 410
            records.append(dict(id=name, excerpt=excerpt, note='Private source lookup', source=dict(provider='document', version=1, anchor=dict(anchor, root=str(project), filePath=str(path)))))

        for name, text, excerpt, kind, line, status in [
            ('ambiguous', '# Original\n\n'+SOURCE+'\n\n'+PREVIEW+'\n', PREVIEW, 'preview', 3, 'ambiguous'),
            ('missing', '# Other\n\nNothing matching.\n', 'Removed quoted passage', 'preview', 999999, 'missing'),
            ('unchanged', '# Heading\n\nsame\n\nsame\n', 'same', 'source', 5, 'unique'),
            ('legacy-source', '# Original\n\n[caption](https://example.invalid/raw-only-quote)\n', 'raw-only-quote', None, 3, 'unique'),
        ]:
            path = project/(name+'.md')
            path.write_text(text)
            identity, index = opened(path)
            anchor = dict(baseHash=index['revision'] if name == 'unchanged' else 'older-snapshot', startLine=line, endLine=line)
            if kind:
                anchor['textKind'] = kind
            located = locate(identity, index, dict(quote=excerpt, anchor=anchor), status)
            if status == 'unique':
                assert located['position']['line'] == line
            else:
                assert located['position'] is None
            close(identity)
            records.append(dict(id=name, excerpt=excerpt, note='Private source lookup', source=dict(provider='document', version=1, anchor=dict(anchor, root=str(project), filePath=str(path)))))

        path = project/'header-location.md'
        path.write_text('# Paged header\n\n| **Unique header** | Value |\n|---|---|\n'
                        + ''.join(f'| row {n} | '+ 'context '*10+'|\n' for n in range(800)))
        identity, index = opened(path)
        assert index['tables'][0]['endPage'] > 1
        located = locate(identity, index, dict(quote='Unique header', anchor=dict(textKind='preview')))
        assert located['position']['line'] == 3
        close(identity)
        # 32 MiB of regular table rows; no whole-file allocation in the locator.
        path = project/'large-location.md'
        row = '| context | ' + 'x'*8192 + ' |\n'
        with path.open('w') as stream:
            stream.write('# Large source lookup\n\n| Name | Value |\n|---|---|\n')
            for _ in range((32*1024*1024)//len(row)):
                stream.write(row)
            stream.write('\nUnique source location 中文😀.\n')
        identity, index = opened(path)
        located = locate(identity, index, dict(quote='Unique source location 中文😀.', anchor=dict(baseHash='older', textKind='source')))
        assert located['position']['line'] == index['sourceLines']-1
        if races:
            endpoint = f'/studio/native/markdown/{identity}/locate-quote'
            body = dict(revision=index['revision'], query=dict(quote='Missing full-table quote 中文😀.', anchor=dict(textKind='preview')))
            with ThreadPoolExecutor(max_workers=1) as worker:
                running = worker.submit(service.api, endpoint, body, timeout=100)
                deadline = time.monotonic()+10
                while time.monotonic() < deadline:
                    status, _ = service.api(endpoint, dict(revision=index['revision'], query=dict(quote='context', anchor=dict(textKind='source'))))
                    if status == 429:
                        break
                    assert not running.done(), 'Lookup completed before close race could be observed'
                    time.sleep(.005)
                else:
                    raise AssertionError('Bounded lookup admission was not observed')
                close(identity)
                assert running.result(timeout=100)[0] == 410, 'Closed snapshot returned a verified location'
        else:
            close(identity)
        if reference:
            path = project/'reference-location.md'
            shutil.copyfile(reference, path)
            with path.open('a') as stream:
                stream.write('\n\n## Private relocated reference\n\n**Unique reference** quote 中文😀.\n')
            identity, index = opened(path)
            located = locate(identity, index, dict(quote='Unique reference quote 中文😀.', anchor=dict(baseHash='older', textKind='preview')))
            assert located['position']['line'] == index['sourceLines']-1
            (root/'reference-line.json').write_text(json.dumps(located['position']['line']))
            records.append(dict(id='reference', excerpt='Unique reference quote 中文😀.', note='Private report source lookup',
                                source=dict(provider='document', version=1, anchor=dict(root=str(project), filePath=str(path), baseHash='older', startLine=3, endLine=3, textKind='preview'))))
            close(identity)
        endpoint = '/studio/session-state/annotations'
        for record in records:
            record['createdAt'] = '2026-10-25T00:00:00Z'
        assert service.api(endpoint, dict(sessionKey='codex:target', drafts=records, additional=''), method='PUT')[0] == 204
        saved = service.api(endpoint+'?sessionKey='+quote('codex:target'))[1]
        assert saved['drafts'] == records
        (root/'expected-comment-records.json').write_text(json.dumps(records, ensure_ascii=False))
        assert not runtime.starts(root)
        return dict(ok=True, measurements=measurements, gatewayPeakRssKiB=memory.close()['nativePeakRssKiB'], recordsUnchanged=True, auth=True, snapshotRevision=True, privatePaths=True, closeDuringLookup=races, boundedAdmission=races)
    finally:
        (root/'api-measurements.json').write_text(json.dumps(measurements, indent=2))
        memory.close()
        service.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--api-only', action='store_true')
    parser.add_argument('--races', action='store_true', help='observe bounded admission and close a private large reader during active quote lookup')
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = (str(Path(value).resolve()) for value in [args.native, args.gateway])
    if args.private_root:
        root = args.private_root
        with (root/'native.log').open('w') as log:
            app = subprocess.Popen([native], cwd=root, stdout=log, stderr=log)
            memory = ProcessMemory(app.pid, gateway)
            try:
                app.wait(timeout=130)
            finally:
                if app.poll() is None:
                    app.terminate()
                    app.wait(timeout=10)
                measured = memory.close()
        result = json.loads((root/'comment-location-result.json').read_text())
        assert app.returncode == 0 and result['ok'], (root, result)
        assert not any(word in (root/'native.log').read_text() for word in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
        assert not runtime.starts(root)
        with sqlite3.connect('file:'+str(root/'config/codex-thread-studio/studio.sqlite3')+'?mode=ro', uri=True) as database:
            saved = json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?', ('codex:target',)).fetchone()[0])
        assert saved == json.loads((root/'expected-comment-records.json').read_text()), 'Reopening rewrote stored anchors'
        for name in ['backend.pid', 'opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc', (root/name).read_text().strip()).exists()
        print(json.dumps(dict(ok=True, checks=result, memory=measured, artifacts=str(root))))
        return
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='comment-location-', dir=base))
    runtime.create_profile(root)
    report = api_checks(gateway, root, args.reference, args.races)
    (root/'api.json').write_text(json.dumps(report, indent=2))
    if args.api_only:
        print(json.dumps(dict(ok=True, api=report, artifacts=str(root))))
        return
    code = runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name, body in [('fake-codex.py', code), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root/name
        path.write_text(body)
        path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    settings = root/'config/codex-thread-studio/settings.json'
    preferences = json.loads(settings.read_text())
    preferences['language'] = 'en-US'
    settings.write_text(json.dumps(preferences))
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
               CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
               XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland',
               GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root),
               STUDIO_GNOME_SMOKE_COMMENT_LOCATION='1')
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless', '--wayland', '--no-x11',
                        '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--', sys.executable,
                        str(Path(__file__).resolve()), native, gateway, '--private-root', str(root)],
                       env=private, cwd=root, check=True, timeout=150)


if __name__ == '__main__':
    main()
