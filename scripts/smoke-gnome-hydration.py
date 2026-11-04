#!/usr/bin/env python3
"""Bounded initial session hydration through private providers and owned Wayland.

No model turns, production profiles, credentials or desktop notifications.
"""
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
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('history_fixture',Path(__file__).with_name('smoke-gnome-conversation-pages.py'))
history=importlib.util.module_from_spec(spec);spec.loader.exec_module(history)
runtime=history.runtime

HELPERS=r'''
TOTAL=20000
ACTIVE=False
PHASE='initial'
ITEM_CURSOR='resume-items:中文/+?&='
TURN_CURSOR='resume-turns:中文/+?&='
old_summary=summary
def summary(identity):
    value=old_summary(identity)
    if identity=='target':value['status']={'type':'active' if ACTIVE else 'idle'}
    return value
def recent(identity):
    return [{'id':'h'+str(i),'type':'agentMessage','text':'History **item '+str(i)+'** 中文😀'} for i in range(TOTAL-120,TOTAL)] if identity=='target' else []
def turn(identity,full):
    return {'id':'turn-'+PHASE if ACTIVE else 'historical','status':'inProgress' if ACTIVE else 'completed','items':([{'id':'h'+str(i),'type':'agentMessage','text':'History **item '+str(i)+'** 中文😀'} for i in range(TOTAL)] if full else [])} if identity=='target' else None
def emit(values):
    with lock:
        for value in values:print(json.dumps(value),flush=True)
def begin(phase):
    global ACTIVE,PHASE
    ACTIVE=True;PHASE=phase
    emit([
        {'method':'turn/started','params':{'threadId':'target','turn':{'id':'turn-'+PHASE}}},
        {'method':'item/started','params':{'threadId':'target','turnId':'turn-'+PHASE,'item':{'id':'agent-'+PHASE,'type':'agentMessage','text':''}}},
        {'method':'item/agentMessage/delta','params':{'threadId':'target','turnId':'turn-'+PHASE,'itemId':'agent-'+PHASE,'delta':'First 中文 '}}
    ])
def finish():
    global ACTIVE
    if not ACTIVE:return
    ACTIVE=False
    emit([
        {'method':'item/agentMessage/delta','params':{'threadId':'target','turnId':'turn-'+PHASE,'itemId':'agent-'+PHASE,'delta':'second 😀'}},
        {'method':'item/completed','params':{'threadId':'target','turnId':'turn-'+PHASE,'item':{'id':'agent-'+PHASE,'type':'agentMessage','text':'First 中文 second 😀'}}},
        {'method':'turn/completed','params':{'threadId':'target','turn':{'id':'turn-'+PHASE,'status':'completed'}}},
        {'method':'thread/status/changed','params':{'threadId':'target','status':{'type':'idle'}}}
    ])
def events():
    while True:
        if (root/'hydration-finish').exists():
            (root/'hydration-finish').unlink();finish()
        time.sleep(.01)
threading.Thread(target=events,daemon=True).start()
def page(identity,result,session):
    def work():
        if session=='target' and (root/'hydration-inject').exists():
            phase=(root/'hydration-inject').read_text();(root/'hydration-inject').unlink();begin(phase)
        end=time.monotonic()+10
        if (root/('hydration-hold-'+session)).exists():(root/'hydration-waiting').write_text(session)
        while (root/('hydration-hold-'+session)).exists() and time.monotonic()<end:time.sleep(.01)
        with (root/'hydration-bytes.jsonl').open('a') as f:f.write(json.dumps({'size':len(json.dumps(result,ensure_ascii=False,separators=(',',':')).encode()),'modern':not (root/'hydration-no-items').exists()})+'\n')
        answer(identity,result)
    threading.Thread(target=work,daemon=True).start()
'''
READS=r'''    elif method in ('thread/read','thread/resume'):
        assert session in ('target','child','router'),session
        if method=='thread/resume' and params.get('excludeTurns') and (root/'hydration-old-resume').exists():
            answer(identity,error='Invalid params: unknown field excludeTurns');continue
        value={**summary(session),'historyMode':'paginated','turns':[]}
        if method=='thread/read' and params.get('includeTurns') is True or method=='thread/resume' and not params.get('excludeTurns'):
            value['turns']=[turn(session,True)] if session=='target' else []
        result={'thread':value}
        if method=='thread/resume':result.update(itemsBackwardsCursor=ITEM_CURSOR,turnsBackwardsCursor=TURN_CURSOR)
        answer(identity,result)
'''
PAGES=r'''    elif method=='thread/items/list':
        assert params.get('limit')==120 and params.get('sortDirection')=='desc',params
        assert params.get('cursor') in (None,ITEM_CURSOR),params
        if (root/'hydration-no-items').exists():answer(identity,error='Method not found: thread/items/list');continue
        if (root/'hydration-invalid').exists():answer(identity,{'data':{},'nextCursor':None});continue
        result={'data':[{'turnId':'historical','item':item} for item in reversed(recent(session))],'nextCursor':None}
        page(identity,result,session)
    elif method=='thread/turns/list':
        assert params.get('limit')==4 and params.get('sortDirection')=='desc',params
        assert params.get('itemsView') in ('summary','full'),params
        if (root/'hydration-no-turns').exists():answer(identity,error='Method not found: thread/turns/list');continue
        assert params.get('cursor') in (None,TURN_CURSOR),params
        value=turn(session,params['itemsView']=='full')
        if params['itemsView']=='summary' and (root/'hydration-invalid-state').exists() and value:value['status']='unknown-status'
        result={'data':[value] if value else [],'nextCursor':None}
        if params['itemsView']=='summary' and (root/'hydration-stale-state').exists():
            (root/'hydration-stale-state').unlink();finish();time.sleep(.1)
        page(identity,result,session)
'''

