#!/usr/bin/env python3
"""Native paged code editing, explicit saves, conflicts and close guards."""
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

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec = importlib.util.spec_from_file_location('source_comments', REPO/'scripts/smoke-gnome-source-comments.py')
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = [str(Path(p).resolve()) for p in [args.native, args.gateway]]
    if args.private_root:
        root = args.private_root
        with (root/'app.log').open('w') as log:
            app = subprocess.Popen([native], cwd=root, env=dict(os.environ), stdout=log, stderr=log)
            memory = ProcessMemory(app.pid, gateway)
            try:
                app.wait(timeout=140)
            finally:
                if app.poll() is None:
                    app.terminate()
                    app.wait(timeout=10)
                measured = memory.close()
        result = json.loads((root/'source-edit-result.json').read_text())
        assert app.returncode == 0 and result['ok'], (root, result)
        original = (root/'original.bin').read_bytes()
        boundary = sum(len(line) for line in original.splitlines(keepends=True)[:200])
        expected = '// first 中文😀\r\n'.encode()+original[:boundary]+'// second 😀\r\n'.encode()+original[boundary:]
        assert (root/'project/edit.rs').read_bytes() == expected, 'Changes escaped their page boundaries'
        assert (root/'project/other.py').read_bytes() == '# local 中文😀\n'.encode()+(root/'other-original.bin').read_bytes()
        assert (root/'project/real-main.rs').read_bytes() == '// Native source editing fixture 中文😀\n'.encode()+(root/'real-original.bin').read_bytes()
        assert (root/'readonly/shared.rs').read_text() == '// Shared read-only file 中文😀\n'
        assert not any(w in (root/'app.log').read_text() for w in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
        for name in ['backend.pid', 'opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Owned provider survived'
        assert not source.runtime.starts(root), 'Source editing sent a model prompt'
        print(json.dumps(dict(ok=True, checks=result, memory=measured, exactFileBytes=True, ownedCleanup=True, artifacts=str(root))))
        return
    base = REPO/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='source-edit-', dir=base))
    source.runtime.create_profile(root)
    original = (''.join(f'// line {n}: 中文😀\r\n' for n in range(1, 601))+'x'*(4*1024*1024)+'\r\n').encode()
    (root/'original.bin').write_bytes(original)
    (root/'project/edit.rs').write_bytes(original)
    other = '# Original Python\nprint("中文😀")\n'.encode()
    (root/'project/other.py').write_bytes(other)
    (root/'other-original.bin').write_bytes(other)
    real = (REPO/'native/gnome/src/app.rs').read_bytes()
    (root/'project/real-main.rs').write_bytes(real)
    (root/'real-original.bin').write_bytes(real)
    (root/'readonly').mkdir()
    (root/'readonly/shared.rs').write_text('// Shared read-only file 中文😀\n')
    settings = root/'config/codex-thread-studio/settings.json'
    raw = json.loads(settings.read_text()); raw['sharedDocumentDirectories'] = [str(root/'readonly')]; settings.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target')))
    source.write_providers(root)
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
               CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
               XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'),
               GDK_BACKEND='wayland', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root),
               STUDIO_GNOME_SMOKE_SOURCE_EDIT='1')
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11',
                        '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,
                        str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],
                       env=private,cwd=root,check=True,timeout=170)


if __name__ == '__main__':
    main()
