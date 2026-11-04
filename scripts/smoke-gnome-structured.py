#!/usr/bin/env python3
"""Structured native Preview, literal source mapping and explicit save on Wayland."""
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
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment, OwnedMutterInput
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('structured_profile', REPO/'scripts/smoke-gnome-source-comments.py')
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(native, gateway, root):
    owned = OwnedMutterInput()
    owned.enable_clipboard_reader()
    with (root/'app.log').open('w') as log:
        app = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
        memory = ProcessMemory(app.pid, gateway, include_web_content=True)
        try:
            deadline = time.monotonic()+150
            while app.poll() is None:
                owned.respond(root)
                if time.monotonic()>deadline:
                    raise TimeoutError(('Structured Preview timed out', root))
                time.sleep(.01)
        except BaseException:
            app.terminate()
            app.wait(timeout=10)
            raise
        finally:
            measured = memory.close()
            owned.close()
    result = json.loads((root/'structured-result.json').read_text())
    assert app.returncode == 0 and result['ok'], (root, result, (root/'app.log').read_text()[-2500:])
    assert measured['webContentProcessesObserved'] == 0, 'Structured files started a web-content process'
    assert not source.runtime.starts(root), 'Structured files sent a provider prompt'
    expected = json.loads((root/'expected-hashes.json').read_text())
    for path, original in expected.items():
        if path == 'project/compact.json5':
            assert (root/path).read_bytes() == '// saved 中文😀\r\n'.encode()+(root/'original.bin').read_bytes()
        elif path == 'project/many.json':
            assert (root/path).read_bytes() == b' '+(root/'many-original.bin').read_bytes()
        else:
            assert sha(root/path) == original, ('Reader modified fixture', path)
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
    for name in ['backend.pid', 'opencode.pid']:
        if (root/name).exists():
            source.runtime.until(lambda: not Path('/proc', (root/name).read_text().strip()).exists(), seconds=10)
    measurement = dict(result, memory=measured, exactFileBytes=True, noProviderPrompts=True,
                       noWebContent=True, ownedCleanup=True, artifacts=str(root))
    (root/'measurement.json').write_text(json.dumps(measurement, indent=2)+'\n')
    print(json.dumps(measurement))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = [str(Path(p).resolve()) for p in [args.native, args.gateway]]
    if args.private_root:
        return run(native, gateway, args.private_root)
    base = REPO/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='structured-', dir=base))
    source.runtime.create_profile(root)
    files = {
        'compact.json5': '// original 中文😀\r\n{"items":[1e+04,0xFF],"n":1,"n":2,"text":"汉字😀"}\r\n',
        'tree.xml': "<?xml version='1.0'?><root><item>中文&amp;😀</item><empty/></root>",
        'data.yaml': 'value: |\n  literal = content\n  中文😀\n',
        'settings.toml': 'key=2\nvalue="""\ninside=unchanged\n"""\n',
        '.env.local': 'URL=https://host/x=y\r\n',
        'invalid.json': '{broken',
        'mixed.xml': '<p>before <b>bold</b> after</p>',
        'many.json': json.dumps([dict(row=f'last_row_{i}', value='context 中文😀 '*3) for i in range(1200)], ensure_ascii=False, separators=(',', ':')),
        'large.json': json.dumps(dict(value='x'*(4*1024*1024))),
        'other.rs': '// other native file\n',
    }
    for name, text in files.items():
        (root/'project'/name).write_bytes(text.encode())
    (root/'original.bin').write_bytes(files['compact.json5'].encode())
    (root/'many-original.bin').write_bytes(files['many.json'].encode())
    (root/'readonly').mkdir()
    (root/'readonly/shared.json').write_text('{"readonly":true}')
    hashes={str(p.relative_to(root)):sha(p) for p in (root/'project').iterdir()}
    hashes['readonly/shared.json']=sha(root/'readonly/shared.json')
    (root/'expected-hashes.json').write_text(json.dumps(hashes))
    settings=root/'config/codex-thread-studio/settings.json'
    raw=json.loads(settings.read_text());raw['sharedDocumentDirectories']=[str(root/'readonly')];settings.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    source.write_providers(root)
    env=dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
             CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
             CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
             CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
             XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'),
             GDK_BACKEND='wayland', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix',
             STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_STRUCTURED='1', STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11',
                        '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,
                        str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],
                       env=private,cwd=root,check=True,timeout=180)


if __name__ == '__main__':
    main()
