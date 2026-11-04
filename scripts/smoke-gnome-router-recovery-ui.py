#!/usr/bin/env python3
"""Native controller inspection/adoption, using private Wayland and fake providers."""
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
import time

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('router_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--headless',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.headless and os.environ.get('STUDIO_RECOVERY_OWNED_DESKTOP')!='1':
        with tempfile.TemporaryDirectory(prefix='studio-recovery-desktop-',dir='/tmp') as directory:
            p=Path(directory);environment=dict(os.environ,STUDIO_RECOVERY_OWNED_DESKTOP='1',GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(p/'config'),XDG_DATA_HOME=str(p/'data'),XDG_CACHE_HOME=str(p/'cache'))
            with isolated_environment(environment,headless=True) as private:
                subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),*sys.argv[1:]],env=private,check=True,timeout=160)
        return
    native,gateway=[str(Path(value).resolve()) for value in [args.native,args.gateway]]
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='router-recovery-ui-',dir=base));runtime.create_profile(root)
    settings=root/'config/codex-thread-studio/settings.json'
    original=json.loads(settings.read_text());original['router']['controllers']={};original['router'].pop('threadId',None);settings.write_text(json.dumps(original))
    (settings.parent/'gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    workspace=root/'data/codex-thread-studio/router';workspace.mkdir(parents=True,exist_ok=True)
    # Simulate a persisted create intent with no returned ID, using an existing
    # session in the fake backend. The API lifecycle fixture covers actual loss.
    with sqlite3.connect(settings.parent/'studio.sqlite3') as db:
        db.execute('CREATE TABLE native_router_setup(backend TEXT PRIMARY KEY,workspace TEXT NOT NULL,thread_id TEXT)')
        db.execute('INSERT INTO native_router_setup VALUES(?,?,NULL)',('codex',str(workspace.resolve())))
    backend=runtime.CODEX.replace("assert params['includeTurns'] is False",'pass').replace("assert params['excludeTurns'] is True",'pass')
    backend=backend.replace("    elif method=='turn/start':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='turn/start':")
    backend=backend.replace("        value=summary(session)","        value=summary(session)\n        if session=='router' and (root/'force-recovery-busy').exists():value['status']={'type':'active'}")
    backend=backend.replace("        answer(identity,{'thread':value})","""        if session=='router' and (root/'hold-recovery-read').exists():
            def delayed(identity=identity,value=value):
                while (root/'hold-recovery-read').exists():time.sleep(.02)
                time.sleep(.12);answer(identity,{'thread':value})
            threading.Thread(target=delayed,daemon=True).start();continue
        answer(identity,{'thread':value})""")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root))
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    results=[]
    for phase in ['initial','restore']:
        environment['STUDIO_GNOME_SMOKE_ROUTER_RECOVERY']=phase
        with (root/f'app-{phase}.log').open('w') as output:
            child=subprocess.Popen([native],env=environment,cwd=root,stdout=output,stderr=output);memory=ProcessMemory(child.pid,gateway)
            try:child.wait(timeout=55)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'Controller recovery timed out: {root}')
            finally:measurement=memory.close()
        result=json.loads((root/f'controller-recovery-{phase}.json').read_text());assert child.returncode==0 and result['ok'],(root,result)
        result['memory']=measurement;results.append(result)
        text=(root/f'app-{phase}.log').read_text();assert not any(word in text for word in ['CRITICAL','panicked','Theme parser error']),(root,text[-3000:])
        for name in ['backend.pid','opencode.pid']:
            if (root/name).exists():
                identity=(root/name).read_text().strip()
                for _ in range(100):
                    if not Path('/proc',identity).exists():break
                    time.sleep(.05)
                assert not Path('/proc',identity).exists(),'Owned backend survived recovery fixture'
    after=json.loads(settings.read_text());expected=json.loads(json.dumps(original));expected['router']['controllers']['codex']='router'
    assert after==expected,'Adoption changed unrelated preferences or saved an unsent settings draft'
    with sqlite3.connect(settings.parent/'studio.sqlite3') as db:assert db.execute('SELECT thread_id FROM native_router_setup WHERE backend=?',('codex',)).fetchone()==('router',)
    requests=runtime.requests(root)
    assert not any(row.get('method') in ['thread/start','turn/start','thread/name/set'] for row in requests),'Recovery created, renamed or prompted a session'
    print(json.dumps({'ok':True,'artifacts':str(root),'launches':results,'preservedSettings':True,'noCreateOrPrompts':True,'backendCleanup':True}))

if __name__=='__main__':main()