def providers():
    codex=runtime.CODEX.replace('for raw in sys.stdin:',HELPERS+'\nfor raw in sys.stdin:',1)
    codex=history.replace_region(codex,"    elif method=='thread/read':","    elif method=='fuzzyFileSearch':",READS)
    codex=history.replace_region(codex,"    elif method=='thread/turns/list':","    elif method=='turn/interrupt':",PAGES)
    codex=codex.replace("    elif method=='thread/list':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':",1)
    codex=codex.replace("if 'id' not in request:continue","if 'id' not in request or 'method' not in request:continue",1)
    return codex,history.providers()[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');parser.add_argument('--headless',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='hydration-ui-',dir=base));runtime.create_profile(root)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    (root/'hydration-hold-target').touch();(root/'hydration-inject').write_text('initial')
    for name,source in zip(['fake-codex.py','fake-opencode.py'],providers()):
        compile(source,name,'exec');path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_HYDRATION='1')
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is needed for --headless testing')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with history.isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=160)

def run(native,gateway,root,env):
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log)
        memory=history.ProcessMemory(child.pid,gateway)
        try:code=child.wait(timeout=145)
        except BaseException:child.terminate();child.wait(timeout=10);raise
        finally:measure=memory.close()
    result=json.loads((root/'hydration.json').read_text()) if (root/'hydration.json').exists() else {'ok':False,'error':'No native fixture result'}
    assert code==0 and result['ok'],(root,code,result,(root/'app.log').read_text()[-3000:])
    requests=runtime.requests(root)
    assert not runtime.starts(root),'Reading history started a model turn'
    reads=[v for v in requests if v.get('method')=='thread/resume']
    assert reads[0]['params']['excludeTurns'] is True,reads[:2]
    assert sum(v.get('method')=='thread/resume' and v['params'].get('excludeTurns') is not True for v in requests)==1,'Legacy resume should be retried exactly once'
    assert sum(v.get('method')=='thread/read' and v['params'].get('includeTurns') is True for v in requests)==1,'Only the explicit legacy compatibility phase may read full history'
    assert any(v.get('method')=='thread/turns/list' and v['params'].get('cursor')=='resume-turns:中文/+?&=' for v in requests)
    assert all(v['params'].get('cursor') in (None,'resume-items:中文/+?&=') for v in requests if v.get('method')=='thread/items/list')
    assert all(v['params'].get('limit')==4 and v['params']['itemsView'] in ('full','summary') for v in requests if v.get('method')=='thread/turns/list')
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        pid=(root/name).read_text().strip()
        assert not Path('/proc',pid).exists(),f'Owned provider survived: {pid}'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-3000:])
    sizes=[json.loads(s) for s in (root/'hydration-bytes.jsonl').read_text().splitlines()]
    modern=[v['size'] for v in sizes if v['modern']]
    print(json.dumps(dict(result,providerHistoryItems=20000,modernPagePayloadBytes={'largest':max(modern),'responses':len(modern),'total':sum(modern)},memory=measure,backendCleanup=True,artifacts=str(root))))

if __name__=='__main__':main()
