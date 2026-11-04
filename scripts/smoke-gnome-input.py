#!/usr/bin/env python3
"""Native questions/MCP forms over the actual protocol, in private Wayland/URI services."""
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
spec=importlib.util.spec_from_file_location('input_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from private_uri_portal import UriPortal
from memory_sampler import ProcessMemory

PUBLISH=r'''
def publish_inputs():
    while True:
        path=root/'input-events.json'
        if path.exists():
            values=json.loads(path.read_text());path.unlink()
            with lock:
                for value in values:print(json.dumps(value),flush=True)
        time.sleep(.01)
threading.Thread(target=publish_inputs,daemon=True).start()
'''

def run(native,gateway,root,environment):
    portal=UriPortal(root)
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=environment,stdout=log,stderr=log)
        memory=ProcessMemory(child.pid,gateway)
        try:
            deadline=time.monotonic()+90
            while child.poll() is None:
                portal.iterate()
                if portal.closed:(root/'input-portal-closed.json').write_text(json.dumps(portal.closed))
                if time.monotonic()>deadline:raise TimeoutError(('Native inputs timed out',root))
                time.sleep(.015)
        except BaseException:
            child.terminate();child.wait(timeout=10);raise
        finally:
            measured=memory.close();portal.close()
    result=json.loads((root/'inputs.json').read_text())
    assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-3000:])
    requests=runtime.requests(root)
    assert not runtime.starts(root),'Input acceptance unexpectedly started a model turn'
    assert portal.requests==['https://example.invalid/native-auth?state=test','https://example.invalid/pending-auth'],portal.requests
    assert portal.closed,'Resolved input did not cancel its pending URI launch'
    held=next(item['handle'] for item in reversed(portal.trace) if item['event']=='request')
    assert held in portal.closed and not any(item['event']=='response' and item.get('handle')==held and item.get('status')==0 for item in portal.trace),'Held URL request completed instead of being cancelled'
    answers=[r for r in requests if 'method' not in r and 'id' in r]
    assert len(answers)==29,(root,answers)
    assert next(r for r in answers if r['id']=='question-option-after-custom')['result']['answers']['custom']['answers']==['Default answer']
    assert not any(r['id']=='resolved-policy' for r in answers)
    for identity in ['policy-exec','policy-allow','policy-deny','cancel-command','cancel-files','empty-decisions','bad-policy','legacy-proposal-once']:
        assert sum(r['id']==identity for r in answers)==1,(identity,answers)
    for identity in ['permissions-subset-review','permissions-subset-session','permissions-none','permissions-all','permissions-unsupported','permissions-cancel-pending','permissions-reconnect-pending','permissions-large']:
        assert sum(r['id']==identity for r in answers)==1,(identity,answers)
    assert not any(r['id'] in ['permissions-resolved-pending','permissions-disconnected-pending'] for r in answers)
    assert sum(r['id']=='questions' for r in answers)==1
    assert not any(r['id']==7 for r in answers)
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Theme parser error'])
    for name in ['backend.pid','opencode.pid']:
        path=root/name
        if path.exists():assert not Path('/proc',path.read_text().strip()).exists(),'Owned provider survived shutdown'
    print(json.dumps(dict(result,artifacts=str(root),memory=measured,wireReplies=len(answers),uriRequests=portal.requests,ownedBackendCleanup=True)))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--headless',action='store_true');parser.add_argument('--private-root');parser.add_argument('--language',choices=['en-US','zh-CN'],default='en-US');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    artifacts=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';artifacts.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='inputs-ui-',dir=artifacts));runtime.create_profile(root)
    preferences=root/'config/codex-thread-studio/settings.json'
    raw=json.loads(preferences.read_text());raw['language']=args.language;preferences.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    backend=runtime.CODEX.replace('for raw in sys.stdin:',PUBLISH+'\nfor raw in sys.stdin:',1)
    backend=backend.replace("    if 'id' not in request:continue","    if 'id' not in request or 'method' not in request:continue")
    backend=backend.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_INPUT='1',STUDIO_GNOME_SMOKE_INPUT_LANGUAGE=args.language,GTK_USE_PORTAL='1')
    command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--'] if args.headless else []
    with isolated_environment(environment,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=120)

if __name__=='__main__':main()
