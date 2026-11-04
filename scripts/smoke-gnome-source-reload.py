#!/usr/bin/env python3
"""Native ordinary-file Reload and destructive-operation guards on Wayland."""
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

sys.dont_write_bytecode=True
REPO=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('reload_profile',REPO/'scripts/smoke-gnome-source-comments.py')
source=importlib.util.module_from_spec(spec);spec.loader.exec_module(source)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root',type=Path)
    args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=[str(Path(p).resolve()) for p in [args.native,args.gateway]]
    if args.private_root:
        root=args.private_root
        with (root/'app.log').open('w') as log:
            app=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log)
            memory=ProcessMemory(app.pid,gateway,include_web_content=True)
            try:app.wait(timeout=160)
            finally:
                if app.poll() is None:app.terminate();app.wait(timeout=10)
                measured=memory.close()
        result=json.loads((root/'source-reload-result.json').read_text())
        assert app.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2000:])
        assert (root/'project/reload.rs').read_bytes()==b'// Restored after missing file\r\n'
        assert (root/'readonly/shared.json').read_text()=='{"version":2}'
        assert (root/'project/reload.html').read_text()=='<h1>Reloaded HTML 中文😀</h1>'
        assert (root/'project/other.rs').read_text()=='// Other source\n'
        assert not source.runtime.starts(root),'Reload sent a provider prompt'
        assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL','panicked','Gtk-WARNING'])
        for name in ['backend.pid','opencode.pid']:
            if (root/name).exists():source.runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(),seconds=10)
        deadline=time.monotonic()+10
        while memory.web_content_alive() and time.monotonic()<deadline:time.sleep(.1)
        assert not memory.web_content_alive(),'Owned HTML content survived close'
        result.update(memory=measured,exactFileBytes=True,noProviderPrompts=True,ownedCleanup=True,artifacts=str(root))
        (root/'measurement.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));return
    base=REPO/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='source-reload-',dir=base));source.runtime.create_profile(root)
    (root/'project/reload.rs').write_bytes(''.join(f'// Line {i}: 中文😀\r\n' for i in range(600)).encode())
    (root/'project/other.rs').write_text('// Other source\n')
    (root/'project/reload.html').write_text('<h1>Original HTML</h1>')
    (root/'readonly').mkdir();(root/'readonly/shared.json').write_text('{"version":1}')
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['sharedDocumentDirectories']=[str(root/'readonly')];settings.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    source.write_providers(root)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
        CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
        CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),
        READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',
        STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_SOURCE_RELOAD='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11',
            '--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),
            native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=190)


if __name__=='__main__':main()
