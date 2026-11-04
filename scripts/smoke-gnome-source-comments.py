#!/usr/bin/env python3
"""Native code/text comment capture, explicit save, restart and quote relocation."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import sqlite3
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('source_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = (str(Path(p).resolve()) for p in [args.native, args.gateway])
    if args.private_root:
        root = args.private_root
        results = []
        for phase in ['capture', 'restore']:
            env = dict(os.environ, STUDIO_GNOME_SMOKE_SOURCE_COMMENTS=phase)
            with (root/(phase+'.log')).open('w') as log:
                app = subprocess.Popen([native], cwd=root, env=env, stdout=log, stderr=log)
                memory = ProcessMemory(app.pid, gateway)
                try:
                    app.wait(timeout=110)
                finally:
                    if app.poll() is None:
                        app.terminate()
                        app.wait(timeout=10)
                    measured = memory.close()
            result = json.loads((root/'source-comments-result.json').read_text())
            assert app.returncode == 0 and result['ok'], (root, phase, result)
            assert not any(w in (root/(phase+'.log')).read_text() for w in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
            results.append(dict(phase=phase, checks=result, memory=measured))
            (root/'source-comments-result.json').rename(root/(phase+'.json'))
            for name in ['backend.pid', 'opencode.pid']:
                if (root/name).exists():
                    assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Owned provider survived'
            assert not runtime.starts(root), 'Source comments sent a model prompt'
            path = root/'project/code comments 中文.rs'
            original = (root/'original-code.bin').read_bytes()
            if phase == 'capture':
                assert path.read_bytes() == original, 'Capturing a source comment wrote the file'
                record = result['record']
                (root/'expected-source-record.json').write_text(json.dumps(record, ensure_ascii=False))
                path.write_bytes(b'// added\r\n'*3+original)
                runtime.fixture.BACKEND = runtime.CODEX
                runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
                service = runtime.fixture.Gateway(Path(gateway), root, 'codex')
                records = [record]
                try:
                    for identity, filename, excerpt, line in [
                        ('code-ambiguous','ambiguous.rs','same',1),
                        ('code-missing','missing.rs','missing passage',99999),
                        ('code-long','long.rs','needle 中文😀',1),
                        ('code-patch','actual.patch','+new',5),
                        ('code-utf16','utf16.txt','UTF16 中文😀',2),
                    ]:
                        records.append(dict(id=identity, excerpt=excerpt, note='Saved source fixture', createdAt='2026-10-25T00:00:00Z',
                                            source=dict(provider='document', version=1, anchor=dict(root=str(root/'project'), filePath=str(root/'project'/filename), baseHash='old', startLine=line, endLine=line, textKind='source'))))
                    status, _ = service.api('/studio/session-state/annotations', dict(sessionKey='codex:target', drafts=records, additional=''), method='PUT')
                    assert status == 204
                finally:
                    service.close()
                (root/'expected-records.json').write_text(json.dumps(records, ensure_ascii=False))
                # Restore GUI provider fixtures after the API helper's setup.
                write_providers(root)
            else:
                assert path.read_bytes() == b'// added\r\n'*3+original
                with sqlite3.connect('file:'+str(root/'config/codex-thread-studio/studio.sqlite3')+'?mode=ro', uri=True) as database:
                    records = json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?', ('codex:target',)).fetchone()[0])
                assert records == json.loads((root/'expected-records.json').read_text()), 'Reopening updated stored anchors'
        print(json.dumps(dict(ok=True, phases=results, ownedCleanup=True, sourceUnchanged=True, recordsUnchanged=True, artifacts=str(root))))
        return
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='source-comments-', dir=base))
    runtime.create_profile(root)
    lines = [f'// line {n}: 中文😀 context\r\n' for n in range(1, 601)]
    lines[204] = 'let target = "中文😀";  \r\n'
    lines[205] = 'let next = "second";\r\n'
    source = ''.join(lines).encode()
    (root/'original-code.bin').write_bytes(source)
    project = root/'project'
    (project/'code comments 中文.rs').write_bytes(source)
    (project/'ambiguous.rs').write_text('same\nsame\n')
    (project/'missing.rs').write_text('// Header\nother\n')
    (project/'long.rs').write_text('x'*300_000+'needle 中文😀\n')
    (project/'actual.patch').write_text('--- a/example\n+++ b/example\n@@ -1 +1 @@\n-old\n+new\n')
    (project/'utf16.txt').write_bytes(('Header\r\nUTF16 中文😀\r\n').encode('utf-16'))
    write_providers(root)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
               CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
               XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'),
               GDK_BACKEND='wayland', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root))
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless', '--wayland', '--no-x11',
                        '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,
                        str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],
                       env=private,cwd=root,check=True,timeout=245)


def write_providers(root):
    code = runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name, body in [('fake-codex.py',code), ('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name
        path.write_text(body)
        path.chmod(0o700)


if __name__ == '__main__':
    main()
